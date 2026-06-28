//! Heartbeat and peer re-sync loop for Syntrix P2P nodes.
//!
//! Every active node (admin or client) runs `start_heartbeat_with_resync` in the background.
//! This loop:
//!   1. Writes a timestamped heartbeat under `heartbeat/<own_node_id>` every 15 seconds.
//!   2. Reloads device membership from `members/` into the NamespaceRegistry AND re-dials
//!      all known active peers on all 4 docs.
//!   3. Reloads role grants from `roles/` prefix into the NamespaceRegistry so that
//!      permission changes propagate to `check_read_access()` within one heartbeat cycle.
//!
//! The device membership reload (step 2) is CRITICAL for P2P sync. When a new member appears
//! in the control doc (e.g. Client2 joined after Client1), the heartbeat loop registers the
//! new device in the registry so that the accept_cb allows its sync connections. Without this,
//! `start_sync` calls are silently rejected and live replication never works.
//!
//! The re-dial step ensures that when a peer restarts (and gets a new network address), sync
//! sessions are automatically re-established within one heartbeat cycle (~15 s).

use std::sync::{Arc, RwLock};
use futures_util::StreamExt;
use crate::addr::parse_device_addr;
use crate::registry::{NamespaceRegistry, RoleGrants, Device};

/// Simple heartbeat loop — writes a timestamp entry every 15 s.
///
/// Prefer [`start_heartbeat_with_resync`] for production use; this variant
/// does not re-establish sync sessions with offline peers.
pub fn start_heartbeat(
    doc: iroh_docs::api::Doc,
    author: iroh_docs::AuthorId,
    node_id: String,
) {
    tokio::spawn(async move {
        loop {
            let ts = chrono::Utc::now().timestamp_millis();
            let key = format!("heartbeat/{}", node_id);
            let val = serde_json::json!({"ts": ts, "status": "online"});
            let _ = doc
                .set_bytes(author, key.into_bytes(), serde_json::to_vec(&val).unwrap())
                .await;
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

/// Heartbeat + periodic re-sync + role reload loop.
///
/// Writes a heartbeat every 15 seconds, re-dials all known active peers from
/// the control document's `members/` prefix, and reloads role grants from the
/// `roles/` prefix into the `NamespaceRegistry`. This ensures that permission
/// changes (e.g. admin updating a role) propagate to `check_read_access()`
/// within one heartbeat cycle without requiring a restart.
pub fn start_heartbeat_with_resync(
    ctrl_doc: iroh_docs::api::Doc,
    cat_doc: iroh_docs::api::Doc,
    op_doc: iroh_docs::api::Doc,
    pay_doc: iroh_docs::api::Doc,
    author: iroh_docs::AuthorId,
    node_id: String,
    store: iroh_blobs::api::Store,
    secret: iroh::SecretKey,
    registry: Arc<RwLock<NamespaceRegistry>>,
) {
    tokio::spawn(async move {
        // Resolve org_id once from the control doc's namespace id via the registry.
        let org_id: String = registry
            .read()
            .ok()
            .and_then(|r| r.lookup_org(&ctrl_doc.id()))
            .unwrap_or_default();

        if org_id.is_empty() {
            eprintln!(
                "[heartbeat] WARN: org_id is empty for ctrl_doc {:?}, device registration will fail",
                ctrl_doc.id()
            );
        } else {
            eprintln!(
                "[heartbeat] starting for org_id={}, ctrl_doc={:?}",
                org_id,
                ctrl_doc.id()
            );
        }

        loop {
            // 1. Write heartbeat
            let ts = chrono::Utc::now().timestamp_millis();
            let key = format!("heartbeat/{}", node_id);
            let val = serde_json::json!({"ts": ts, "status": "online"});
            let _ = ctrl_doc
                .set_bytes(author, key.into_bytes(), serde_json::to_vec(&val).unwrap())
                .await;

            // 2. Reload device membership + re-dial all known peers
            let mut member_count = 0;
            let mut registered_count = 0;
            if let Ok(entries) = ctrl_doc
                .get_many(iroh_docs::store::Query::key_prefix("members/"))
                .await
            {
                let mut entries = Box::pin(entries);
                while let Some(res) = entries.next().await {
                    member_count += 1;
                    if let Ok(entry) = res {
                        if let Ok(key_bytes) = std::str::from_utf8(entry.key()) {
                            if let Some(node_id_str) = key_bytes.strip_prefix("members/") {
                                if let Ok(content_bytes) =
                                    store.blobs().get_bytes(entry.content_hash()).await
                                {
                                    if let Ok(val) =
                                        serde_json::from_slice::<serde_json::Value>(&content_bytes)
                                    {
                                        let active = val["active"].as_bool().unwrap_or(true);
                                        let r = val["role"].as_str().unwrap_or("sales").to_string();
                                        let person = val["person"].as_str().unwrap_or("").to_string();
                                        let name_str = val["name"].as_str().unwrap_or("").to_string();
                                        let device_addr =
                                            val["device_addr"].as_str().unwrap_or("").to_string();

                                        // Register this device in the namespace registry so the
                                        // accept_cb allows its sync connections.
                                        if let Ok(node_id_bytes) = hex::decode(node_id_str) {
                                            let mut id = [0u8; 32];
                                            let len = node_id_bytes.len().min(32);
                                            id[..len].copy_from_slice(&node_id_bytes[..len]);
                                            if let Ok(mut reg) = registry.write() {
                                                reg.upsert_device(org_id.clone(), id, Device {
                                                    node_id: id,
                                                    active,
                                                    role: r.clone(),
                                                    person: person.clone(),
                                                    name: name_str.clone(),
                                                });
                                                registered_count += 1;
                                            }
                                        }

                                        if active {
                                            if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                                if endpoint_addr.id != secret.public() {
                                                    let peers_vec = vec![endpoint_addr];
                                                    let res_ctrl = ctrl_doc.start_sync(peers_vec.clone()).await;
                                                    eprintln!("[heartbeat] start_sync ctrl_doc -> {}: {:?}", &node_id_str[..8], res_ctrl.is_ok());
                                                    // Only sync other docs if they're different from control_doc
                                                    if cat_doc.id() != ctrl_doc.id() {
                                                        let res_cat = cat_doc.start_sync(peers_vec.clone()).await;
                                                        eprintln!("[heartbeat] start_sync cat_doc  -> {}: {:?}", &node_id_str[..8], res_cat.is_ok());
                                                    } else {
                                                        eprintln!("[heartbeat] SKIP cat_doc (same as ctrl, id={:?})", cat_doc.id());
                                                    }
                                                    if op_doc.id() != ctrl_doc.id() {
                                                        let _ = op_doc.start_sync(peers_vec.clone()).await;
                                                    } else {
                                                        eprintln!("[heartbeat] SKIP op_doc (same as ctrl)");
                                                    }
                                                    if pay_doc.id() != ctrl_doc.id() {
                                                        let _ = pay_doc.start_sync(peers_vec.clone()).await;
                                                    } else {
                                                        eprintln!("[heartbeat] SKIP pay_doc (same as ctrl)");
                                                    }
                                                } else {
                                                    eprintln!("[heartbeat] SKIP self (peer {} is us)", &node_id_str[..8]);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            eprintln!(
                "[heartbeat] found {} members, registered {} devices in org {}",
                member_count, registered_count, org_id
            );

            // 3. Reload role grants from control doc into the registry.
            //    This is critical for second peers joining an org: the initial
            //    sync_and_populate_org_members_impl may find an empty control doc
            //    (sync hasn't completed yet), so roles are absent from the registry.
            //    Within one heartbeat cycle the control doc data arrives, and this
            //    block loads the roles into the registry so check_read_access() works.
            if !org_id.is_empty() {
                if let Ok(entries) = ctrl_doc
                    .get_many(iroh_docs::store::Query::key_prefix("roles/"))
                    .await
                {
                    let mut entries = Box::pin(entries);
                    while let Some(res) = entries.next().await {
                        if let Ok(entry) = res {
                            if let Ok(content_bytes) =
                                store.blobs().get_bytes(entry.content_hash()).await
                            {
                                if let Ok(val) =
                                    serde_json::from_slice::<serde_json::Value>(&content_bytes)
                                {
                                    let can_open: Vec<String> = val["can_open"]
                                        .as_array()
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|v| v.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let can_write: Vec<String> = val["can_write"]
                                        .as_array()
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|v| v.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default();

                                    if let Ok(key_str) = std::str::from_utf8(entry.key()) {
                                        let role_name =
                                            key_str.strip_prefix("roles/").unwrap_or(key_str);
                                        if let Ok(mut reg) = registry.write() {
                                            reg.upsert_role(
                                                org_id.clone(),
                                                role_name.to_string(),
                                                RoleGrants {
                                                    can_open,
                                                    can_write,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}
