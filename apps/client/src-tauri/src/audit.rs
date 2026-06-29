use serde::{Deserialize, Serialize};
use crate::identity::AppState;

/// An individual audit log entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub key: String,
    pub event_type: String,
    pub hlc_ts: u64,
    pub schema_version: u32,
    pub entity: String,
    pub doc_id: String,
    pub payload: serde_json::Value,
}

/// Filters for audit queries.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub entity: Option<String>,
    pub event_type: Option<String>,
    pub node: Option<String>,
    pub author: Option<String>,
    pub doc_id: Option<String>,
    pub since_ts: Option<u64>,
    pub until_ts: Option<u64>,
}

/// Read the event log for a given iroh-docs doc and return matching audit entries.
fn scan_doc_events(
    doc: &iroh_docs::api::Doc,
    store: &iroh_blobs::api::Store,
    filter: &AuditFilter,
) -> Vec<AuditEntry> {
    let mut results = Vec::new();

    let Ok(stream_raw) = tauri::async_runtime::block_on(
        doc.get_many(iroh_docs::store::Query::key_prefix("evt:"))
    ) else { return results; };

    let mut stream = Box::pin(stream_raw);
    use futures_util::stream::StreamExt;

    while let Some(Ok(entry)) = tauri::async_runtime::block_on(stream.next()) {
        let key = String::from_utf8_lossy(entry.key()).to_string();
        let hash = entry.content_hash();

        let Ok(bytes) = tauri::async_runtime::block_on(store.blobs().get_bytes(hash)) else { continue; };
        let Ok(val) = serde_json::from_slice::<serde_json::Value>(bytes.as_ref()) else { continue; };

        let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let hlc_ts = val.get("hlc").and_then(|h| h.get("ts")).and_then(|v| v.as_u64()).unwrap_or(0);
        let schema_version = val.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
        let payload = val.get("payload").cloned().unwrap_or_default();
        let node = val.get("hlc").and_then(|h| h.get("node")).and_then(|v| v.as_str()).unwrap_or("").to_string();

        let entity = crate::events::entity_from_event_type(&event_type).to_string();

        let doc_id = payload.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        if let Some(ref ent) = filter.entity { if &entity != ent { continue; } }
        if let Some(ref et) = filter.event_type { if &event_type != et { continue; } }
        if let Some(ref n) = filter.node { if &node != n { continue; } }
        if let Some(ref did) = filter.doc_id { if &doc_id != did { continue; } }
        if let Some(since) = filter.since_ts { if hlc_ts < since { continue; } }
        if let Some(until) = filter.until_ts { if hlc_ts > until { continue; } }

        results.push(AuditEntry {
            key,
            event_type,
            hlc_ts,
            schema_version,
            entity,
            doc_id,
            payload,
        });
    }

    results.sort_by(|a, b| a.hlc_ts.cmp(&b.hlc_ts));
    results
}

/// Audit query: scan the org's entity docs for matching events.
pub fn audit_query(
    state: &AppState,
    org_id: &str,
    filter: &AuditFilter,
    limit: usize,
    offset: usize,
) -> Vec<AuditEntry> {
    let org = match state.get_org_docs(org_id) {
        Some(o) => o,
        None => return vec![],
    };
    let store = state.store().clone();

    let mut results = Vec::new();

    for (_ns, doc) in &org.entity_docs {
        let mut entries = scan_doc_events(doc, &store, filter);
        results.append(&mut entries);
    }

    results.sort_by(|a, b| a.hlc_ts.cmp(&b.hlc_ts));

    let offset = offset.min(results.len());
    let mut results = if offset > 0 { results.split_off(offset) } else { results };
    results.truncate(limit);
    results
}
