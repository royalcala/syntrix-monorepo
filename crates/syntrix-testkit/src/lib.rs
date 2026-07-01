use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct PollConfig {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self { max_retries: 30, base_delay: Duration::from_millis(200), max_delay: Duration::from_secs(5) }
    }
}

pub fn temp_node_dir(prefix: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join(prefix);
    std::fs::create_dir_all(&path).ok();
    (dir, path)
}

pub async fn poll_until<T, E, F, Fut>(mut f: F, config: &PollConfig) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut delay = config.base_delay;
    for attempt in 0..config.max_retries {
        match f().await {
            Ok(value) => return Ok(value),
            Err(_) if attempt + 1 < config.max_retries => {
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(config.max_delay);
            }
            Err(_) => return Err(format!("poll_until exhausted after {} attempts", config.max_retries)),
        }
    }
    Err("poll_until: unreachable".into())
}

pub fn parse_node_id(hex_str: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != 32 { anyhow::bail!("node_id must be 32 bytes, got {}", bytes.len()); }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Ok(id)
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TestInvitePayload {
    pub org_name: String,
    pub role: String,
    pub admin_addr: Option<String>,
    pub topic_id: String,
    pub can_open: Vec<String>,
    pub can_write: Vec<String>,
}

pub async fn wait_for_sync(timeout_secs: u64) {
    tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
}

pub fn temp_limbo_db() -> (tempfile::TempDir, Arc<turso_core::Connection>) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let io: Arc<dyn turso_core::IO> = Arc::new(turso_core::PlatformIO::new().expect("PlatformIO"));
    let db = turso_core::Database::open_file_with_flags(
        io,
        db_path.to_str().unwrap(),
        turso_core::OpenFlags::default(),
        turso_core::DatabaseOpts::new().with_index_method(true),
        None,
    )
    .expect("open limbo db");
    let conn = db.connect().expect("connect to limbo db");
    (dir, conn)
}

pub fn seed_test_document(
    conn: &Arc<turso_core::Connection>,
    entity: &str,
    org_id: &str,
    data: &serde_json::Value,
) -> anyhow::Result<String> {
    let doc_id = data["id"].as_str().map(String::from)
        .unwrap_or_else(|| format!("doc-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let payload_str = serde_json::to_string(data)?;
    let title = data.get("name").or_else(|| data.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let sql = format!(
        "INSERT OR REPLACE INTO {} (org_id, doc_id, payload, fts_title, fts_body) VALUES (?1, ?2, ?3, ?4, ?5)",
        entity
    );
    use std::num::NonZero;
    let mut stmt = conn.prepare(&sql)?;
    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
    stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.clone()))?;
    stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(payload_str.clone()))?;
    stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(title.clone()))?;
    stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(payload_str))?;
    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {}
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("seed_test_document: database busy or interrupted");
            }
        }
    }
    Ok(doc_id)
}

fn query_doc(
    conn: &Arc<turso_core::Connection>,
    sql: &str,
    org_id: &str,
    doc_id: &str,
) -> Result<serde_json::Value, String> {
    use std::num::NonZero;
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string())).map_err(|e| e.to_string())?;
    stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.to_string())).map_err(|e| e.to_string())?;
    loop {
        match stmt.step().map_err(|e| e.to_string())? {
            turso_core::StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let payload_str: String = row.get(0).map_err(|e| e.to_string())?;
                    return serde_json::from_str(&payload_str).map_err(|e| e.to_string());
                }
            }
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step().map_err(|e| e.to_string())?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                return Err("busy".to_string());
            }
        }
    }
    Err("not found".to_string())
}

pub async fn wait_for_document_limbo(
    conn: &Arc<turso_core::Connection>,
    org_id: &str,
    entity: &str,
    doc_id: &str,
) -> anyhow::Result<serde_json::Value> {
    let sql = format!("SELECT payload FROM {} WHERE org_id=?1 AND doc_id=?2", entity);
    poll_until(
        || {
            let result = query_doc(conn, &sql, org_id, doc_id);
            async move { result }
        },
        &PollConfig::default(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("wait_for_document_limbo({}): {}", doc_id, e))
}

pub async fn wait_for_event(
    conn: &Arc<turso_core::Connection>,
    org_id: &str,
    key: &str,
) -> anyhow::Result<serde_json::Value> {
    poll_until(
        || {
            let result = query_doc(conn, "SELECT payload FROM event_log WHERE org_id=?1 AND key=?2", org_id, key);
            async move { result }
        },
        &PollConfig::default(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("wait_for_event({}): {}", key, e))
}

pub fn create_entity_table(conn: &Arc<turso_core::Connection>, table: &str) -> anyhow::Result<()> {
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} (\
         org_id TEXT NOT NULL, doc_id TEXT NOT NULL, payload TEXT NOT NULL DEFAULT '{{}}', \
         fts_title TEXT NOT NULL DEFAULT '', fts_body TEXT NOT NULL DEFAULT '', \
         change_time INTEGER NOT NULL DEFAULT 0, node_id TEXT NOT NULL DEFAULT '', \
         PRIMARY KEY (org_id, doc_id))",
        table
    );
    conn.execute(&sql)?;
    Ok(())
}

pub fn create_event_log_table(conn: &Arc<turso_core::Connection>) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS event_log (\
         key TEXT PRIMARY KEY, org_id TEXT NOT NULL, event_type TEXT NOT NULL, \
         hlc_ts INTEGER NOT NULL, hlc_count INTEGER NOT NULL, hlc_node TEXT NOT NULL, \
         schema_version INTEGER NOT NULL, entity TEXT NOT NULL, payload TEXT NOT NULL)"
    )?;
    Ok(())
}
