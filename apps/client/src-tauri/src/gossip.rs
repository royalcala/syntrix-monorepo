use std::collections::HashMap;
use std::sync::Arc;

use futures_util::StreamExt;
use iroh_gossip::net::Gossip;
use iroh_gossip::api::{GossipTopic, Event};
use iroh_gossip::TopicId;

use crate::indexes::RelationalEngine;

pub struct GossipEventBus {
    gossip: Gossip,
    topics: HashMap<String, GossipTopic>,
}

impl GossipEventBus {
    pub fn new(gossip: Gossip) -> Self {
        Self {
            gossip,
            topics: HashMap::new(),
        }
    }

    pub async fn join_org(
        &mut self,
        org_id: &str,
        topic_id: TopicId,
        bootstrap_peers: Vec<iroh::PublicKey>,
        indexer: Arc<RelationalEngine>,
        node_id_hex: String,
    ) -> anyhow::Result<()> {
        let topic_recv = self.gossip.subscribe(topic_id, bootstrap_peers.clone()).await?;
        let topic_send = self.gossip.subscribe(topic_id, bootstrap_peers).await?;

        let org_id_clone = org_id.to_string();
        let indexer_clone = indexer.clone();
        let node_id_hex_clone = node_id_hex.clone();

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

                        let sender_node = val.get("hlc")
                            .and_then(|h| h.get("node"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("");

                        if sender_node == &node_id_hex_clone[..16] {
                            continue;
                        }

                        if let Err(e) = process_gossip_event(
                            &org_id_clone,
                            &val,
                            &indexer_clone,
                        ) {
                            eprintln!("[gossip] process error: {e}");
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("[gossip] error: {e}");
                    }
                    None => break,
                }
            }
        });

        self.topics.insert(org_id.to_string(), topic_send);
        Ok(())
    }

    pub async fn broadcast(&mut self, org_id: &str, event_bytes: bytes::Bytes) -> anyhow::Result<()> {
        match self.topics.get_mut(org_id) {
            Some(topic) => {
                topic.broadcast(event_bytes).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("no gossip topic for org {}", org_id)),
        }
    }

    pub fn org_ids(&self) -> Vec<String> {
        self.topics.keys().cloned().collect()
    }
}

fn process_gossip_event(
    org_id: &str,
    val: &serde_json::Value,
    indexer: &RelationalEngine,
) -> anyhow::Result<()> {
    let event_type = match val.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Ok(()),
    };

    let entity = entity_from_event_type(event_type);
    let payload = match val.get("payload") {
        Some(p) => p.clone(),
        None => return Ok(()),
    };

    let doc_id = payload.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    if let Some(hlc_val) = val.get("hlc") {
        let hlc = crate::indexes::HlcTimestamp {
            ts: hlc_val.get("ts").and_then(|v| v.as_u64()).unwrap_or(0),
            count: hlc_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            node: hlc_val.get("node").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };
        let _ = indexer.upsert_document_with_hlc(org_id, entity, &doc_id, &payload, &hlc);
    } else {
        let _ = indexer.upsert_document(org_id, entity, &doc_id, &payload);
    }

    let _ = indexer.append_event(org_id, val);

    Ok(())
}

fn entity_from_event_type(event_type: &str) -> &str {
    match event_type.split('.').next().unwrap_or(event_type) {
        "invoice" | "invoices" => "invoices",
        "order" | "orders" => "orders",
        "product" | "products" => "products",
        "customer" | "customers" => "customers",
        "supplier" | "suppliers" => "suppliers",
        "payroll" => "payroll",
        other => other,
    }
}
