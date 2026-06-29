use serde::{Deserialize, Serialize};
use crate::identity::AppState;

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

pub fn audit_query(
    state: &AppState,
    org_id: &str,
    filter: &AuditFilter,
    limit: usize,
    offset: usize,
) -> Vec<AuditEntry> {
    let indexer = state.indexer();
    let since = filter.since_ts.unwrap_or(0);
    let events = match indexer.query_events_since(org_id, since, limit + offset) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut results: Vec<AuditEntry> = events.into_iter().filter_map(|e| {
        let payload = e.payload.clone();
        let doc_id = payload.get("id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        if let Some(ref ent) = filter.entity { if &e.entity != ent { return None; } }
        if let Some(ref et) = filter.event_type { if &e.event_type != et { return None; } }
        if let Some(ref n) = filter.node { if &e.hlc.node != n { return None; } }
        if let Some(ref did) = filter.doc_id { if &doc_id != did { return None; } }
        if let Some(since) = filter.since_ts { if e.hlc.ts < since { return None; } }
        if let Some(until) = filter.until_ts { if e.hlc.ts > until { return None; } }

        Some(AuditEntry {
            key: e.key,
            event_type: e.event_type,
            hlc_ts: e.hlc.ts,
            schema_version: e.schema_version,
            entity: e.entity,
            doc_id,
            payload,
        })
    }).collect();

    results.sort_by(|a, b| a.hlc_ts.cmp(&b.hlc_ts));
    let offset = offset.min(results.len());
    let mut results = if offset > 0 { results.split_off(offset) } else { results };
    results.truncate(limit);
    results
}
