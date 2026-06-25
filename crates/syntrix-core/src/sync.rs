//! Sync status types and peer status query for Syntrix P2P organisations.
//!
//! `get_sync_info` queries the control document to produce a `SyncInfo` that:
//!   - Lists all **registered members** (from the `members/` prefix), enriched with their
//!     name, person, role, and device address.
//!   - Cross-references with **recent heartbeats** (from the `heartbeat/` prefix) to determine
//!     whether each member is currently `"online"` (heartbeat < 60 s ago) or `"offline"`.
//!
//! This ensures the peer count denominator reflects registered members, not just those
//! who happen to have sent a heartbeat recently.

use futures_util::StreamExt;

/// Status of a single peer in the organisation.
#[derive(serde::Serialize)]
pub struct PeerStatus {
    pub node_id: String,
    pub status: String,
    pub last_seen: i64,
    pub name: Option<String>,
    pub person: Option<String>,
    pub role: Option<String>,
    pub device_addr: Option<String>,
}

/// Overall sync state returned to the frontend.
#[derive(serde::Serialize)]
pub struct SyncInfo {
    pub node_id: String,
    pub is_online: bool,
    pub peers: Vec<PeerStatus>,
}

/// Query the control document and produce a `SyncInfo` describing the P2P state
/// of the organisation from this node's perspective.
pub async fn get_sync_info(
    doc: iroh_docs::api::Doc,
    store: iroh_blobs::api::Store,
    node_id: [u8; 32],
) -> anyhow::Result<SyncInfo> {
    // 1. Collect all registered active members (members/<peer_id_hex>)
    let mut member_entries =
        Box::pin(doc.get_many(iroh_docs::store::Query::key_prefix("members/")).await?);
    let mut members = std::collections::HashMap::new();

    while let Some(res) = member_entries.next().await {
        if let Ok(entry) = res {
            let key_bytes = entry.key();
            if let Ok(key) = std::str::from_utf8(key_bytes) {
                if let Some(peer_id) = key.strip_prefix("members/") {
                    if let Ok(bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if val["active"].as_bool().unwrap_or(true) {
                                members.insert(peer_id.to_string(), val);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Collect all heartbeats (heartbeat/<peer_id_hex>) → latest timestamp
    let mut heartbeat_entries =
        Box::pin(doc.get_many(iroh_docs::store::Query::key_prefix("heartbeat/")).await?);
    let mut heartbeats = std::collections::HashMap::new();

    while let Some(res) = heartbeat_entries.next().await {
        if let Ok(entry) = res {
            let key_bytes = entry.key();
            if let Ok(key) = std::str::from_utf8(key_bytes) {
                if let Some(peer_id) = key.strip_prefix("heartbeat/") {
                    if let Ok(bytes) = store.blobs().get_bytes(entry.content_hash()).await {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(ts) = val["ts"].as_i64() {
                                heartbeats.insert(peer_id.to_string(), ts);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Build peer list: registered members first, then unregistered heartbeat senders
    let now = chrono::Utc::now().timestamp_millis();
    let mut peers = Vec::new();
    let mut processed = std::collections::HashSet::new();

    for (peer_id, member_val) in members {
        let ts = heartbeats.get(&peer_id).cloned().unwrap_or(0);
        let status = if ts > 0 && now - ts < 60_000 { "online" } else { "offline" };

        let name       = member_val["name"].as_str().map(String::from);
        let person     = member_val["person"].as_str().map(String::from);
        let role       = member_val["role"].as_str().map(String::from);
        let device_addr = member_val["device_addr"].as_str().map(String::from);

        processed.insert(peer_id.clone());
        peers.push(PeerStatus {
            node_id: peer_id,
            status: status.to_string(),
            last_seen: ts,
            name,
            person,
            role,
            device_addr,
        });
    }

    // Include heartbeat senders not explicitly registered as members
    for (peer_id, ts) in heartbeats {
        if !processed.contains(&peer_id) {
            let status = if now - ts < 60_000 { "online" } else { "offline" };
            peers.push(PeerStatus {
                node_id: peer_id,
                status: status.to_string(),
                last_seen: ts,
                name: None,
                person: None,
                role: None,
                device_addr: None,
            });
        }
    }

    Ok(SyncInfo {
        node_id: hex::encode(node_id),
        is_online: true,
        peers,
    })
}
