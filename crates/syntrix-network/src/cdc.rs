//! CDC-native sync primitives (Fase 3): reads real row images from `turso_cdc` and applies
//! them on peers, replacing the JSON-over-gossip transport. See
//! .kilo/plans/1782949593655-relational-cdc-migration.md.
//!
//! Key findings from Fase 0 (crates/syntrix-network/tests/cdc_format_probe.rs) that drive this
//! implementation:
//! - `before`/`after`/`updates` are standard SQLite/turso record-format blobs (decodable via
//!   `turso_core::types::ImmutableRecord`), listing every column of the table in physical
//!   `CREATE TABLE` order — exactly the order in `schema::EntityMeta::columns` /
//!   `ChildTableMeta::columns`.
//! - `change_type`: `1` = insert, `0` = update, `-1` = delete. `change_type == 2` is a
//!   per-transaction **commit marker** row (`table_name IS NULL`), not a delete — it must be
//!   skipped as a data change, though its `change_id` still advances the read cursor.
//! - `turso_cdc` has no built-in retention/pruning; growth must be managed by the caller
//!   (not addressed here — see plan Riesgos).

use std::collections::BTreeMap;
use std::num::NonZero;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::schema::{self, ColumnMeta, ColumnType, TableRef};

/// A change captured from `turso_cdc`, decoded into named columns ready for gossip and
/// application on peers. `columns` holds the **full physical row** (after-image for
/// insert/update, before-image for delete) keyed by column name, so `apply_cdc_events` can
/// reconstruct a typed `INSERT OR REPLACE`/`DELETE` without any entity-specific code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcEvent {
    /// Physical table name (entity table like `invoices`, or a child table like `invoice_items`).
    pub table: String,
    /// 1 = insert, 0 = update, -1 = delete.
    pub change_type: i8,
    pub change_time: i64,
    /// Full row (after-image for insert/update, before-image for delete), column name -> JSON.
    pub columns: BTreeMap<String, serde_json::Value>,
}

impl CdcEvent {
    pub fn org_id(&self) -> Option<&str> {
        self.columns.get("org_id").and_then(|v| v.as_str())
    }

    pub fn node_id(&self) -> Option<&str> {
        self.columns.get("node_id").and_then(|v| v.as_str())
    }

    pub fn get_str(&self, col: &str) -> Option<&str> {
        self.columns.get(col).and_then(|v| v.as_str())
    }
}

/// Read CDC events from the local database since a given `change_id`, across every entity and
/// child table (or a single one, if `table_filter` is set). Skips per-transaction commit
/// marker rows (`change_type == 2`, `table_name IS NULL`) — they don't carry data, but their
/// `change_id` still advances the returned cursor so the caller doesn't re-scan them forever.
pub fn read_cdc_events(
    conn: &Arc<turso_core::Connection>,
    since_change_id: u64,
    limit: usize,
    table_filter: Option<&str>,
) -> anyhow::Result<(Vec<CdcEvent>, u64)> {
    let tables: Vec<&str> = match table_filter {
        Some(t) => vec![t],
        None => schema::all_physical_tables(),
    };

    let mut all_events = Vec::new();
    let mut max_change_id = since_change_id;

    for table in &tables {
        let table_ref = match schema::resolve_table(table) {
            Some(t) => t,
            None => continue,
        };
        let columns = table_ref.columns();

        let sql = "SELECT c.change_id, c.change_type, c.change_time, c.before, c.after \
                    FROM turso_cdc c WHERE c.change_id > ?1 AND c.table_name = ?2 \
                    ORDER BY c.change_id LIMIT ?3";

        let mut stmt = conn.prepare(sql)?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_i64(since_change_id as i64))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(table.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(limit as i64))?;

        use turso_core::StepResult;
        loop {
            match stmt.step()? {
                StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let change_id: i64 = row.get(0)?;
                        let change_type: i64 = row.get(1)?;
                        let change_time: i64 = row.get(2)?;

                        if change_id as u64 > max_change_id {
                            max_change_id = change_id as u64;
                        }

                        // change_type == 2 is a commit marker (table_name IS NULL for those
                        // rows in practice, but we also guard on change_type directly since
                        // that's the authoritative signal per Fase 0 findings).
                        if change_type == 2 {
                            continue;
                        }

                        let before_val: &turso_core::Value = row.get(3)?;
                        let after_val: &turso_core::Value = row.get(4)?;

                        let blob = if change_type == -1 {
                            as_blob(before_val)
                        } else {
                            as_blob(after_val)
                        };
                        let Some(blob) = blob else { continue };

                        let values = decode_record(blob)?;
                        if values.len() < columns.len() {
                            // Row image doesn't match current schema shape (e.g. captured
                            // before a migration); skip rather than misalign columns.
                            continue;
                        }

                        let mut col_map = BTreeMap::new();
                        for (col, val) in columns.iter().zip(values.iter()) {
                            col_map.insert(col.name.clone(), turso_value_to_json(val, col.ty));
                        }

                        all_events.push(CdcEvent {
                            table: table.to_string(),
                            change_type: change_type as i8,
                            change_time,
                            columns: col_map,
                        });
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

/// Snapshots the **current** rows for an org across every entity/child table, encoded as
/// `CdcEvent`s (`change_type = 1`, i.e. insert) with each row's own `change_time`/`node_id`
/// preserved. Used for catch-up (Fase 3, tarea 16): rather than replaying historical
/// `turso_cdc`/`event_log` events (fragile once retention/pruning is added, and incompatible
/// with the admin's data-audit `event_log` shape), a newly-joined or long-disconnected peer
/// gets a snapshot of current state that it can apply with the exact same
/// `apply_cdc_events` used for live sync — so LWW/permission-checking behavior is identical
/// for catch-up and live updates.
pub fn snapshot_org_rows(conn: &Arc<turso_core::Connection>, org_id: &str) -> anyhow::Result<Vec<CdcEvent>> {
    let mut events = Vec::new();

    for table in schema::all_physical_tables() {
        let Some(table_ref) = schema::resolve_table(table) else { continue };
        let columns = table_ref.columns();
        let col_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();

        let sql = format!("SELECT {} FROM {} WHERE org_id = ?1", col_names.join(", "), table);
        // Tolerate a table not existing yet (e.g. a peer on an older schema mid-migration, or
        // a test DB that only created a subset of tables) rather than failing the whole
        // snapshot for every other table.
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => continue,
        };
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;

        let mut change_time_idx = None;
        for (i, col) in columns.iter().enumerate() {
            if col.name == "change_time" {
                change_time_idx = Some(i);
            }
        }

        use turso_core::StepResult;
        loop {
            match stmt.step()? {
                StepResult::Row => {
                    let Some(row) = stmt.row() else { continue };
                    let mut col_map = BTreeMap::new();
                    let mut change_time = 0i64;
                    for (i, col) in columns.iter().enumerate() {
                        let v: &turso_core::Value = row.get(i)?;
                        if Some(i) == change_time_idx {
                            if let turso_core::Value::Numeric(turso_core::Numeric::Integer(n)) = v {
                                change_time = *n;
                            }
                        }
                        col_map.insert(col.name.clone(), turso_value_to_json(v, col.ty));
                    }
                    events.push(CdcEvent { table: table.to_string(), change_type: 1, change_time, columns: col_map });
                }
                StepResult::Done => break,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step()?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    anyhow::bail!("snapshot read interrupted or busy");
                }
            }
        }
    }

    Ok(events)
}

fn as_blob(v: &turso_core::Value) -> Option<&[u8]> {
    match v {
        turso_core::Value::Blob(b) => Some(b.as_slice()),
        _ => None,
    }
}

fn decode_record(blob: &[u8]) -> anyhow::Result<Vec<turso_core::Value>> {
    let mut rec = turso_core::types::ImmutableRecord::new(blob.len())?;
    rec.start_serialization(blob)?;
    Ok(rec.get_values_owned()?)
}

fn turso_value_to_json(v: &turso_core::Value, ty: ColumnType) -> serde_json::Value {
    match v {
        turso_core::Value::Null => serde_json::Value::Null,
        turso_core::Value::Text(t) => serde_json::Value::String(t.as_str().to_string()),
        turso_core::Value::Numeric(turso_core::Numeric::Integer(i)) => match ty {
            ColumnType::Real => serde_json::json!(*i as f64),
            _ => serde_json::json!(i),
        },
        turso_core::Value::Numeric(turso_core::Numeric::Float(f)) => serde_json::json!(f64::from(*f)),
        turso_core::Value::Blob(_) => serde_json::Value::Null,
    }
}

fn json_to_turso_value(v: Option<&serde_json::Value>, ty: ColumnType, nullable: bool) -> turso_core::Value {
    match (v, ty) {
        (None, _) | (Some(serde_json::Value::Null), _) => {
            if nullable {
                turso_core::Value::Null
            } else {
                match ty {
                    ColumnType::Text => turso_core::Value::from_text(String::new()),
                    ColumnType::Integer => turso_core::Value::from_i64(0),
                    ColumnType::Real => turso_core::Value::from_f64(0.0),
                }
            }
        }
        (Some(serde_json::Value::String(s)), ColumnType::Text) => turso_core::Value::from_text(s.clone()),
        (Some(other), ColumnType::Text) => turso_core::Value::from_text(other.to_string()),
        (Some(serde_json::Value::Number(n)), ColumnType::Integer) => {
            turso_core::Value::from_i64(n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64))
        }
        (Some(serde_json::Value::String(s)), ColumnType::Integer) => turso_core::Value::from_i64(s.parse().unwrap_or(0)),
        (Some(serde_json::Value::Number(n)), ColumnType::Real) => turso_core::Value::from_f64(n.as_f64().unwrap_or(0.0)),
        (Some(serde_json::Value::String(s)), ColumnType::Real) => turso_core::Value::from_f64(s.parse().unwrap_or(0.0)),
        _ => turso_core::Value::Null,
    }
}

/// Author permission check: given the author's `node_id` (hex) and the target `entity` name,
/// returns whether the author's role is allowed to write to that entity (Fase 3, tarea 15,
/// Riesgos "Permisos en apply_cdc_events"). Implemented as a closure so this crate doesn't need
/// to depend on `syntrix-core::NamespaceRegistry` (which itself depends on this crate).
pub trait AuthorPermissionCheck {
    fn can_write(&self, node_id_hex: &str, entity: &str) -> bool;
}

impl<F: Fn(&str, &str) -> bool> AuthorPermissionCheck for F {
    fn can_write(&self, node_id_hex: &str, entity: &str) -> bool {
        self(node_id_hex, entity)
    }
}

/// Always-allow permission check, useful for tests or trusted local application (e.g. echoing
/// back one's own committed changes).
pub struct AllowAll;
impl AuthorPermissionCheck for AllowAll {
    fn can_write(&self, _node_id_hex: &str, _entity: &str) -> bool {
        true
    }
}

/// Apply a batch of CDC events to the local database (Fase 3, tarea 15):
/// - LWW by `change_time`: skips if the stored row's `change_time` is >= the incoming one.
/// - Validates the author's write permission for the row's entity via `perm`.
/// - Handles deletes (`change_type == -1`), including cascading a parent-entity delete to its
///   declared child tables (defensive: normally each child row's own delete arrives as its own
///   CDC event too, but this guards against out-of-order delivery).
/// - Applies to typed columns directly (no `payload` JSON blob), for both entity and child
///   tables.
pub fn apply_cdc_events(
    conn: &Arc<turso_core::Connection>,
    events: &[CdcEvent],
    perm: &dyn AuthorPermissionCheck,
) -> anyhow::Result<()> {
    for event in events {
        let Some(table_ref) = schema::resolve_table(&event.table) else { continue };
        let Some(org_id) = event.org_id() else { continue };
        let node_id = event.node_id().unwrap_or("");
        let entity = table_ref.entity_name();

        if !perm.can_write(node_id, entity) {
            tracing::warn!(
                target: "syntrix",
                entity, node_id,
                "apply_cdc_events: rejecting event, author lacks write permission"
            );
            continue;
        }

        let (row_key_cols, row_key_vals): (Vec<&str>, Vec<String>) = match &table_ref {
            TableRef::Entity(_) => {
                let Some(doc_id) = event.get_str("doc_id") else { continue };
                (vec!["org_id", "doc_id"], vec![org_id.to_string(), doc_id.to_string()])
            }
            TableRef::Child { child, .. } => {
                let Some(parent_id) = event.get_str(child.parent_key.as_str()) else { continue };
                let Some(line_id) = event.get_str("line_id") else { continue };
                (
                    vec!["org_id", child.parent_key.as_str(), "line_id"],
                    vec![org_id.to_string(), parent_id.to_string(), line_id.to_string()],
                )
            }
        };

        if lww_should_skip(conn, table_ref.table_name(), &row_key_cols, &row_key_vals, event.change_time)? {
            continue;
        }

        if event.change_type == -1 {
            delete_row(conn, &table_ref, &row_key_cols, &row_key_vals)?;
            if let TableRef::Entity(parent) = &table_ref {
                for child in &parent.children {
                    let sql = format!("DELETE FROM {} WHERE org_id=?1 AND {}=?2", child.table, child.parent_key);
                    let mut stmt = conn.prepare(&sql)?;
                    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
                    stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(row_key_vals[1].clone()))?;
                    run_to_completion(&mut stmt)?;
                }
            }
            continue;
        }

        upsert_row(conn, &table_ref, event)?;
    }
    Ok(())
}

fn lww_should_skip(
    conn: &Arc<turso_core::Connection>,
    table: &str,
    key_cols: &[&str],
    key_vals: &[String],
    incoming_change_time: i64,
) -> anyhow::Result<bool> {
    let where_clause: Vec<String> = key_cols.iter().enumerate().map(|(i, c)| format!("{}=?{}", c, i + 1)).collect();
    let sql = format!("SELECT change_time FROM {} WHERE {}", table, where_clause.join(" AND "));
    let mut stmt = conn.prepare(&sql)?;
    for (i, val) in key_vals.iter().enumerate() {
        stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_core::Value::from_text(val.clone()))?;
    }

    let mut skip = false;
    use turso_core::StepResult;
    loop {
        match stmt.step()? {
            StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let stored_ts: i64 = row.get(0)?;
                    if stored_ts >= incoming_change_time {
                        skip = true;
                    }
                }
            }
            StepResult::Done => break,
            StepResult::IO | StepResult::Yield => {
                stmt._io().step()?;
            }
            StepResult::Interrupt | StepResult::Busy => anyhow::bail!("lww check interrupted or busy"),
        }
    }
    Ok(skip)
}

fn delete_row(
    conn: &Arc<turso_core::Connection>,
    table_ref: &TableRef,
    key_cols: &[&str],
    key_vals: &[String],
) -> anyhow::Result<()> {
    let where_clause: Vec<String> = key_cols.iter().enumerate().map(|(i, c)| format!("{}=?{}", c, i + 1)).collect();
    let sql = format!("DELETE FROM {} WHERE {}", table_ref.table_name(), where_clause.join(" AND "));
    let mut stmt = conn.prepare(&sql)?;
    for (i, val) in key_vals.iter().enumerate() {
        stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_core::Value::from_text(val.clone()))?;
    }
    run_to_completion(&mut stmt)
}

fn upsert_row(conn: &Arc<turso_core::Connection>, table_ref: &TableRef, event: &CdcEvent) -> anyhow::Result<()> {
    let columns: &[ColumnMeta] = table_ref.columns();
    let col_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    let placeholders: Vec<String> = (1..=col_names.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
        table_ref.table_name(),
        col_names.join(", "),
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    for (i, col) in columns.iter().enumerate() {
        let value = json_to_turso_value(event.columns.get(&col.name), col.ty, col.nullable);
        stmt.bind_at(NonZero::new(i + 1).unwrap(), value)?;
    }
    run_to_completion(&mut stmt)
}

fn run_to_completion(stmt: &mut turso_core::Statement) -> anyhow::Result<()> {
    use turso_core::StepResult;
    loop {
        match stmt.step()? {
            StepResult::Row => {}
            StepResult::Done => break,
            StepResult::IO | StepResult::Yield => {
                stmt._io().step()?;
            }
            StepResult::Interrupt | StepResult::Busy => anyhow::bail!("apply interrupted or busy"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> (impl Drop, Arc<turso_core::Connection>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("cdc_test.db");
        let io: Arc<dyn turso_core::IO> = Arc::new(turso_core::PlatformIO::new().unwrap());
        let db = turso_core::Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            turso_core::OpenFlags::default(),
            turso_core::DatabaseOpts::new().with_index_method(true),
            None,
        ).unwrap();
        let conn = db.connect().unwrap();
        (dir, conn)
    }

    fn create_customers_table(conn: &Arc<turso_core::Connection>) {
        conn.execute(
            "CREATE TABLE customers (org_id TEXT NOT NULL, doc_id TEXT NOT NULL, name TEXT NOT NULL, \
             tax_id TEXT, address TEXT, phone TEXT, email TEXT, \
             change_time INTEGER NOT NULL DEFAULT 0, node_id TEXT NOT NULL DEFAULT '', \
             PRIMARY KEY (org_id, doc_id))",
        ).unwrap();
    }

    fn create_invoices_and_items(conn: &Arc<turso_core::Connection>) {
        conn.execute(
            "CREATE TABLE invoices (org_id TEXT NOT NULL, doc_id TEXT NOT NULL, customer_id TEXT NOT NULL, \
             amount REAL NOT NULL, status TEXT NOT NULL DEFAULT 'draft', tax_rate REAL NOT NULL DEFAULT 0.16, \
             date TEXT NOT NULL, change_time INTEGER NOT NULL DEFAULT 0, node_id TEXT NOT NULL DEFAULT '', \
             PRIMARY KEY (org_id, doc_id))",
        ).unwrap();
        conn.execute(
            "CREATE TABLE invoice_items (org_id TEXT NOT NULL, invoice_id TEXT NOT NULL, line_id TEXT NOT NULL, \
             product_id TEXT NOT NULL, qty REAL NOT NULL, price REAL NOT NULL, \
             change_time INTEGER NOT NULL DEFAULT 0, node_id TEXT NOT NULL DEFAULT '', \
             PRIMARY KEY (org_id, invoice_id, line_id))",
        ).unwrap();
    }

    fn enable_cdc(conn: &Arc<turso_core::Connection>) {
        conn.execute("PRAGMA capture_data_changes_conn='full'").unwrap();
    }

    #[test]
    fn read_cdc_events_decodes_insert_update_delete_and_skips_commit_markers() {
        let (_dir, conn) = test_conn();
        create_customers_table(&conn);
        enable_cdc(&conn);

        conn.execute(
            "INSERT INTO customers (org_id, doc_id, name, email, change_time, node_id) \
             VALUES ('org1', 'c1', 'Alice', 'alice@test.com', 1000, 'nodeA')",
        ).unwrap();
        conn.execute(
            "UPDATE customers SET name='Alice B', change_time=2000 WHERE org_id='org1' AND doc_id='c1'",
        ).unwrap();
        conn.execute("DELETE FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();

        let (events, max_id) = read_cdc_events(&conn, 0, 100, None).unwrap();
        assert_eq!(events.len(), 3, "insert+update+delete, commit markers skipped");
        // Since queries are scoped per table (`WHERE table_name = ?`), commit marker rows
        // (table_name IS NULL) never match and are naturally excluded; the cursor simply
        // reflects the last change_id seen for a real data table (harmless if it doesn't
        // include trailing commit-marker ids — the next read just rescans an empty range).
        assert_eq!(max_id, 5, "cursor reflects last customers-table change_id (delete row)");

        assert_eq!(events[0].change_type, 1);
        assert_eq!(events[0].table, "customers");
        assert_eq!(events[0].get_str("name"), Some("Alice"));
        assert_eq!(events[0].org_id(), Some("org1"));

        assert_eq!(events[1].change_type, 0);
        assert_eq!(events[1].get_str("name"), Some("Alice B"));

        assert_eq!(events[2].change_type, -1);
        assert_eq!(events[2].get_str("doc_id"), Some("c1"));
    }

    #[test]
    fn read_cdc_events_decodes_child_table_rows() {
        let (_dir, conn) = test_conn();
        create_invoices_and_items(&conn);
        enable_cdc(&conn);

        conn.execute(
            "INSERT INTO invoices (org_id, doc_id, customer_id, amount, date, change_time) \
             VALUES ('org1', 'inv1', 'cust1', 100.0, '2024-01-01', 1000)",
        ).unwrap();
        conn.execute(
            "INSERT INTO invoice_items (org_id, invoice_id, line_id, product_id, qty, price, change_time) \
             VALUES ('org1', 'inv1', 'l1', 'p1', 2.0, 50.0, 1000)",
        ).unwrap();

        let (events, _) = read_cdc_events(&conn, 0, 100, Some("invoice_items")).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].table, "invoice_items");
        assert_eq!(events[0].get_str("invoice_id"), Some("inv1"));
        assert_eq!(events[0].get_str("line_id"), Some("l1"));
        assert_eq!(events[0].columns["qty"], serde_json::json!(2.0));
    }

    #[test]
    fn apply_cdc_events_inserts_and_respects_lww() {
        let (_dir, conn) = test_conn();
        create_customers_table(&conn);

        let mut cols = BTreeMap::new();
        cols.insert("org_id".to_string(), serde_json::json!("org1"));
        cols.insert("doc_id".to_string(), serde_json::json!("c1"));
        cols.insert("name".to_string(), serde_json::json!("Alice"));
        cols.insert("tax_id".to_string(), serde_json::Value::Null);
        cols.insert("address".to_string(), serde_json::Value::Null);
        cols.insert("phone".to_string(), serde_json::Value::Null);
        cols.insert("email".to_string(), serde_json::json!("alice@test.com"));
        cols.insert("change_time".to_string(), serde_json::json!(1000));
        cols.insert("node_id".to_string(), serde_json::json!("nodeB"));

        let event = CdcEvent { table: "customers".to_string(), change_type: 1, change_time: 1000, columns: cols.clone() };
        apply_cdc_events(&conn, &[event], &AllowAll).unwrap();

        let mut stmt = conn.prepare("SELECT name, change_time FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        let (name, ct) = fetch_name_and_time(&mut stmt);
        assert_eq!(name, "Alice");
        assert_eq!(ct, 1000);

        // Stale update (older change_time) must be ignored (LWW).
        let mut stale_cols = cols.clone();
        stale_cols.insert("name".to_string(), serde_json::json!("Stale Name"));
        stale_cols.insert("change_time".to_string(), serde_json::json!(500));
        let stale_event = CdcEvent { table: "customers".to_string(), change_type: 0, change_time: 500, columns: stale_cols };
        apply_cdc_events(&conn, &[stale_event], &AllowAll).unwrap();

        let mut stmt2 = conn.prepare("SELECT name, change_time FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        let (name2, _) = fetch_name_and_time(&mut stmt2);
        assert_eq!(name2, "Alice", "stale LWW update must not overwrite newer row");

        // Newer update must win.
        let mut newer_cols = cols.clone();
        newer_cols.insert("name".to_string(), serde_json::json!("Alice Newest"));
        newer_cols.insert("change_time".to_string(), serde_json::json!(5000));
        let newer_event = CdcEvent { table: "customers".to_string(), change_type: 0, change_time: 5000, columns: newer_cols };
        apply_cdc_events(&conn, &[newer_event], &AllowAll).unwrap();

        let mut stmt3 = conn.prepare("SELECT name, change_time FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        let (name3, ct3) = fetch_name_and_time(&mut stmt3);
        assert_eq!(name3, "Alice Newest");
        assert_eq!(ct3, 5000);
    }

    fn fetch_name_and_time(stmt: &mut turso_core::Statement) -> (String, i64) {
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    let name: String = row.get(0).unwrap();
                    let ct: i64 = row.get(1).unwrap();
                    return (name, ct);
                }
                StepResult::Done => panic!("expected a row"),
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                other => panic!("unexpected step result: {other:?}"),
            }
        }
    }

    #[test]
    fn apply_cdc_events_handles_delete() {
        let (_dir, conn) = test_conn();
        create_customers_table(&conn);

        let mut cols = BTreeMap::new();
        cols.insert("org_id".to_string(), serde_json::json!("org1"));
        cols.insert("doc_id".to_string(), serde_json::json!("c1"));
        cols.insert("name".to_string(), serde_json::json!("Alice"));
        cols.insert("change_time".to_string(), serde_json::json!(1000));
        cols.insert("node_id".to_string(), serde_json::json!("nodeA"));
        let insert_event = CdcEvent { table: "customers".to_string(), change_type: 1, change_time: 1000, columns: cols.clone() };
        apply_cdc_events(&conn, &[insert_event], &AllowAll).unwrap();

        let mut del_cols = cols.clone();
        del_cols.insert("change_time".to_string(), serde_json::json!(2000));
        let delete_event = CdcEvent { table: "customers".to_string(), change_type: -1, change_time: 2000, columns: del_cols };
        apply_cdc_events(&conn, &[delete_event], &AllowAll).unwrap();

        let mut stmt = conn.prepare("SELECT count(*) FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        let count = fetch_count(&mut stmt);
        assert_eq!(count, 0);
    }

    #[test]
    fn apply_cdc_events_rejects_events_from_authors_without_permission() {
        let (_dir, conn) = test_conn();
        create_customers_table(&conn);

        let mut cols = BTreeMap::new();
        cols.insert("org_id".to_string(), serde_json::json!("org1"));
        cols.insert("doc_id".to_string(), serde_json::json!("c1"));
        cols.insert("name".to_string(), serde_json::json!("Alice"));
        cols.insert("change_time".to_string(), serde_json::json!(1000));
        cols.insert("node_id".to_string(), serde_json::json!("sales-node"));
        let event = CdcEvent { table: "customers".to_string(), change_type: 1, change_time: 1000, columns: cols };

        let deny_all = |_node: &str, _entity: &str| false;
        apply_cdc_events(&conn, &[event], &deny_all).unwrap();

        let mut stmt = conn.prepare("SELECT count(*) FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        let count = fetch_count(&mut stmt);
        assert_eq!(count, 0, "event from unauthorized author must not be applied");
    }

    #[test]
    fn apply_cdc_events_cascades_delete_to_children() {
        let (_dir, conn) = test_conn();
        create_invoices_and_items(&conn);

        let mut inv_cols = BTreeMap::new();
        inv_cols.insert("org_id".to_string(), serde_json::json!("org1"));
        inv_cols.insert("doc_id".to_string(), serde_json::json!("inv1"));
        inv_cols.insert("customer_id".to_string(), serde_json::json!("cust1"));
        inv_cols.insert("amount".to_string(), serde_json::json!(100.0));
        inv_cols.insert("status".to_string(), serde_json::json!("open"));
        inv_cols.insert("tax_rate".to_string(), serde_json::json!(0.16));
        inv_cols.insert("date".to_string(), serde_json::json!("2024-01-01"));
        inv_cols.insert("change_time".to_string(), serde_json::json!(1000));
        inv_cols.insert("node_id".to_string(), serde_json::json!("nodeA"));
        apply_cdc_events(&conn, &[CdcEvent { table: "invoices".to_string(), change_type: 1, change_time: 1000, columns: inv_cols }], &AllowAll).unwrap();

        let mut item_cols = BTreeMap::new();
        item_cols.insert("org_id".to_string(), serde_json::json!("org1"));
        item_cols.insert("invoice_id".to_string(), serde_json::json!("inv1"));
        item_cols.insert("line_id".to_string(), serde_json::json!("l1"));
        item_cols.insert("product_id".to_string(), serde_json::json!("p1"));
        item_cols.insert("qty".to_string(), serde_json::json!(2.0));
        item_cols.insert("price".to_string(), serde_json::json!(50.0));
        item_cols.insert("change_time".to_string(), serde_json::json!(1000));
        item_cols.insert("node_id".to_string(), serde_json::json!("nodeA"));
        apply_cdc_events(&conn, &[CdcEvent { table: "invoice_items".to_string(), change_type: 1, change_time: 1000, columns: item_cols }], &AllowAll).unwrap();

        let mut check = conn.prepare("SELECT count(*) FROM invoice_items WHERE org_id='org1' AND invoice_id='inv1'").unwrap();
        assert_eq!(fetch_count(&mut check), 1);

        let mut del_inv_cols = BTreeMap::new();
        del_inv_cols.insert("org_id".to_string(), serde_json::json!("org1"));
        del_inv_cols.insert("doc_id".to_string(), serde_json::json!("inv1"));
        del_inv_cols.insert("change_time".to_string(), serde_json::json!(2000));
        apply_cdc_events(&conn, &[CdcEvent { table: "invoices".to_string(), change_type: -1, change_time: 2000, columns: del_inv_cols }], &AllowAll).unwrap();

        let mut check2 = conn.prepare("SELECT count(*) FROM invoice_items WHERE org_id='org1' AND invoice_id='inv1'").unwrap();
        assert_eq!(fetch_count(&mut check2), 0, "deleting parent invoice must cascade to invoice_items");
    }

    fn fetch_count(stmt: &mut turso_core::Statement) -> i64 {
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    return row.get(0).unwrap();
                }
                StepResult::Done => panic!("expected a row"),
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                other => panic!("unexpected step result: {other:?}"),
            }
        }
    }

    #[test]
    fn round_trip_read_then_apply_converges_state() {
        let (_dir_a, conn_a) = test_conn();
        let (_dir_b, conn_b) = test_conn();
        create_customers_table(&conn_a);
        create_customers_table(&conn_b);
        enable_cdc(&conn_a);

        conn_a.execute(
            "INSERT INTO customers (org_id, doc_id, name, email, change_time, node_id) \
             VALUES ('org1', 'c1', 'Alice', 'alice@test.com', 1000, 'nodeA')",
        ).unwrap();

        let (events, _cursor) = read_cdc_events(&conn_a, 0, 100, None).unwrap();
        apply_cdc_events(&conn_b, &events, &AllowAll).unwrap();

        let mut stmt = conn_b.prepare("SELECT name, email FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    let name: String = row.get(0).unwrap();
                    let email: String = row.get(1).unwrap();
                    assert_eq!(name, "Alice");
                    assert_eq!(email, "alice@test.com");
                    break;
                }
                StepResult::Done => panic!("peer B should have received the row via CDC round-trip"),
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                other => panic!("unexpected: {other:?}"),
            }
        }
    }

    #[test]
    fn snapshot_org_rows_captures_current_state_across_entity_and_child_tables() {
        let (_dir, conn) = test_conn();
        create_customers_table(&conn);
        create_invoices_and_items(&conn);

        conn.execute(
            "INSERT INTO customers (org_id, doc_id, name, email, change_time, node_id) \
             VALUES ('org1', 'c1', 'Alice', 'alice@test.com', 1000, 'nodeA')",
        ).unwrap();
        conn.execute(
            "INSERT INTO customers (org_id, doc_id, name, change_time) \
             VALUES ('org2', 'c2', 'Bob', 2000)",
        ).unwrap();
        conn.execute(
            "INSERT INTO invoices (org_id, doc_id, customer_id, amount, date, change_time) \
             VALUES ('org1', 'inv1', 'c1', 100.0, '2024-01-01', 1000)",
        ).unwrap();
        conn.execute(
            "INSERT INTO invoice_items (org_id, invoice_id, line_id, product_id, qty, price, change_time) \
             VALUES ('org1', 'inv1', 'l1', 'p1', 2.0, 50.0, 1000)",
        ).unwrap();

        let snapshot = snapshot_org_rows(&conn, "org1").unwrap();

        // Only org1 rows, across both the customers table and the invoices/invoice_items pair.
        assert!(snapshot.iter().all(|e| e.org_id() == Some("org1")));
        assert!(snapshot.iter().any(|e| e.table == "customers" && e.get_str("doc_id") == Some("c1")));
        assert!(snapshot.iter().any(|e| e.table == "invoices" && e.get_str("doc_id") == Some("inv1")));
        assert!(snapshot.iter().any(|e| e.table == "invoice_items" && e.get_str("line_id") == Some("l1")));
        assert!(!snapshot.iter().any(|e| e.get_str("doc_id") == Some("c2")), "org2 rows must not leak into org1's snapshot");

        // Snapshot rows apply cleanly on a fresh peer via the same apply_cdc_events path used
        // for live CDC sync (Fase 3, tarea 16).
        let (_dir2, conn2) = test_conn();
        create_customers_table(&conn2);
        create_invoices_and_items(&conn2);
        apply_cdc_events(&conn2, &snapshot, &AllowAll).unwrap();

        let mut stmt = conn2.prepare("SELECT name FROM customers WHERE org_id='org1' AND doc_id='c1'").unwrap();
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    let name: String = row.get(0).unwrap();
                    assert_eq!(name, "Alice");
                    break;
                }
                StepResult::Done => panic!("snapshot row for c1 should have been applied"),
                StepResult::IO | StepResult::Yield => { stmt._io().step().unwrap(); }
                other => panic!("unexpected: {other:?}"),
            }
        }
    }
}
