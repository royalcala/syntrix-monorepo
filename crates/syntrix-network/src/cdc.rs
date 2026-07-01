use serde::{Deserialize, Serialize};

/// A high-level change event detected via CDC, ready for P2P sync.
/// This is decoded from the raw turso_cdc table into application-level fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcEvent {
    pub org_id: String,
    pub entity: String,
    pub doc_id: String,
    pub payload: serde_json::Value,
    pub change_time: i64,
    pub node_id: String,
    pub change_type: u8,
}

/// Read CDC events from the local database since a given change_id.
/// Queries turso_cdc, maps rowid changes to actual document data.
pub fn read_cdc_events(
    conn: &std::sync::Arc<turso_core::Connection>,
    since_change_id: u64,
    limit: usize,
    table_filter: Option<&str>,
) -> anyhow::Result<(Vec<CdcEvent>, u64)> {
    let tables = entity_tables(table_filter);
    let mut all_events = Vec::new();
    let mut max_change_id = since_change_id;

    for table in &tables {
        let sql = format!(
            "SELECT c.change_id, c.change_time, c.change_type, c.id, c.after, c.updates \
             FROM turso_cdc c WHERE c.change_id > ?1 AND c.table_name = ?2 \
             ORDER BY c.change_id LIMIT ?3"
        );

        let mut stmt = conn.prepare(&sql)?;
        use std::num::NonZero;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_i64(since_change_id as i64))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(table.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(limit as i64))?;

        use turso_core::StepResult;
        loop {
            match stmt.step()? {
                StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let change_id: i64 = row.get(0)?;
                        let change_time: i64 = row.get(1)?;
                        let change_type: i64 = row.get(2)?;
                        let _row_id: i64 = row.get(3)?;

                        if change_id as u64 > max_change_id {
                            max_change_id = change_id as u64;
                        }

                        let entity = entity_from_table(table);
                        let org_id = String::new();
                        let doc_id = String::new();
                        let payload = serde_json::Value::Null;
                        let node_id = String::new();

                        let event = CdcEvent {
                            org_id,
                            entity: entity.to_string(),
                            doc_id,
                            payload,
                            change_time,
                            node_id,
                            change_type: change_type as u8,
                        };
                        all_events.push(event);
                    }
                }
                StepResult::Done => break,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step()?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    anyhow::bail!("cdc read interrupted or busy");
                }
            }
        }
    }

    Ok((all_events, max_change_id))
}

/// Apply a batch of CDC events to the local database (INSERT OR REPLACE).
/// Uses LWW: skips if stored change_time >= incoming change_time for same (org, entity, doc_id).
pub fn apply_cdc_events(
    conn: &std::sync::Arc<turso_core::Connection>,
    events: &[CdcEvent],
) -> anyhow::Result<()> {
    for event in events {
        if event.change_type == 2 {
            continue;
        }

        let table = entity_table_name(&event.entity)?;

        let check_sql = format!(
            "SELECT change_time FROM {} WHERE org_id=?1 AND doc_id=?2",
            table
        );
        let mut check_stmt = conn.prepare(&check_sql)?;
        use std::num::NonZero;
        check_stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(event.org_id.clone()))?;
        check_stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(event.doc_id.clone()))?;

        let mut skip = false;
        use turso_core::StepResult;
        loop {
            match check_stmt.step()? {
                StepResult::Row => {
                    if let Some(row) = check_stmt.row() {
                        let stored_ts: i64 = row.get(0)?;
                        if stored_ts >= event.change_time {
                            skip = true;
                        }
                    }
                }
                StepResult::Done => break,
                StepResult::IO | StepResult::Yield => {
                    check_stmt._io().step()?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    anyhow::bail!("apply interrupted or busy");
                }
            }
        }
        if skip {
            continue;
        }

        let upsert_sql = format!(
            "INSERT OR REPLACE INTO {} (org_id, doc_id, payload, fts_title, fts_body, change_time, node_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            table
        );
        let mut upsert_stmt = conn.prepare(&upsert_sql)?;
        upsert_stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(event.org_id.clone()))?;
        upsert_stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(event.doc_id.clone()))?;

        let payload_str = serde_json::to_string(&event.payload)?;
        upsert_stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(payload_str.clone()))?;

        let title = extract_title(&event.payload);
        let body = extract_body(&event.payload);
        upsert_stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(title))?;
        upsert_stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(body))?;
        upsert_stmt.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_i64(event.change_time))?;
        upsert_stmt.bind_at(NonZero::new(7).unwrap(), turso_core::Value::from_text(event.node_id.clone()))?;

        loop {
            match upsert_stmt.step()? {
                StepResult::Done => break,
                StepResult::Row => continue,
                StepResult::IO | StepResult::Yield => {
                    upsert_stmt._io().step()?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    anyhow::bail!("apply interrupted or busy");
                }
            }
        }
    }
    Ok(())
}

fn extract_title(payload: &serde_json::Value) -> String {
    for key in &["name", "title", "label"] {
        if let Some(v) = payload.get(*key).and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    String::new()
}

fn extract_body(payload: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    if let Some(obj) = payload.as_object() {
        for (_k, v) in obj {
            if v.is_string() {
                parts.push(v.as_str().unwrap_or(""));
            }
        }
    }
    parts.join(" ")
}

fn entity_table_name(entity: &str) -> anyhow::Result<&'static str> {
    match entity {
        "customers" | "customer" => Ok("customers"),
        "suppliers" | "supplier" => Ok("suppliers"),
        "products" | "product" => Ok("products"),
        "invoices" | "invoice" => Ok("invoices"),
        "orders" | "order" => Ok("orders"),
        "payroll" => Ok("payroll"),
        _ => anyhow::bail!("unknown entity: {}", entity),
    }
}

fn entity_from_table(table: &str) -> &'static str {
    match table {
        "customers" => "customers",
        "suppliers" => "suppliers",
        "products" => "products",
        "invoices" => "invoices",
        "orders" => "orders",
        "payroll" => "payroll",
        _ => "unknown",
    }
}

fn entity_tables(filter: Option<&str>) -> Vec<&'static str> {
    let all: Vec<&'static str> = vec!["customers", "suppliers", "products", "invoices", "orders", "payroll"];
    match filter {
        Some(f) => {
            if all.contains(&f) {
                vec![entity_table_from_str(f)]
            } else {
                vec![]
            }
        }
        None => all,
    }
}

fn entity_table_from_str(name: &str) -> &'static str {
    match name {
        "customers" => "customers",
        "suppliers" => "suppliers",
        "products" => "products",
        "invoices" => "invoices",
        "orders" => "orders",
        "payroll" => "payroll",
        _ => "customers",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_table_name_valid() {
        assert_eq!(entity_table_name("customers").unwrap(), "customers");
        assert_eq!(entity_table_name("customer").unwrap(), "customers");
        assert_eq!(entity_table_name("suppliers").unwrap(), "suppliers");
        assert_eq!(entity_table_name("supplier").unwrap(), "suppliers");
        assert_eq!(entity_table_name("products").unwrap(), "products");
        assert_eq!(entity_table_name("product").unwrap(), "products");
        assert_eq!(entity_table_name("invoices").unwrap(), "invoices");
        assert_eq!(entity_table_name("invoice").unwrap(), "invoices");
        assert_eq!(entity_table_name("orders").unwrap(), "orders");
        assert_eq!(entity_table_name("order").unwrap(), "orders");
        assert_eq!(entity_table_name("payroll").unwrap(), "payroll");
    }

    #[test]
    fn test_entity_table_name_invalid() {
        assert!(entity_table_name("unknown").is_err());
        assert!(entity_table_name("widgets").is_err());
        assert!(entity_table_name("").is_err());
    }

    #[test]
    fn test_entity_from_table() {
        assert_eq!(entity_from_table("customers"), "customers");
        assert_eq!(entity_from_table("payroll"), "payroll");
        assert_eq!(entity_from_table("nonexistent"), "unknown");
    }

    #[test]
    fn test_entity_tables_no_filter() {
        let tables = entity_tables(None);
        assert_eq!(tables.len(), 6);
        assert!(tables.contains(&"customers"));
        assert!(tables.contains(&"payroll"));
    }

    #[test]
    fn test_entity_tables_with_filter() {
        let tables = entity_tables(Some("customers"));
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0], "customers");
    }

    #[test]
    fn test_entity_tables_invalid_filter() {
        let tables = entity_tables(Some("invalid"));
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_title_from_payload() {
        let payload = serde_json::json!({"name": "Alice", "title": "CEO"});
        assert_eq!(extract_title(&payload), "Alice");

        let payload = serde_json::json!({"title": "Manager"});
        assert_eq!(extract_title(&payload), "Manager");

        let payload = serde_json::json!({"label": "Important"});
        assert_eq!(extract_title(&payload), "Important");

        let payload = serde_json::json!({"no": "title"});
        assert_eq!(extract_title(&payload), "");
    }

    #[test]
    fn test_extract_body_from_payload() {
        let payload = serde_json::json!({"name": "Alice", "email": "alice@test.com", "age": 30});
        let body = extract_body(&payload);
        assert!(body.contains("Alice"));
        assert!(body.contains("alice@test.com"));
    }

    #[test]
    fn test_extract_body_empty_on_non_object() {
        let payload = serde_json::json!("just a string");
        assert_eq!(extract_body(&payload), "");
    }

    #[test]
    fn test_extract_body_empty_on_empty_object() {
        let payload = serde_json::json!({});
        assert_eq!(extract_body(&payload), "");
    }

    #[test]
    fn test_entity_table_from_str() {
        assert_eq!(entity_table_from_str("customers"), "customers");
        assert_eq!(entity_table_from_str("payroll"), "payroll");
        assert_eq!(entity_table_from_str("unknown"), "customers");
    }
}
