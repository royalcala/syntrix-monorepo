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
    _state: &AppState,
    _org_name: &str,
    _filter: &AuditFilter,
    _limit: usize,
    _offset: usize,
) -> Vec<AuditEntry> {
    // Admin reads events from redb EVENT_LOG, but admin currently has no indexer.
    // For now, return empty. This will be populated when admin gains a RelationalEngine.
    Vec::new()
}
