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

const EVENT_LOG: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("event_log");

pub fn audit_query(
    state: &AppState,
    org_name: &str,
    filter: &AuditFilter,
    limit: usize,
    offset: usize,
) -> Vec<AuditEntry> {
    let read_txn = match state.db.begin_read() {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let event_log = match read_txn.open_table(EVENT_LOG) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let prefix = format!("evt:{}:", org_name);
    let range = match event_log.range(prefix.as_str()..) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let mut results: Vec<AuditEntry> = Vec::new();

    for item in range {
        let (key, value) = match item {
            Ok(kv) => kv,
            Err(_) => continue,
        };
        let k = key.value();
        if !k.starts_with(&prefix) {
            break;
        }

        let val: serde_json::Value = match serde_json::from_slice(value.value()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = val["type"].as_str().unwrap_or("").to_string();
        let hlc_ts = val["hlc"]["ts"].as_u64().unwrap_or(0);

        // Apply filters
        if let Some(ref f_entity) = filter.entity {
            let entity = entity_from_event_type(&event_type);
            if entity != f_entity {
                continue;
            }
        }
        if let Some(ref f_type) = filter.event_type {
            if !event_type.contains(f_type.as_str()) {
                continue;
            }
        }
        if let Some(ref f_node) = filter.node {
            let hlc_node = val["hlc"]["node"].as_str().unwrap_or("");
            if hlc_node != f_node {
                continue;
            }
        }
        if let Some(since) = filter.since_ts {
            if hlc_ts < since {
                continue;
            }
        }
        if let Some(until) = filter.until_ts {
            if hlc_ts > until {
                continue;
            }
        }
        if let Some(ref f_doc) = filter.doc_id {
            let payload = val.get("payload").cloned().unwrap_or_default();
            let doc_id = payload.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if doc_id != f_doc {
                continue;
            }
        }

        let entity = entity_from_event_type(&event_type);
        let payload = val.get("payload").cloned().unwrap_or_default();

        let doc_id = payload
            .get("id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        let schema_version = val["schema_version"].as_u64().unwrap_or(1) as u32;

        let event_type_owned = event_type.clone();

        results.push(AuditEntry {
            key: k.to_string(),
            event_type: event_type_owned,
            hlc_ts,
            schema_version,
            entity: entity.to_string(),
            doc_id,
            payload,
        });

        if results.len() >= limit + offset {
            break;
        }
    }

    // Sort by HLC timestamp descending (most recent first)
    results.sort_by(|a, b| b.hlc_ts.cmp(&a.hlc_ts));

    if offset < results.len() {
        results = results.split_off(offset);
    }
    if results.len() > limit {
        results.truncate(limit);
    }

    results
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
