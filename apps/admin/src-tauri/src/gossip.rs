use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use iroh_gossip::net::Gossip;
use iroh_gossip::api::{Event, GossipTopic};
use iroh_gossip::TopicId;

const EVENT_LOG: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("event_log");

/// Lightweight gossip bus for the admin node.
///
/// The admin participates in gossip topics to:
///   - receive heartbeats from client peers,
///   - broadcast its own heartbeat so clients see it online,
///   - track peer liveness for the sync-info dashboard,
///   - store data events in the admin's redb EVENT_LOG (feeds the audit panel).
pub struct AdminGossipBus {
    gossip: Gossip,
    topics: HashMap<String, GossipTopic>,
    // org_id -> (node_id_hex -> last_ts_millis)
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
    db: Arc<redb::Database>,
}

impl AdminGossipBus {
    pub fn new(gossip: Gossip, db: Arc<redb::Database>) -> Self {
        Self {
            gossip,
            topics: HashMap::new(),
            heartbeats: Arc::new(std::sync::RwLock::new(HashMap::new())),
            db,
        }
    }

    /// Return a clone of the heartbeats map so external code (e.g. `get_sync_info`)
    /// can read peer liveness without holding the gossip lock.
    pub fn heartbeats_ref(
        &self,
    ) -> Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>> {
        self.heartbeats.clone()
    }

    /// Read current heartbeats for a single org.
    pub fn get_heartbeats(&self, org_id: &str) -> HashMap<String, i64> {
        self.heartbeats
            .read()
            .unwrap()
            .get(org_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Subscribe to a gossip topic, spawn a background task that captures
    /// heartbeat messages AND data events, and keep a `GossipTopic` handle
    /// for broadcasting.
    pub async fn join_org(
        &mut self,
        org_id: &str,
        topic_id: TopicId,
    ) -> anyhow::Result<()> {
        let topic_recv = self.gossip.subscribe(topic_id, vec![]).await?;
        let topic_send = self.gossip.subscribe(topic_id, vec![]).await?;

        let org = org_id.to_string();
        let hb = self.heartbeats.clone();
        let db = self.db.clone();

        tokio::spawn(async move {
            let mut stream = topic_recv;
            loop {
                match stream.next().await {
                    Some(Ok(Event::Received(msg))) => {
                        let content = match std::str::from_utf8(&msg.content) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let val: serde_json::Value = match serde_json::from_str(content) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        // Heartbeat: no "type" field
                        if val.get("type").is_none() {
                            if let (Some(node_id), Some(ts)) = (
                                val.get("node_id").and_then(|v| v.as_str()),
                                val.get("ts").and_then(|v| v.as_i64()),
                            ) {
                                if let Ok(mut map) = hb.write() {
                                    map.entry(org.clone())
                                        .or_default()
                                        .insert(node_id.to_string(), ts);
                                }
                            }
                            continue;
                        }

                        // Data event: store in admin's EVENT_LOG for the audit panel.
                        write_event_to_redb(&db, &org, &val);
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("[admin-gossip] recv error: {e}");
                    }
                    None => break,
                }
            }
        });

        self.topics.insert(org_id.to_string(), topic_send);
        Ok(())
    }

    /// Broadcast raw bytes to all peers subscribed to the given org topic.
    pub async fn broadcast(
        &mut self,
        org_id: &str,
        event_bytes: bytes::Bytes,
    ) {
        if let Some(topic) = self.topics.get_mut(org_id) {
            let _ = topic.broadcast(event_bytes).await;
        }
    }

    /// List org-ids that we are currently subscribed to.
    pub fn org_ids(&self) -> Vec<String> {
        self.topics.keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Store a gossip data event into the admin's redb EVENT_LOG table.
// ---------------------------------------------------------------------------

fn write_event_to_redb(db: &redb::Database, org_id: &str, val: &serde_json::Value) {
    let event_type = val["type"].as_str().unwrap_or("unknown");
    let hlc_val = &val["hlc"];
    let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
    let hlc_count = hlc_val["count"].as_u64().unwrap_or(0);
    let hlc_node = hlc_val["node"].as_str().unwrap_or("");

    let key = format!(
        "evt:{}:{:020}:{:08}:{}",
        org_id, hlc_ts, hlc_count, hlc_node
    );

    let write_result = (|| -> anyhow::Result<()> {
        let write_txn = db.begin_write()?;
        {
            let mut event_log = write_txn.open_table(EVENT_LOG)?;
            event_log.insert(key.as_str(), serde_json::to_vec(val)?.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        eprintln!(
            "[admin-gossip] failed to write event {} to EVENT_LOG: {e}",
            event_type
        );
    } else {
        tracing::info!(
            target: "syntrix",
            org = %org_id,
            event_type = %event_type,
            "admin-audit: stored gossip event in EVENT_LOG"
        );
    }
}
