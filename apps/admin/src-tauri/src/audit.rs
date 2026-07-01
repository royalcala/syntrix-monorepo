use std::num::NonZero;
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

pub fn audit_query(
    state: &AppState,
    org_name: &str,
    filter: &AuditFilter,
    limit: usize,
    offset: usize,
) -> Vec<AuditEntry> {
    let mut sql = String::from(
        "SELECT key, event_type, hlc_ts, schema_version, entity, payload FROM event_log WHERE org_id = ?1"
    );
    let mut params: Vec<(usize, turso_core::Value)> = Vec::new();
    params.push((1, turso_core::Value::from_text(org_name.to_string())));

    let mut idx = 2usize;

    if let Some(ref entity) = filter.entity {
        sql.push_str(&format!(" AND entity = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(entity.clone())));
        idx += 1;
    }
    if let Some(ref event_type) = filter.event_type {
        sql.push_str(&format!(" AND event_type LIKE ?{}", idx));
        params.push((idx, turso_core::Value::from_text(format!("%{}%", event_type))));
        idx += 1;
    }
    if let Some(ref node) = filter.node {
        sql.push_str(&format!(" AND hlc_node = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(node.clone())));
        idx += 1;
    }
    if let Some(since) = filter.since_ts {
        sql.push_str(&format!(" AND hlc_ts >= ?{}", idx));
        params.push((idx, turso_core::Value::from_i64(since as i64)));
        idx += 1;
    }
    if let Some(until) = filter.until_ts {
        sql.push_str(&format!(" AND hlc_ts <= ?{}", idx));
        params.push((idx, turso_core::Value::from_i64(until as i64)));
        idx += 1;
    }

    let has_doc_filter = filter.doc_id.is_some();
    let fetch_limit = if has_doc_filter { limit + offset + 200 } else { limit };
    let fetch_offset = if has_doc_filter { 0 } else { offset };

    sql.push_str(" ORDER BY hlc_ts DESC");
    sql.push_str(&format!(" LIMIT ?{}", idx));
    params.push((idx, turso_core::Value::from_i64(fetch_limit as i64)));
    idx += 1;
    sql.push_str(&format!(" OFFSET ?{}", idx));
    params.push((idx, turso_core::Value::from_i64(fetch_offset as i64)));

    let mut stmt = match state.db.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    for (i, val) in &params {
        if stmt.bind_at(NonZero::new(*i).unwrap(), val.clone()).is_err() {
            return vec![];
        }
    }

    let mut results: Vec<AuditEntry> = Vec::new();
    loop {
        match stmt.step() {
            Ok(turso_core::StepResult::Row) => {}
            Ok(turso_core::StepResult::Done) => break,
            Ok(turso_core::StepResult::IO | turso_core::StepResult::Yield) => {
                let _ = stmt._io().step();
                continue;
            }
            Ok(turso_core::StepResult::Interrupt | turso_core::StepResult::Busy) => {
                continue;
            }
            _ => break,
        }

        let row = match stmt.row() {
            Some(r) => r,
            None => continue,
        };

        let key: String = match row.get(0) { Ok(v) => v, Err(_) => continue };
        let event_type: String = match row.get(1) { Ok(v) => v, Err(_) => continue };
        let hlc_ts: i64 = match row.get(2) { Ok(v) => v, Err(_) => continue };
        let schema_version: i64 = match row.get(3) { Ok(v) => v, Err(_) => continue };
        let entity: String = match row.get(4) { Ok(v) => v, Err(_) => continue };
        let payload_str: String = match row.get(5) { Ok(v) => v, Err(_) => continue };
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

        let doc_id = payload
            .get("id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("node_id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        if let Some(ref f_doc) = filter.doc_id {
            if doc_id != *f_doc {
                continue;
            }
        }

        results.push(AuditEntry {
            key,
            event_type,
            hlc_ts: hlc_ts as u64,
            schema_version: schema_version as u32,
            entity,
            doc_id,
            payload,
        });
    }

    if has_doc_filter {
        results = results.into_iter().skip(offset).take(limit).collect();
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
