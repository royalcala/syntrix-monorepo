use std::sync::Arc;
use turso_core::Connection;

/// Runs all pending migrations via the production migration runner.
pub fn run_migrations(conn: &Arc<Connection>) {
    syntrix_client_lib::storage::run_migrations(conn).expect("run_migrations");
}

pub fn run_migration_sql(conn: &Arc<Connection>, path: &str) -> String {
    let sql = std::fs::read_to_string(path).expect("read migration file");
    for statement in sql.split("--> statement-breakpoint") {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            conn.execute(trimmed).expect("migration step");
        }
    }
    sql
}

pub fn assert_table_columns(conn: &Arc<Connection>, table: &str, expected_count: usize) {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql).unwrap();
    let mut count = 0usize;
    loop {
        match stmt.step().unwrap() {
            turso_core::StepResult::Row => { count += 1; }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => { stmt._io().step().unwrap(); }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => panic!("db busy"),
        }
    }
    assert_eq!(count, expected_count, "{table} should have {expected_count} columns");
}
