use crate::identity::AppState;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

#[derive(Debug, Deserialize)]
pub struct SyncEventEncoded {
    pub name: String, pub args: serde_json::Value,
    #[serde(rename = "seqNum")] pub seq_num: u64,
    #[serde(rename = "parentSeqNum")] pub parent_seq_num: u64,
    #[serde(rename = "clientId")] pub client_id: String,
    #[serde(rename = "sessionId")] pub session_id: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncEntry {
    #[serde(rename = "eventEncoded")] pub event_encoded: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SyncPullResult {
    pub batch: Vec<SyncEntry>,
    #[serde(rename = "hasMore")] pub has_more: bool,
    pub cursor: Option<HlcCursor>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HlcCursor { pub ts: u64, pub count: u32, pub node: String }

#[derive(Debug, Serialize)]
pub struct ConnectionState { pub connected: bool, pub peers: u32 }

pub fn sync_push(state: &mut AppState, org_id: &str, batch: Vec<SyncEventEncoded>) -> anyhow::Result<()> {
    let org = state.get_org_docs(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let doc = org.operational_doc.clone();
    let author = state.author();
    let node_hex = hex::encode(state.node_id());

    for event in &batch {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
        let counter = state.counter();
        let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
        let key = format!("evt:{:020}:{:08}:{}", ts, count, &node_hex[..16]);
        let value = serde_json::json!({
            "type": event.name, "hlc": {"ts":ts,"count":count,"node":&node_hex[..16]},
            "payload": &event.args, "seqNum": event.seq_num, "parentSeqNum": event.parent_seq_num,
            "clientId": event.client_id, "sessionId": event.session_id,
        });
        tauri::async_runtime::block_on(doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value)?))?;
        
        let entity = event.name.split('.').next().unwrap_or(&event.name);
        let doc_id = event.args.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| event.args.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or(&key);
        let _ = state.indexer.upsert_document(org_id, entity, doc_id, &event.args);
    }
    // Record after loop to avoid borrow conflict
    for event in batch {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
        let count = state.counter().load(std::sync::atomic::Ordering::SeqCst) as u32;
        let value = serde_json::json!({"hlc":{"ts":ts,"count":count,"node":&node_hex[..16]}});
        state.record_event(org_id, event.seq_num, value);
    }
    Ok(())
}

pub fn sync_pull(state: &AppState, org_id: &str, cursor: Option<HlcCursor>) -> SyncPullResult {
    let since = cursor.as_ref().map(|c| c.ts).unwrap_or(0);
    let all = state.get_events_since(org_id, since);
    let (batch, has_more) = if all.len() > 100 {
        (all[..100].to_vec(), true)
    } else {
        (all, false)
    };
    let cursor = batch.last().and_then(|e| {
        e.event_encoded.get("hlc").and_then(|h| Some(HlcCursor {
            ts: h.get("ts")?.as_u64()?,
            count: h.get("count")?.as_u64()? as u32,
            node: h.get("node")?.as_str()?.to_string(),
        }))
    });
    SyncPullResult { batch, has_more, cursor }
}

pub fn sync_ping(state: &AppState, org_id: &str) -> anyhow::Result<ConnectionState> {
    state.get_org_docs(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    Ok(ConnectionState { connected: true, peers: 0 })
}

pub fn sync_status(state: &AppState) -> String {
    format!("online · {} orgs · node {}", state.list_orgs().len(), hex::encode(state.node_id())[..8].to_string())
}

pub fn start_heartbeat(doc: iroh_docs::api::Doc, author: iroh_docs::AuthorId, node_id: String) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let key = format!("heartbeat/{}", node_id);
            let val = serde_json::json!({"ts": ts, "status": "online"});
            let _ = doc.set_bytes(author, key.into_bytes(), serde_json::to_vec(&val).unwrap()).await;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

#[derive(serde::Serialize)]
pub struct PeerStatus {
    pub node_id: String,
    pub status: String,
    pub last_seen: i64,
}

#[derive(serde::Serialize)]
pub struct SyncInfo {
    pub node_id: String,
    pub is_online: bool,
    pub peers: Vec<PeerStatus>,
}

pub async fn get_sync_info(state: &AppState, org_id: &str) -> anyhow::Result<SyncInfo> {
    let org_state = state.get_org_docs(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let doc = &org_state.control_doc;
    
    let mut entries = Box::pin(doc.get_many(iroh_docs::store::Query::key_prefix("heartbeat/")).await?);
    let mut peers = Vec::new();
    let now = chrono::Utc::now().timestamp_millis();
    
    while let Some(res) = entries.next().await {
        let entry = res?;
        let key_bytes = entry.key();
        if let Ok(key) = std::str::from_utf8(key_bytes) {
            if let Some(peer_id) = key.strip_prefix("heartbeat/") {
                if let Ok(bytes) = state.store().blobs().get_bytes(entry.content_hash()).await {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(ts) = val["ts"].as_i64() {
                            let status = if now - ts < 60000 { "online" } else { "offline" };
                            peers.push(PeerStatus {
                                node_id: peer_id.to_string(),
                                status: status.to_string(),
                                last_seen: ts,
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(SyncInfo {
        node_id: hex::encode(state.node_id()),
        is_online: true,
        peers,
    })
}
