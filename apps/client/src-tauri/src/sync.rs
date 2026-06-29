use crate::identity::AppState;
use serde::{Deserialize, Serialize};

pub use syntrix_core::{SyncInfo, get_sync_info, start_heartbeat_with_resync};

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

/// Get the doc for a given entity name from the org.
fn entity_doc<'a>(org: &'a crate::identity::OrgState, entity: &str) -> Option<&'a iroh_docs::api::Doc> {
    // Map common prefixes to entity names
    let entity = match entity.split('.').next().unwrap_or(entity) {
        "invoice" | "invoices" => "invoices",
        "order" | "orders" => "orders",
        "product" | "products" => "products",
        "customer" | "customers" => "customers",
        "supplier" | "suppliers" => "suppliers",
        "payroll" => "payroll",
        other => other,
    };
    org.entity_docs.get(entity)
}

pub fn sync_push(state: &mut AppState, org_id: &str, batch: Vec<SyncEventEncoded>) -> anyhow::Result<()> {
    let org = state.get_org_docs(org_id).ok_or_else(|| anyhow::anyhow!("org {} not found", org_id))?;
    let author = state.author();
    let node_hex = hex::encode(state.node_id());

    for event in &batch {
        let entity = event.name.split('.').next().unwrap_or(&event.name);
        let doc = entity_doc(org, entity)
            .ok_or_else(|| anyhow::anyhow!("unknown entity in event: {}", event.name))?;

        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
        let counter = state.counter();
        let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
        let key = format!("evt:{:020}:{:08}:{}", ts, count, &node_hex[..16]);
        let value = serde_json::json!({
            "type": event.name, "hlc": {"ts":ts,"count":count,"node":&node_hex[..16]},
            "schema_version": event.schema_version,
            "payload": &event.args, "seqNum": event.seq_num, "parentSeqNum": event.parent_seq_num,
            "clientId": event.client_id, "sessionId": event.session_id,
        });
        tauri::async_runtime::block_on(doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value)?))?;

        let doc_id = event.args.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| event.args.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or(&key);

        let upcasted = crate::events::upcast_payload(entity, event.args.clone(), event.schema_version);
        let _ = state.indexer.upsert_document(org_id, entity, doc_id, &upcasted);
    }
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
