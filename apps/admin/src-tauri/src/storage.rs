use std::path::PathBuf;
use std::sync::Arc;
use include_dir::{include_dir, Dir};

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

static MIGRATIONS_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

pub fn run_migrations(conn: &Arc<turso_core::Connection>) -> anyhow::Result<()> {
    let journal = include_str!("../migrations/meta/_journal.json");
    syntrix_migrate::run_migrations(conn, &MIGRATIONS_DIR, journal)
}
