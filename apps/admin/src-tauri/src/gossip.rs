use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use iroh_gossip::net::Gossip;
use iroh_gossip::api::{Event, GossipTopic};
use iroh_gossip::TopicId;

/// Lightweight gossip bus for the admin node.
///
/// The admin participates in gossip topics to:
///   - receive heartbeats from client peers,
///   - broadcast its own heartbeat so clients see it online,
///   - track peer liveness for the sync-info dashboard.
///
/// Unlike the client, the admin does NOT process data events —
/// it only cares about heartbeat messages (`{"ts":...,"node_id":...,"status":"online"}`).
pub struct AdminGossipBus {
    gossip: Gossip,
    topics: HashMap<String, GossipTopic>,
    // org_id -> (node_id_hex -> last_ts_millis)
    heartbeats: Arc<std::sync::RwLock<HashMap<String, HashMap<String, i64>>>>,
}

impl AdminGossipBus {
    pub fn new(gossip: Gossip) -> Self {
        Self {
            gossip,
            topics: HashMap::new(),
            heartbeats: Arc::new(std::sync::RwLock::new(HashMap::new())),
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
    /// heartbeat messages, and keep a `GossipTopic` handle for broadcasting.
    pub async fn join_org(
        &mut self,
        org_id: &str,
        topic_id: TopicId,
    ) -> anyhow::Result<()> {
        // One subscription for the background receiver stream …
        let topic_recv = self.gossip.subscribe(topic_id, vec![]).await?;
        // … and a second one for broadcasting (they cannot share the same handle).
        let topic_send = self.gossip.subscribe(topic_id, vec![]).await?;

        let org = org_id.to_string();
        let hb = self.heartbeats.clone();

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

                        // Admin only tracks heartbeats (messages without a "type" field).
                        if val.get("type").is_some() {
                            continue;
                        }

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
