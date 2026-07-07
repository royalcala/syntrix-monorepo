mod common;

use syntrix_testkit::temp_limbo_db;
use std::sync::Arc;
use turso_core::Connection;

fn insert_row(conn: &Arc<Connection>, sql: &str, params: &[(&str, &str)]) {
    use std::num::NonZero;
    let mut stmt = conn.prepare(sql).unwrap();
    for (i, &(_name, val)) in params.iter().enumerate() {
        let idx = NonZero::new(i + 1).unwrap();
        stmt.bind_at(idx, turso_core::Value::from_text(val.to_string())).unwrap();
    }
    loop {
        match stmt.step().unwrap() {
            turso_core::StepResult::Row => {}
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => { stmt._io().step().unwrap(); }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => panic!("db busy"),
        }
    }
}

fn query_i64(conn: &Arc<Connection>, sql: &str, params: &[(&str, &str)], col: usize) -> Vec<i64> {
    use std::num::NonZero;
    let mut stmt = conn.prepare(sql).unwrap();
    for (i, &(_name, val)) in params.iter().enumerate() {
        let idx = NonZero::new(i + 1).unwrap();
        stmt.bind_at(idx, turso_core::Value::from_text(val.to_string())).unwrap();
    }
    let mut results = Vec::new();
    loop {
        match stmt.step().unwrap() {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let val: i64 = row.get(col).unwrap();
                    results.push(val);
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => { stmt._io().step().unwrap(); }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => panic!("db busy"),
        }
    }
    results
}

fn query_string(conn: &Arc<Connection>, sql: &str, params: &[(&str, &str)], col: usize) -> Vec<String> {
    use std::num::NonZero;
    let mut stmt = conn.prepare(sql).unwrap();
    for (i, &(_name, val)) in params.iter().enumerate() {
        let idx = NonZero::new(i + 1).unwrap();
        stmt.bind_at(idx, turso_core::Value::from_text(val.to_string())).unwrap();
    }
    let mut results = Vec::new();
    loop {
        match stmt.step().unwrap() {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let val: String = row.get(col).unwrap();
                    results.push(val);
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => { stmt._io().step().unwrap(); }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => panic!("db busy"),
        }
    }
    results
}

fn query_string_opt(conn: &Arc<Connection>, sql: &str, params: &[(&str, &str)], col: usize) -> Vec<Option<String>> {
    use std::num::NonZero;
    let mut stmt = conn.prepare(sql).unwrap();
    for (i, &(_name, val)) in params.iter().enumerate() {
        let idx = NonZero::new(i + 1).unwrap();
        stmt.bind_at(idx, turso_core::Value::from_text(val.to_string())).unwrap();
    }
    let mut results = Vec::new();
    loop {
        match stmt.step().unwrap() {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let val: Option<String> = row.get(col).ok();
                    results.push(val);
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => { stmt._io().step().unwrap(); }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => panic!("db busy"),
        }
    }
    results
}

// ── view_definitions CRUD ──────────────────────────────────────────

#[test]
fn test_migration_0003_creates_view_definitions_and_ia_queries() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);

    let tables = query_string(&conn,
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('view_definitions', 'ia_queries') ORDER BY name",
        &[], 0);
    assert_eq!(tables, vec!["ia_queries", "view_definitions"]);
}

#[test]
fn test_view_definitions_insert_and_select() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);
    common::assert_table_columns(&conn, "view_definitions", 11);

    insert_row(&conn,
        "INSERT INTO view_definitions (org_id, doc_id, sql, entity, components_json, root, meta_json, created_by, tags, change_time, node_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        &[("org_id", "org-1"), ("doc_id", "v1"), ("sql", "SELECT * FROM customers"),
          ("entity", "customers"), ("components_json", r#"{"type":"table"}"#),
          ("root", "root"), ("meta_json", r#"{"query":"list"}"#),
          ("created_by", "node-1"), ("tags", "a,b"), ("change_time", "1000"),
          ("node_id", "node-1")]);

    let sql_val = query_string(&conn,
        "SELECT sql FROM view_definitions WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "v1")], 0);
    assert_eq!(sql_val, vec!["SELECT * FROM customers"]);

    let entity_val = query_string(&conn,
        "SELECT entity FROM view_definitions WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "v1")], 0);
    assert_eq!(entity_val, vec!["customers"]);
}

#[test]
fn test_view_definitions_filters_by_org_id() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);

    for (org, doc) in [("org-a", "v1"), ("org-a", "v2"), ("org-b", "v3")] {
        insert_row(&conn,
            "INSERT INTO view_definitions (org_id, doc_id, sql, entity, components_json, root, meta_json, created_by, tags, change_time, node_id) \
             VALUES (?1,?2,'SELECT 1','x','{}','root','{}','u','',0,'')",
            &[("org_id", org), ("doc_id", doc)]);
    }

    let mut docs = query_string(&conn,
        "SELECT doc_id FROM view_definitions WHERE org_id=?1 ORDER BY doc_id",
        &[("org_id", "org-a")], 0);
    docs.sort();
    assert_eq!(docs, vec!["v1", "v2"]);
}

#[test]
fn test_view_definitions_lww_columns() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);

    insert_row(&conn,
        "INSERT INTO view_definitions (org_id, doc_id, sql, entity, components_json, root, meta_json, created_by, tags, change_time, node_id) \
         VALUES (?1,?2,'SELECT 1','x','{}','root','{}','u','',1000,'node-a')",
        &[("org_id", "org-1"), ("doc_id", "v1")]);

    let change_time = query_i64(&conn,
        "SELECT change_time FROM view_definitions WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "v1")], 0);
    assert_eq!(change_time, vec![1000]);

    let node_id = query_string(&conn,
        "SELECT node_id FROM view_definitions WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "v1")], 0);
    assert_eq!(node_id, vec!["node-a"]);
}

// ── ia_queries CRUD + state transitions ───────────────────────────

#[test]
fn test_ia_query_insert_and_state_transition() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);

    insert_row(&conn,
        "INSERT INTO ia_queries (org_id, doc_id, text, status, change_time, node_id) VALUES (?1,?2,?3,'pending',1000,'node-a')",
        &[("org_id", "org-1"), ("doc_id", "q1"), ("text", "show me invoices for may")]);

    let status = query_string(&conn,
        "SELECT status FROM ia_queries WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "q1")], 0);
    assert_eq!(status, vec!["pending"]);

    // Transition: pending → completed + view_id
    insert_row(&conn,
        "UPDATE ia_queries SET status='completed', view_id=?1 WHERE org_id=?2 AND doc_id=?3",
        &[("view_id", "v-may-invoices"), ("org_id", "org-1"), ("doc_id", "q1")]);

    let status = query_string(&conn,
        "SELECT status FROM ia_queries WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "q1")], 0);
    assert_eq!(status, vec!["completed"]);

    let view_ids = query_string_opt(&conn,
        "SELECT view_id FROM ia_queries WHERE org_id=?1 AND doc_id=?2",
        &[("org_id", "org-1"), ("doc_id", "q1")], 0);
    assert_eq!(view_ids, vec![Some("v-may-invoices".to_string())]);
}

#[test]
fn test_ia_queries_index_by_org_and_status() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);

    insert_row(&conn,
        "INSERT INTO ia_queries (org_id, doc_id, text, status, change_time, node_id) VALUES (?1,?2,?3,'pending',0,'')",
        &[("org_id", "org-1"), ("doc_id", "q1"), ("text", "show invoices")]);
    insert_row(&conn,
        "INSERT INTO ia_queries (org_id, doc_id, text, status, change_time, node_id) VALUES (?1,?2,?3,'pending',0,'')",
        &[("org_id", "org-1"), ("doc_id", "q2"), ("text", "show customers")]);
    insert_row(&conn,
        "INSERT INTO ia_queries (org_id, doc_id, text, status, change_time, node_id) VALUES (?1,?2,?3,'pending',0,'')",
        &[("org_id", "org-2"), ("doc_id", "q3"), ("text", "show products")]);

    let docs = query_string(&conn,
        "SELECT doc_id FROM ia_queries WHERE org_id=?1 AND status=?2 ORDER BY doc_id",
        &[("org_id", "org-1"), ("status", "pending")], 0);
    assert_eq!(docs, vec!["q1", "q2"]);
}

// ── device_type (schema validation) ───────────────────────────────

#[test]
fn test_admin_devices_schema_includes_device_type() {
    let (_dir, conn) = temp_limbo_db();
    common::run_migration_0003(&conn);
    common::assert_table_columns(&conn, "view_definitions", 11);
    common::assert_table_columns(&conn, "ia_queries", 7);
}
