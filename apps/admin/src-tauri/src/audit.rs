use std::num::NonZero;
use serde::{Deserialize, Serialize};
use crate::identity::AppState;

/// Data-audit entry (Decision 5): one row per record-level change (insert/update/delete),
/// reconstructed from CDC once Fase 3 lands. Until then, `write_event_to_limbo` (gossip.rs)
/// populates this from the legacy JSON business-event stream with `change_type = "write"`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub id: i64,
    pub entity: String,
    pub change_type: String,
    pub doc_id: String,
    pub row_image: serde_json::Value,
    pub change_time: i64,
    pub node_id: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub entity: Option<String>,
    pub change_type: Option<String>,
    pub node: Option<String>,
    pub doc_id: Option<String>,
    pub since_ts: Option<i64>,
    pub until_ts: Option<i64>,
}

pub fn audit_query(
    state: &AppState,
    org_name: &str,
    filter: &AuditFilter,
    limit: usize,
    offset: usize,
) -> Vec<AuditEntry> {
    let mut sql = String::from(
        "SELECT id, entity, change_type, doc_id, row_image, change_time, node_id FROM event_log WHERE org_id = ?1"
    );
    let mut params: Vec<(usize, turso_core::Value)> = Vec::new();
    params.push((1, turso_core::Value::from_text(org_name.to_string())));

    let mut idx = 2usize;

    if let Some(ref entity) = filter.entity {
        sql.push_str(&format!(" AND entity = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(entity.clone())));
        idx += 1;
    }
    if let Some(ref change_type) = filter.change_type {
        sql.push_str(&format!(" AND change_type = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(change_type.clone())));
        idx += 1;
    }
    if let Some(ref node) = filter.node {
        sql.push_str(&format!(" AND node_id = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(node.clone())));
        idx += 1;
    }
    if let Some(ref doc_id) = filter.doc_id {
        sql.push_str(&format!(" AND doc_id = ?{}", idx));
        params.push((idx, turso_core::Value::from_text(doc_id.clone())));
        idx += 1;
    }
    if let Some(since) = filter.since_ts {
        sql.push_str(&format!(" AND change_time >= ?{}", idx));
        params.push((idx, turso_core::Value::from_i64(since)));
        idx += 1;
    }
    if let Some(until) = filter.until_ts {
        sql.push_str(&format!(" AND change_time <= ?{}", idx));
        params.push((idx, turso_core::Value::from_i64(until)));
        idx += 1;
    }

    sql.push_str(" ORDER BY change_time DESC");
    sql.push_str(&format!(" LIMIT ?{}", idx));
    params.push((idx, turso_core::Value::from_i64(limit as i64)));
    idx += 1;
    sql.push_str(&format!(" OFFSET ?{}", idx));
    params.push((idx, turso_core::Value::from_i64(offset as i64)));

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

        let id: i64 = match row.get(0) { Ok(v) => v, Err(_) => continue };
        let entity: String = match row.get(1) { Ok(v) => v, Err(_) => continue };
        let change_type: String = match row.get(2) { Ok(v) => v, Err(_) => continue };
        let doc_id: String = match row.get(3) { Ok(v) => v, Err(_) => continue };
        let row_image_str: String = match row.get(4) { Ok(v) => v, Err(_) => continue };
        let change_time: i64 = match row.get(5) { Ok(v) => v, Err(_) => continue };
        let node_id: String = match row.get(6) { Ok(v) => v, Err(_) => continue };
        let row_image: serde_json::Value = serde_json::from_str(&row_image_str).unwrap_or_default();

        results.push(AuditEntry {
            id,
            entity,
            change_type,
            doc_id,
            row_image,
            change_time,
            node_id,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_filter_default() {
        let filter = AuditFilter::default();
        assert!(filter.entity.is_none());
        assert!(filter.since_ts.is_none());
        assert!(filter.until_ts.is_none());
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            id: 1,
            entity: "invoices".into(),
            change_type: "insert".into(),
            doc_id: "inv-1".into(),
            row_image: serde_json::json!({"id": "inv-1", "amount": 500}),
            change_time: 200,
            node_id: "node1".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("invoices"));
        assert!(json.contains("inv-1"));
    }
}
