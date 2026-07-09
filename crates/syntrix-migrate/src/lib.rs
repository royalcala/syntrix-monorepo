use include_dir::Dir;
use serde::Deserialize;
use std::sync::Arc;
use turso_core::Connection;

#[derive(Deserialize)]
struct Journal {
    entries: Vec<JournalEntry>,
}

#[derive(Deserialize)]
struct JournalEntry {
    idx: u32,
    tag: String,
    breakpoints: bool,
}

pub fn run_migrations(conn: &Arc<Connection>, dir: &Dir<'static>, journal_json: &str) -> anyhow::Result<()> {
    conn.execute("PRAGMA foreign_keys=OFF")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS __migrations (idx INTEGER PRIMARY KEY, tag TEXT NOT NULL, applied_at INTEGER NOT NULL)",
    )?;

    let journal: Journal = serde_json::from_str(journal_json)?;

    let mut entries = journal.entries;
    entries.sort_by_key(|e| e.idx);

    let existing: Vec<String> = {
        let mut stmt = conn.prepare("SELECT tag FROM __migrations ORDER BY idx")?;
        let mut tags = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let tag: String = row.get(0)?;
                        tags.push(tag);
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("__migrations read: database busy");
                }
            }
        }
        tags
    };

    for entry in &entries {
        if existing.contains(&entry.tag) {
            continue;
        }

        let filename = format!("{}.sql", entry.tag);
        let sql_file = dir
            .get_file(&filename)
            .ok_or_else(|| anyhow::anyhow!("migration file not found: {}", filename))?;
        let sql = sql_file
            .contents_utf8()
            .ok_or_else(|| anyhow::anyhow!("migration file is not valid UTF-8: {}", filename))?;

        conn.execute("BEGIN")?;

        let result = (|| -> anyhow::Result<()> {
            if entry.breakpoints {
                for (i, stmt_text) in sql.split("--> statement-breakpoint").enumerate() {
                    let trimmed = stmt_text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    conn.execute(trimmed).map_err(|e| {
                        anyhow::anyhow!("migration {}: step {i} failed: {e}", entry.tag)
                    })?;
                }
            } else {
                conn.execute(sql).map_err(|e| {
                    anyhow::anyhow!("migration {}: execute failed: {e}", entry.tag)
                })?;
            }

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let mut stmt = conn.prepare("INSERT INTO __migrations (idx, tag, applied_at) VALUES (?1, ?2, ?3)")?;
            stmt.bind_at(
                std::num::NonZero::new(1).unwrap(),
                turso_core::Value::from_i64(entry.idx as i64),
            )?;
            stmt.bind_at(
                std::num::NonZero::new(2).unwrap(),
                turso_core::Value::from_text(entry.tag.clone()),
            )?;
            stmt.bind_at(
                std::num::NonZero::new(3).unwrap(),
                turso_core::Value::from_i64(now_ms),
            )?;

            loop {
                match stmt.step()? {
                    turso_core::StepResult::Done => break,
                    turso_core::StepResult::Row => {}
                    turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                        stmt._io().step()?;
                    }
                    turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                        anyhow::bail!("__migrations insert: database busy");
                    }
                }
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT")?;
            }
            Err(e) => {
                conn.execute("ROLLBACK")?;
                return Err(e);
            }
        }
    }

    conn.execute("PRAGMA capture_data_changes_conn='full'")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use include_dir::{include_dir, Dir};

    static FIXTURES: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/migrations");

    fn test_conn() -> (impl Drop, std::sync::Arc<turso_core::Connection>) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("migrate_test.db");
        let io: std::sync::Arc<dyn turso_core::IO> =
            std::sync::Arc::new(turso_core::PlatformIO::new().unwrap());
        let db = turso_core::Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            turso_core::OpenFlags::default(),
            turso_core::DatabaseOpts::new().with_index_method(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();
        (dir, conn)
    }

    fn count_migrations(conn: &std::sync::Arc<turso_core::Connection>) -> i64 {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM __migrations").unwrap();
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    return stmt.row().unwrap().get::<i64>(0).unwrap();
                }
                StepResult::Done => return 0,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                StepResult::Interrupt | StepResult::Busy => {
                    panic!("database busy");
                }
            }
        }
    }

    fn table_exists(conn: &std::sync::Arc<turso_core::Connection>, name: &str) -> bool {
        let sql = format!(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='{name}'"
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    return stmt.row().unwrap().get::<i64>(0).unwrap() > 0;
                }
                StepResult::Done => return false,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                StepResult::Interrupt | StepResult::Busy => {
                    panic!("database busy");
                }
            }
        }
    }

    fn is_tag_in_migrations(conn: &std::sync::Arc<turso_core::Connection>, tag: &str) -> bool {
        let sql = format!(
            "SELECT COUNT(*) FROM __migrations WHERE tag='{tag}'"
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        use turso_core::StepResult;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    return stmt.row().unwrap().get::<i64>(0).unwrap() > 0;
                }
                StepResult::Done => return false,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().unwrap();
                }
                StepResult::Interrupt | StepResult::Busy => {
                    panic!("database busy");
                }
            }
        }
    }

    #[test]
    fn fresh_db_applies_and_records() {
        let (_dir, conn) = test_conn();
        let journal = r#"{"entries":[{"idx":0,"tag":"0000_init","breakpoints":true},{"idx":1,"tag":"0001_add","breakpoints":true}]}"#;
        super::run_migrations(&conn, &FIXTURES, journal).unwrap();

        assert_eq!(count_migrations(&conn), 2);
        assert!(table_exists(&conn, "t_a"), "t_a should exist");
        assert!(table_exists(&conn, "t_b"), "t_b should exist");
    }

    #[test]
    fn second_run_is_noop() {
        let (_dir, conn) = test_conn();
        let journal = r#"{"entries":[{"idx":0,"tag":"0000_init","breakpoints":true},{"idx":1,"tag":"0001_add","breakpoints":true}]}"#;
        super::run_migrations(&conn, &FIXTURES, journal).unwrap();
        assert_eq!(count_migrations(&conn), 2);

        super::run_migrations(&conn, &FIXTURES, journal).unwrap();
        assert_eq!(count_migrations(&conn), 2, "second run should not add rows");
    }

    #[test]
    fn applies_only_pending() {
        let (_dir, conn) = test_conn();
        let journal_1 = r#"{"entries":[{"idx":0,"tag":"0000_init","breakpoints":true}]}"#;
        super::run_migrations(&conn, &FIXTURES, journal_1).unwrap();
        assert_eq!(count_migrations(&conn), 1);

        let journal_2 = r#"{"entries":[{"idx":0,"tag":"0000_init","breakpoints":true},{"idx":1,"tag":"0001_add","breakpoints":true}]}"#;
        super::run_migrations(&conn, &FIXTURES, journal_2).unwrap();
        assert_eq!(count_migrations(&conn), 2, "only 0001_add should have been applied");
        assert!(table_exists(&conn, "t_b"), "t_b should now exist");
    }

    #[test]
    fn rollback_on_bad_statement() {
        let (_dir, conn) = test_conn();
        let journal = r#"{"entries":[{"idx":0,"tag":"0000_init","breakpoints":true},{"idx":1,"tag":"0001_add","breakpoints":true},{"idx":2,"tag":"0002_bad","breakpoints":true}]}"#;
        let result = super::run_migrations(&conn, &FIXTURES, journal);
        assert!(result.is_err(), "bad SQL should cause error");
        assert!(is_tag_in_migrations(&conn, "0002_bad") == false, "failed tag should not be recorded");
        assert_eq!(count_migrations(&conn), 2, "only first two entries should be recorded");
    }
}
