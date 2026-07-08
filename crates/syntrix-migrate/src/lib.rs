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
