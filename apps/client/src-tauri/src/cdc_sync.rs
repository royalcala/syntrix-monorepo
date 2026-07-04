//! CDC-native sync loop (Fase 3, tareas 14/17): periodically reads locally-committed changes
//! from `turso_cdc` (via `syntrix_network::cdc::read_cdc_events`) and publishes them over
//! gossipsub, replacing the old JSON-per-`commit_event` publish. Receivers apply the batch via
//! `syntrix_network::cdc::apply_cdc_events` (see `gossip.rs::process_gossip_event`).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use syntrix_network::P2PNode;

use crate::indexes::SqlEngine;

/// Gossip message envelope distinguishing CDC batches from other gossip traffic (heartbeats,
/// role/device updates) sharing the same org topic.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CdcBatchMessage {
    pub kind: CdcBatchKind,
    pub events: Vec<syntrix_network::cdc::CdcEvent>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum CdcBatchKind {
    #[serde(rename = "cdc_batch")]
    CdcBatch,
}

const CDC_READ_LIMIT: usize = 500;

/// Runs forever, scanning every org in `cdc_topics` every `interval` for new `turso_cdc` rows
/// and publishing them. `cdc_topics` is shared with `AppState` (updated on org join/startup)
/// so newly-joined orgs are picked up without restarting the loop.
pub async fn run_cdc_publish_loop(
    p2p: P2PNode,
    indexer: Arc<SqlEngine>,
    cdc_topics: Arc<RwLock<HashMap<String, String>>>,
    interval: Duration,
) {
    let local_node_id = hex::encode(p2p.local_peer_id_bytes());
    loop {
        tokio::time::sleep(interval).await;
        let orgs: Vec<(String, String)> = {
            let map = match cdc_topics.read() {
                Ok(m) => m,
                Err(_) => continue,
            };
            map.iter().map(|(o, t)| (o.clone(), t.clone())).collect()
        };

        for (org_id, topic) in orgs {
            if let Err(e) = publish_org_cdc_batch(&p2p, &indexer, &org_id, &topic, &local_node_id) {
                tracing::warn!(target: "syntrix", org = %org_id, error = %e, "cdc publish loop: failed to publish batch");
            }
        }
    }
}

fn publish_org_cdc_batch(
    p2p: &P2PNode,
    indexer: &Arc<SqlEngine>,
    org_id: &str,
    topic: &str,
    local_node_id: &str,
) -> anyhow::Result<()> {
    let cursor = indexer.get_cdc_cursor(org_id)?;
    let (events, new_cursor) = indexer.read_cdc_events(cursor, CDC_READ_LIMIT, None)?;

    // Only publish events authored by this node — events received and applied
    // from peers also generate local CDC entries (via apply_cdc_events →
    // upsert_row), but republishing them creates a feedback loop where the
    // receiving peer re-receives its own forwarded events and LWW-rejects them.
    let org_events: Vec<_> = events.into_iter()
        .filter(|e| e.org_id() == Some(org_id) && e.node_id() == Some(local_node_id))
        .collect();

    if new_cursor > cursor {
        indexer.set_cdc_cursor(org_id, new_cursor)?;
    }

    if org_events.is_empty() {
        return Ok(());
    }

    let message = CdcBatchMessage { kind: CdcBatchKind::CdcBatch, events: org_events };
    let bytes = serde_json::to_vec(&message)?;
    p2p.publish(topic, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdc_batch_message_roundtrips_through_json() {
        let mut cols = std::collections::BTreeMap::new();
        cols.insert("org_id".to_string(), serde_json::json!("org1"));
        cols.insert("doc_id".to_string(), serde_json::json!("c1"));
        let event = syntrix_network::cdc::CdcEvent {
            table: "customers".to_string(),
            change_type: 1,
            change_time: 1000,
            columns: cols,
        };
        let msg = CdcBatchMessage { kind: CdcBatchKind::CdcBatch, events: vec![event] };
        let bytes = serde_json::to_vec(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["kind"], "cdc_batch");
        assert_eq!(parsed["events"][0]["table"], "customers");

        let round_tripped: CdcBatchMessage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(round_tripped.events.len(), 1);
        assert_eq!(round_tripped.events[0].org_id(), Some("org1"));
    }
}
