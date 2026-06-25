//! Heartbeat and peer re-sync loop for Syntrix P2P nodes.
//!
//! Every active node (admin or client) runs `start_heartbeat_with_resync` in the background.
//! This loop:
//!   1. Writes a timestamped heartbeat under `heartbeat/<own_node_id>` every 15 seconds.
//!   2. Re-dials all known active peers from the `members/` prefix in the control document.
//!
//! The re-dial step ensures that when a peer restarts (and gets a new network address), sync
//! sessions are automatically re-established within one heartbeat cycle (~15 s).

use futures_util::StreamExt;
use crate::addr::parse_device_addr;

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

/// Heartbeat + periodic re-sync loop.
///
/// Writes a heartbeat every 15 seconds and re-dials all known active peers from
/// the control document's `members/` prefix, re-establishing sync sessions across
/// all four organisation namespaces (control, catalogs, operational, payroll).
pub fn start_heartbeat_with_resync(
    ctrl_doc: iroh_docs::api::Doc,
    cat_doc: iroh_docs::api::Doc,
    op_doc: iroh_docs::api::Doc,
    pay_doc: iroh_docs::api::Doc,
    author: iroh_docs::AuthorId,
    node_id: String,
    store: iroh_blobs::api::Store,
    secret: iroh::SecretKey,
) {
    tokio::spawn(async move {
        loop {
            // 1. Write heartbeat
            let ts = chrono::Utc::now().timestamp_millis();
            let key = format!("heartbeat/{}", node_id);
            let val = serde_json::json!({"ts": ts, "status": "online"});
            let _ = ctrl_doc
                .set_bytes(author, key.into_bytes(), serde_json::to_vec(&val).unwrap())
                .await;

            // 2. Re-dial all known peers from control doc members/
            if let Ok(entries) = ctrl_doc
                .get_many(iroh_docs::store::Query::key_prefix("members/"))
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
                                let active = val["active"].as_bool().unwrap_or(true);
                                let device_addr =
                                    val["device_addr"].as_str().unwrap_or("").to_string();

                                if active {
                                    if let Some(endpoint_addr) = parse_device_addr(&device_addr) {
                                        if endpoint_addr.id != secret.public() {
                                            let peers_vec = vec![endpoint_addr];
                                            let _ =
                                                ctrl_doc.start_sync(peers_vec.clone()).await;
                                            let _ = cat_doc.start_sync(peers_vec.clone()).await;
                                            let _ = op_doc.start_sync(peers_vec.clone()).await;
                                            let _ = pay_doc.start_sync(peers_vec.clone()).await;
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
