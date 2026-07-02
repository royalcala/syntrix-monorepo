//! Read-only SQL console for the admin app (Fase 4, tarea 22): lets an operator run ad-hoc
//! `SELECT` queries against the admin's relational replica, with pagination, plus save/manage
//! favorite queries ("saved views") — including the built-in Audit Trail / Logs views that
//! replace bespoke UI code with a saved view over `event_log` (tarea 23).

use std::num::NonZero;

use crate::identity::AppState;

#[derive(Debug, serde::Serialize)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub truncated: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SavedView {
    pub id: String,
    pub name: String,
    pub sql_query: String,
    pub created_at: i64,
    pub updated_at: i64,
}

const MAX_ROWS: usize = 1000;

/// Validates that `query` is a single, read-only statement before executing it. This is a
/// lightweight lexical guard (not a full SQL parser) appropriate for an internal admin tool:
/// rejects anything that isn't a bare `SELECT`/`WITH` statement, and rejects statement
/// separators (`;` other than a single trailing one) to prevent stacked queries.
fn validate_readonly_query(query: &str) -> anyhow::Result<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        anyhow::bail!("query must not be empty");
    }

    let body = trimmed.strip_suffix(';').unwrap_or(trimmed).trim();
    if body.contains(';') {
        anyhow::bail!("only a single statement is allowed");
    }

    let lower = body.to_ascii_lowercase();
    let first_word = lower.split_whitespace().next().unwrap_or("");
    if first_word != "select" && first_word != "with" {
        anyhow::bail!("only SELECT statements are allowed (got: {first_word})");
    }

    const FORBIDDEN: &[&str] = &[
        "insert", "update", "delete", "drop", "alter", "create", "attach", "detach", "pragma",
        "vacuum", "replace",
    ];
    for word in lower.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if FORBIDDEN.contains(&word) || word.starts_with("pragma_") {
            anyhow::bail!("statement contains forbidden keyword: {word}");
        }
    }

    Ok(body.to_string())
}

pub fn run_sql_impl(state: &AppState, query: &str, limit: usize, offset: usize) -> anyhow::Result<SqlResult> {
    let body = validate_readonly_query(query)?;
    let capped_limit = limit.min(MAX_ROWS).max(1);

    let paginated = format!("SELECT * FROM ({body}) LIMIT ?1 OFFSET ?2");
    let mut stmt = state.db.prepare(&paginated)?;
    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_i64((capped_limit + 1) as i64))?;
    stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_i64(offset as i64))?;

    let columns: Vec<String> = (0..stmt.num_columns()).map(|i| stmt.get_column_name(i).to_string()).collect();
    let mut rows = Vec::new();

    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let mut values = Vec::with_capacity(columns.len());
                    for i in 0..columns.len() {
                        let v: &turso_core::Value = row.get(i)?;
                        values.push(value_to_json(v));
                    }
                    rows.push(values);
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("run_sql: database busy or interrupted");
            }
        }
    }

    let truncated = rows.len() > capped_limit;
    rows.truncate(capped_limit);

    Ok(SqlResult { columns, rows, truncated })
}

fn value_to_json(v: &turso_core::Value) -> serde_json::Value {
    match v {
        turso_core::Value::Null => serde_json::Value::Null,
        turso_core::Value::Text(t) => serde_json::Value::String(t.as_str().to_string()),
        turso_core::Value::Numeric(turso_core::Numeric::Integer(i)) => serde_json::json!(i),
        turso_core::Value::Numeric(turso_core::Numeric::Float(f)) => serde_json::json!(f64::from(*f)),
        turso_core::Value::Blob(b) => serde_json::json!(format!("<blob:{} bytes>", b.len())),
    }
}

fn run_to_completion(stmt: &mut turso_core::Statement) -> anyhow::Result<()> {
    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {}
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("database busy or interrupted");
            }
        }
    }
    Ok(())
}

pub fn list_saved_views_impl(state: &AppState) -> anyhow::Result<Vec<SavedView>> {
    let mut stmt = state.db.prepare("SELECT id, name, sql_query, created_at, updated_at FROM saved_views ORDER BY name")?;
    let mut views = Vec::new();
    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    views.push(SavedView {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        sql_query: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    });
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("list_saved_views: database busy or interrupted");
            }
        }
    }
    Ok(views)
}

pub fn create_saved_view_impl(state: &AppState, name: &str, sql_query: &str) -> anyhow::Result<SavedView> {
    // A saved view's query is only ever run through `run_sql_impl` (read-only enforced
    // there), but validate at save time too so invalid views can't be created silently.
    validate_readonly_query(sql_query)?;

    let id = uuid_v4_like();
    let now = current_millis();
    let mut stmt = state.db.prepare(
        "INSERT INTO saved_views (id, name, sql_query, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
    )?;
    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(id.clone()))?;
    stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(name.to_string()))?;
    stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(sql_query.to_string()))?;
    stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_i64(now))?;
    run_to_completion(&mut stmt)?;

    Ok(SavedView { id, name: name.to_string(), sql_query: sql_query.to_string(), created_at: now, updated_at: now })
}

pub fn delete_saved_view_impl(state: &AppState, id: &str) -> anyhow::Result<()> {
    let mut stmt = state.db.prepare("DELETE FROM saved_views WHERE id=?1")?;
    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(id.to_string()))?;
    run_to_completion(&mut stmt)
}

/// Seeds the built-in "Audit Trail" / "Logs" saved views (Fase 4, tarea 23) on first run,
/// replacing bespoke `AuditTrail.tsx` query logic with a plain saved view over `event_log`.
/// Each default view is checked individually (not just "Audit Trail") so deleting one of them
/// doesn't cause the others to be silently re-created (and duplicated) on the next restart.
pub fn seed_default_saved_views(state: &AppState) -> anyhow::Result<()> {
    const DEFAULTS: &[(&str, &str)] = &[
        (
            "Audit Trail",
            "SELECT change_time, entity, change_type, doc_id, node_id, row_image FROM event_log ORDER BY change_time DESC LIMIT 200",
        ),
        (
            "Recent Customers",
            "SELECT doc_id, name, email, change_time FROM customers ORDER BY change_time DESC LIMIT 200",
        ),
    ];

    let existing = list_saved_views_impl(state)?;
    for (name, sql_query) in DEFAULTS {
        if existing.iter().any(|v| v.name == *name) {
            continue;
        }
        create_saved_view_impl(state, name, sql_query)?;
    }
    Ok(())
}

fn current_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn uuid_v4_like() -> String {
    let mut bytes = [0u8; 16];
    for b in bytes.iter_mut() {
        *b = fastrand::u8(..);
    }
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (impl Drop, AppState) {
        let (dir, path) = syntrix_testkit::temp_node_dir("sql_console_test");
        let state = tauri::async_runtime::block_on(AppState::new_with_data_dir(path)).expect("state");
        (dir, state)
    }

    #[test]
    fn validate_readonly_query_accepts_select_and_with() {
        assert!(validate_readonly_query("SELECT * FROM customers").is_ok());
        assert!(validate_readonly_query("  select id from invoices  ").is_ok());
        assert!(validate_readonly_query("WITH x AS (SELECT 1) SELECT * FROM x").is_ok());
    }

    #[test]
    fn validate_readonly_query_rejects_mutations_and_stacked_statements() {
        assert!(validate_readonly_query("DELETE FROM customers").is_err());
        assert!(validate_readonly_query("INSERT INTO customers (doc_id) VALUES ('x')").is_err());
        assert!(validate_readonly_query("PRAGMA table_info(customers)").is_err());
        assert!(validate_readonly_query("SELECT * FROM customers; DELETE FROM customers").is_err());
        assert!(validate_readonly_query("").is_err());
    }

    #[test]
    fn validate_readonly_query_rejects_pragma_table_valued_functions() {
        // `pragma_*` table-valued functions (e.g. `pragma_table_info`) are a distinct lexical
        // token from the bare word `pragma`, so they must be rejected explicitly rather than
        // relying on the FORBIDDEN word list matching them.
        assert!(validate_readonly_query("SELECT * FROM pragma_table_info('customers')").is_err());
        assert!(validate_readonly_query("SELECT * FROM pragma_wal_checkpoint(2)").is_err());
    }

    #[test]
    fn run_sql_impl_executes_select_and_paginates() {
        let (_dir, state) = test_state();
        for i in 0..5 {
            let mut stmt = state.db.prepare(
                "INSERT INTO customers (org_id, doc_id, name) VALUES ('org1', ?1, ?2)",
            ).unwrap();
            stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(format!("c{i}"))).unwrap();
            stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(format!("Customer {i}"))).unwrap();
            run_to_completion(&mut stmt).unwrap();
        }

        let result = run_sql_impl(&state, "SELECT doc_id, name FROM customers ORDER BY doc_id", 3, 0).unwrap();
        assert_eq!(result.columns, vec!["doc_id", "name"]);
        assert_eq!(result.rows.len(), 3);
        assert!(result.truncated);

        let result2 = run_sql_impl(&state, "SELECT doc_id, name FROM customers ORDER BY doc_id", 10, 0).unwrap();
        assert_eq!(result2.rows.len(), 5);
        assert!(!result2.truncated);
    }

    #[test]
    fn run_sql_impl_rejects_write_statements() {
        let (_dir, state) = test_state();
        assert!(run_sql_impl(&state, "DELETE FROM customers", 10, 0).is_err());
        assert!(run_sql_impl(&state, "PRAGMA capture_data_changes_conn='off'", 10, 0).is_err());
    }

    #[test]
    fn saved_views_crud_roundtrip() {
        let (_dir, state) = test_state();
        let created = create_saved_view_impl(&state, "My View", "SELECT * FROM customers").unwrap();
        let listed = list_saved_views_impl(&state).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        delete_saved_view_impl(&state, &created.id).unwrap();
        assert!(list_saved_views_impl(&state).unwrap().is_empty());
    }

    #[test]
    fn seed_default_saved_views_is_idempotent() {
        let (_dir, state) = test_state();
        seed_default_saved_views(&state).unwrap();
        seed_default_saved_views(&state).unwrap();
        let views = list_saved_views_impl(&state).unwrap();
        assert_eq!(views.iter().filter(|v| v.name == "Audit Trail").count(), 1);
    }

    #[test]
    fn seed_default_saved_views_recreates_only_the_deleted_default_without_duplicating_others() {
        let (_dir, state) = test_state();
        seed_default_saved_views(&state).unwrap();

        let views = list_saved_views_impl(&state).unwrap();
        let audit_trail = views.iter().find(|v| v.name == "Audit Trail").unwrap().clone();
        delete_saved_view_impl(&state, &audit_trail.id).unwrap();

        // Re-seeding after deleting only "Audit Trail" must not duplicate "Recent Customers".
        seed_default_saved_views(&state).unwrap();
        let views_after = list_saved_views_impl(&state).unwrap();
        assert_eq!(views_after.iter().filter(|v| v.name == "Audit Trail").count(), 1);
        assert_eq!(views_after.iter().filter(|v| v.name == "Recent Customers").count(), 1);
    }
}
