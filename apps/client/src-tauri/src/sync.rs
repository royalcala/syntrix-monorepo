use crate::identity::AppState;
use serde::{Deserialize, Serialize};

pub use syntrix_core::{SyncInfo, get_sync_info};

#[derive(Debug, Deserialize)]
pub struct SyncEventEncoded {
    pub name: String, pub args: serde_json::Value,
    #[serde(rename = "seqNum")] pub seq_num: u64,
    #[serde(rename = "parentSeqNum")] pub parent_seq_num: u64,
    #[serde(rename = "clientId")] pub client_id: String,
    #[serde(rename = "sessionId")] pub session_id: String,
    #[serde(rename = "schemaVersion", default = "default_schema_version")] pub schema_version: u32,
}

fn default_schema_version() -> u32 { 1 }

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
    let _org = state.get_org(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let node_hex = hex::encode(state.node_id());

    for event in &batch {
        let entity = event.name.split('.').next().unwrap_or(&event.name);
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
        let counter = state.counter();
        let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
        let key = format!("evt:{}:{:020}:{:08}:{}", org_id, ts, count, &node_hex[..16]);
        let value = serde_json::json!({
            "type": event.name, "hlc": {"ts":ts,"count":count,"node":&node_hex[..16]},
            "schema_version": event.schema_version,
            "payload": &event.args, "seqNum": event.seq_num, "parentSeqNum": event.parent_seq_num,
            "clientId": event.client_id, "sessionId": event.session_id,
        });

        let indexer = state.indexer();
        let _ = indexer.append_event(org_id, &value);

        let doc_id = event.args.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| event.args.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or(&key);

        let upcasted = crate::events::upcast_payload(entity, event.args.clone(), event.schema_version);
        let _ = indexer.upsert_document(org_id, entity, doc_id, &upcasted);
        state.live_manager().notify_table_changed(&state.conn(), &[entity.to_string()]);
    }
    Ok(())
}

pub fn sync_pull(state: &AppState, org_id: &str, cursor: Option<HlcCursor>) -> SyncPullResult {
    let since = cursor.as_ref().map(|c| c.ts).unwrap_or(0);
    let indexer = state.indexer();
    let events = indexer.query_events_since(org_id, since, 100).unwrap_or_default();

    let (batch, has_more) = if events.len() > 100 {
        let slice: Vec<_> = events[..100].iter().map(|e| SyncEntry {
            event_encoded: serde_json::json!({
                "type": e.event_type,
                "hlc": e.hlc,
                "schema_version": e.schema_version,
                "payload": e.payload,
            }),
        }).collect();
        (slice, true)
    } else {
        let all: Vec<_> = events.iter().map(|e| SyncEntry {
            event_encoded: serde_json::json!({
                "type": e.event_type,
                "hlc": e.hlc,
                "schema_version": e.schema_version,
                "payload": e.payload,
            }),
        }).collect();
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
    state.get_org(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    Ok(ConnectionState { connected: true, peers: 0 })
}

pub fn sync_status(state: &AppState) -> String {
    format!("online · {} orgs · node {}", state.list_orgs().len(), hex::encode(state.node_id())[..8].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlc_cursor_serialization() {
        let cursor = HlcCursor { ts: 1000, count: 5, node: "node1".into() };
        let json = serde_json::to_string(&cursor).unwrap();
        assert!(json.contains("\"ts\":1000"));
        assert!(json.contains("\"count\":5"));
        assert!(json.contains("\"node\":\"node1\""));
    }

    #[test]
    fn test_sync_entry_serialization() {
        let entry = SyncEntry {
            event_encoded: serde_json::json!({"type": "customer.created", "payload": {"id": "c1"}}),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("customer.created"));
    }

    #[test]
    fn test_connection_state() {
        let cs = ConnectionState { connected: true, peers: 3 };
        assert!(cs.connected);
        assert_eq!(cs.peers, 3);
    }

    #[test]
    fn test_default_schema_version() {
        assert_eq!(default_schema_version(), 1);
    }

    #[test]
    fn test_sync_pull_result_has_more() {
        let result = SyncPullResult {
            batch: vec![],
            has_more: false,
            cursor: None,
        };
        assert!(!result.has_more);
        assert!(result.cursor.is_none());
    }
}

pub async fn get_sync_info_impl(
    state: &AppState,
    org: &str,
) -> anyhow::Result<SyncInfo> {
    let members = state.indexer().get_members(org)?;
    let heartbeats = state.indexer().get_heartbeats(org)?;
    Ok(syntrix_core::sync::get_sync_info(members, heartbeats, state.node_id()))
}
