use std::path::PathBuf;
use std::sync::Arc;

pub fn open_limbo(data_dir: &PathBuf) -> anyhow::Result<Arc<turso_core::Connection>> {
    use turso_core::IO;
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join("syntrix-admin.db");
    let io: Arc<dyn IO> = Arc::new(turso_core::PlatformIO::new()?);
    let db = turso_core::Database::open_file_with_flags(
        io,
        db_path.to_str().unwrap(),
        turso_core::OpenFlags::default(),
        turso_core::DatabaseOpts::new()
            .with_index_method(true),
        None,
    )?;
    let conn = db.connect()?;
    Ok(conn)
}

pub fn run_migrations(conn: &Arc<turso_core::Connection>) -> anyhow::Result<()> {
    let sql = include_str!("../migrations/0000_unique_imperial_guard.sql");
    // Execute migration SQL — if tables already exist from a previous run,
    // the error is benign and we continue. Drizzle-generated SQL doesn't use
    // IF NOT EXISTS.
    if let Err(e) = conn.execute(sql) {
        let msg = e.to_string();
        if !msg.contains("already exists") {
            anyhow::bail!("migration failed: {e}");
        }
    }
    conn.execute("PRAGMA capture_data_changes_conn='full'")?;
    Ok(())
}
