use std::num::NonZero;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use syntrix_core::schema::{ColumnMeta, ColumnType, ChildTableMeta, EntityMeta};
use syntrix_network::cdc::CdcEvent;

pub use syntrix_core::entity_table_name;

fn entity_from_event_type(event_type: &str) -> &str {
    match event_type.split('.').next().unwrap_or(event_type) {
        "invoice" | "invoices" => "invoices",
        "order" | "orders" => "orders",
        "product" | "products" => "products",
        "customer" | "customers" => "customers",
        "supplier" | "suppliers" => "suppliers",
        "payroll" => "payroll",
        other => other,
    }
}

#[derive(Debug, Clone)]
pub struct QueryFilter {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub filters: Vec<QueryFilter>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self { filters: vec![], sort: None, limit: None, offset: None }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HlcTimestamp {
    pub ts: u64,
    pub count: u32,
    pub node: String,
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ts.cmp(&other.ts)
            .then(self.count.cmp(&other.count))
            .then(self.node.cmp(&other.node))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventEntry {
    pub key: String,
    pub event_type: String,
    pub hlc: HlcTimestamp,
    pub schema_version: u32,
    pub entity: String,
    pub payload: serde_json::Value,
}

pub struct SqlEngine {
    pub conn: Arc<turso_core::Connection>,
    /// Serializes ALL database operations on `conn`. turso_core's WAL only
    /// supports ONE read transaction at a time per connection — without this
    /// lock, the background CDC publish loop, gossip event processing, and
    /// Tauri command handlers race on the same connection and cause a panic
    /// in wal.rs ("cannot start a new read tx without ending an existing one").
    pub db_lock: Arc<Mutex<()>>,
    /// Per-org mutexes for serializing CDC event application. Gossipsub
    /// can deliver the same CDC batch twice (direct + admin-forwarded).
    /// Without serialization, concurrent processing of duplicate batches
    /// can cause LWW races where an older event overwrites a newer one.
    apply_locks: std::sync::Mutex<std::collections::HashMap<String, Arc<std::sync::Mutex<()>>>>,
}

fn current_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Convert a JSON value into the turso value matching a column's declared type. Missing/null
/// business fields become SQL NULL (nullable columns) or the type's zero value otherwise.
fn json_to_column_value(v: Option<&serde_json::Value>, ty: ColumnType, nullable: bool) -> turso_core::Value {
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
        (Some(serde_json::Value::Number(n)), ColumnType::Text) => turso_core::Value::from_text(n.to_string()),
        (Some(serde_json::Value::Bool(b)), ColumnType::Text) => turso_core::Value::from_text(b.to_string()),
        (Some(serde_json::Value::Number(n)), ColumnType::Integer) => {
            turso_core::Value::from_i64(n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64))
        }
        (Some(serde_json::Value::Bool(b)), ColumnType::Integer) => turso_core::Value::from_i64(if *b { 1 } else { 0 }),
        (Some(serde_json::Value::String(s)), ColumnType::Integer) => {
            turso_core::Value::from_i64(s.parse::<i64>().unwrap_or(0))
        }
        (Some(serde_json::Value::Number(n)), ColumnType::Real) => turso_core::Value::from_f64(n.as_f64().unwrap_or(0.0)),
        (Some(serde_json::Value::String(s)), ColumnType::Real) => {
            turso_core::Value::from_f64(s.parse::<f64>().unwrap_or(0.0))
        }
        _ => turso_core::Value::Null,
    }
}

/// Convert a turso row value back into a JSON value according to the column's declared type.
fn column_value_to_json(v: &turso_core::Value) -> serde_json::Value {
    match v {
        turso_core::Value::Null => serde_json::Value::Null,
        turso_core::Value::Text(t) => serde_json::Value::String(t.as_str().to_string()),
        turso_core::Value::Numeric(turso_core::Numeric::Integer(i)) => serde_json::json!(i),
        turso_core::Value::Numeric(turso_core::Numeric::Float(f)) => {
            serde_json::json!(f64::from(*f))
        }
        turso_core::Value::Blob(_) => serde_json::Value::Null,
    }
}

fn bind_value(stmt: &mut turso_core::Statement, idx: usize, value: turso_core::Value) -> anyhow::Result<()> {
    stmt.bind_at(NonZero::new(idx).unwrap(), value)?;
    Ok(())
}

fn run_to_completion(stmt: &mut turso_core::Statement, ctx: &str) -> anyhow::Result<()> {
    loop {
        match stmt.step()? {
            turso_core::StepResult::Row => {}
            turso_core::StepResult::Done => break,
            turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                stmt._io().step()?;
            }
            turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                anyhow::bail!("{}: database busy or interrupted", ctx);
            }
        }
    }

    Ok(())
}

impl SqlEngine {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let conn = crate::storage::open_limbo(&data_dir)?;
        crate::storage::run_migrations(&conn)?;
        Ok(Self {
            conn,
            db_lock: Arc::new(Mutex::new(())),
            apply_locks: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    pub fn with_connection(conn: Arc<turso_core::Connection>) -> Self {
        Self {
            conn,
            db_lock: Arc::new(Mutex::new(())),
            apply_locks: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Returns a per-org mutex for serializing CDC event application.
    /// Ensures that for a given org, only one batch of CDC events is
    /// processed at a time, preventing LWW races from gossipsub duplicates.
    pub fn org_apply_lock(&self, org_id: &str) -> Arc<std::sync::Mutex<()>> {
        let mut locks = self.apply_locks.lock().unwrap();
        locks.entry(org_id.to_string())
            .or_insert_with(|| Arc::new(std::sync::Mutex::new(())))
            .clone()
    }

    pub fn query_events_since(
        &self,
        org_id: &str,
        cursor_ts: u64,
        limit: usize,
    ) -> anyhow::Result<Vec<EventEntry>> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare(
            "SELECT key, event_type, hlc_ts, hlc_count, hlc_node, schema_version, entity, payload FROM event_log WHERE org_id=?1 AND hlc_ts>?2 ORDER BY hlc_ts, hlc_count LIMIT ?3",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_i64(cursor_ts as i64))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(limit as i64))?;

        let mut results = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let key: String = row.get(0)?;
                        let event_type: String = row.get(1)?;
                        let hlc_ts: i64 = row.get(2)?;
                        let hlc_count: i64 = row.get(3)?;
                        let hlc_node: String = row.get(4)?;
                        let schema_version: i64 = row.get(5)?;
                        let entity: String = row.get(6)?;
                        let payload_str: String = row.get(7)?;
                        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

                        results.push(EventEntry {
                            key,
                            event_type,
                            hlc: HlcTimestamp {
                                ts: hlc_ts as u64,
                                count: hlc_count as u32,
                                node: hlc_node,
                            },
                            schema_version: schema_version as u32,
                            entity,
                            payload,
                        });
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("query_events_since: database busy or interrupted");
                }
            }
        }
        Ok(results)
    }

    pub fn upsert_member(&self, org_id: &str, node_id: &str, member_json: &serde_json::Value) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let data = serde_json::to_string(member_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO members (org_id, node_id, data) VALUES (?1, ?2, ?3)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(data.clone()))?;
        run_to_completion(&mut stmt, "upsert_member")
    }

    pub fn get_members(&self, org_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("SELECT data FROM members WHERE org_id=?1")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;

        let mut results = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let data_str: String = row.get(0)?;
                        if let Ok(val) = serde_json::from_str(&data_str) {
                            results.push(val);
                        }
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_members: database busy or interrupted");
                }
            }
        }

        Ok(results)
    }

    pub fn get_member_role(&self, org_id: &str, node_id: &str) -> Option<String> {
        let members = self.get_members(org_id).unwrap_or_default();
        members.iter().find(|m| m.get("node_id").and_then(|v| v.as_str()) == Some(node_id))
            .and_then(|m| m.get("role").and_then(|v| v.as_str()).map(String::from))
    }

    pub fn delete_member(&self, org_id: &str, node_id: &str) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("DELETE FROM members WHERE org_id=?1 AND node_id=?2")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        run_to_completion(&mut stmt, "delete_member")
    }

    pub fn upsert_role_cfg(&self, org_id: &str, role_name: &str, role_json: &serde_json::Value) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let data = serde_json::to_string(role_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO roles (org_id, role_name, data) VALUES (?1, ?2, ?3)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(role_name.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(data.clone()))?;
        run_to_completion(&mut stmt, "upsert_role_cfg")
    }

    pub fn get_roles(&self, org_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("SELECT data FROM roles WHERE org_id=?1")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;

        let mut results = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let data_str: String = row.get(0)?;
                        if let Ok(val) = serde_json::from_str(&data_str) {
                            results.push(val);
                        }
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_roles: database busy or interrupted");
                }
            }
        }

        Ok(results)
    }

    pub fn delete_role(&self, org_id: &str, role_name: &str) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("DELETE FROM roles WHERE org_id=?1 AND role_name=?2")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(role_name.to_string()))?;
        run_to_completion(&mut stmt, "delete_role")
    }

    /// Whether the device identified by `node_id_hex` is allowed to write to `entity` in
    /// `org_id`, per the local copy of `members`/`roles` (populated from `device.updated`/
    /// `role.updated` gossip — see identity.rs::process_gossip_event). Used to validate the
    /// author of an incoming CDC event before applying it (Fase 3, tarea 15), since the
    /// in-memory `NamespaceRegistry` only tracks this node's own device/role, not peers'.
    pub fn can_node_write(&self, org_id: &str, node_id_hex: &str, entity: &str) -> bool {
        let members = self.get_members(org_id).unwrap_or_default();
        let Some(member) = members.iter().find(|m| m.get("node_id").and_then(|v| v.as_str()) == Some(node_id_hex))
        else {
            return false;
        };
        if member.get("active").and_then(|v| v.as_bool()) == Some(false) {
            return false;
        }
        let Some(role) = member.get("role").and_then(|v| v.as_str()) else { return false };
        if role == "admin" {
            return true;
        }
        let roles = self.get_roles(org_id).unwrap_or_default();
        roles.iter().any(|r| {
            r.get("name").and_then(|n| n.as_str()) == Some(role)
                && r.get("can_write")
                    .and_then(|a| a.as_array())
                    .map(|a| a.iter().any(|v| v.as_str() == Some(entity) || v.as_str() == Some("*")))
                    .unwrap_or(false)
        })
    }

    pub fn upsert_heartbeat(&self, org_id: &str, node_id: &str, hb_json: &serde_json::Value) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let ts = hb_json["ts"].as_i64().unwrap_or(0);
        let data = serde_json::to_string(hb_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO heartbeats (org_id, node_id, ts, data) VALUES (?1, ?2, ?3, ?4)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(ts))?;
        stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(data.clone()))?;
        run_to_completion(&mut stmt, "upsert_heartbeat")
    }

    pub fn get_heartbeats(&self, org_id: &str) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("SELECT node_id, ts FROM heartbeats WHERE org_id=?1")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;

        let mut results = std::collections::HashMap::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let node_id: String = row.get(0)?;
                        let ts: i64 = row.get(1)?;
                        results.insert(node_id, ts);
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_heartbeats: database busy or interrupted");
                }
            }
        }

        Ok(results)
    }

    /// Typed projection: business_json -> real columns (Fase 2, tarea 9). Replaces the old
    /// `payload` JSON blob write. Also handles child rows (invoice_items/order_items) via
    /// delete+reinsert, and keeps the FTS5 shadow table in sync.
    pub fn upsert_document_full(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &serde_json::Value,
        change_time: i64,
        node_id: &str,
    ) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let meta = syntrix_core::entity_meta(entity)?;
        let business_cols = syntrix_core::business_columns(meta);

        let mut col_names: Vec<&str> = vec!["org_id", "doc_id"];
        col_names.extend(business_cols.iter().map(|c| c.name.as_str()));
        col_names.push("change_time");
        col_names.push("node_id");

        let placeholders: Vec<String> = (1..=col_names.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
            meta.table,
            col_names.join(", "),
            placeholders.join(", ")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut stmt, 2, turso_core::Value::from_text(doc_id.to_string()))?;
        let mut idx = 3;
        for col in &business_cols {
            let json_key = business_json_key(col);
            let v = json_to_column_value(json_payload.get(json_key), col.ty, col.nullable);
            bind_value(&mut stmt, idx, v)?;
            idx += 1;
        }
        bind_value(&mut stmt, idx, turso_core::Value::from_i64(change_time))?;
        idx += 1;
        bind_value(&mut stmt, idx, turso_core::Value::from_text(node_id.to_string()))?;
        run_to_completion(&mut stmt, "upsert_document_full")?;

        for child in &meta.children {
            self.replace_children(org_id, doc_id, child, json_payload, change_time, node_id)?;
        }

        Ok(())
    }

    fn replace_children(
        &self,
        org_id: &str,
        parent_doc_id: &str,
        child: &ChildTableMeta,
        parent_payload: &serde_json::Value,
        change_time: i64,
        node_id: &str,
    ) -> anyhow::Result<()> {
        let delete_sql = format!("DELETE FROM {} WHERE org_id=?1 AND {}=?2", child.table, child.parent_key);
        let mut del_stmt = self.conn.prepare(&delete_sql)?;
        bind_value(&mut del_stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut del_stmt, 2, turso_core::Value::from_text(parent_doc_id.to_string()))?;
        run_to_completion(&mut del_stmt, "replace_children:delete")?;

        let items = parent_payload.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let business_cols = syntrix_core::child_business_columns(child);

        for (idx_in_list, item) in items.iter().enumerate() {
            let mut col_names: Vec<&str> = vec!["org_id", child.parent_key.as_str()];
            col_names.extend(business_cols.iter().map(|c| c.name.as_str()));
            col_names.push("change_time");
            col_names.push("node_id");

            let placeholders: Vec<String> = (1..=col_names.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                child.table,
                col_names.join(", "),
                placeholders.join(", ")
            );
            let mut stmt = self.conn.prepare(&sql)?;
            bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
            bind_value(&mut stmt, 2, turso_core::Value::from_text(parent_doc_id.to_string()))?;
            let mut col_idx = 3;
            for col in &business_cols {
                let value = if col.name == "line_id" {
                    let line_id = item.get("line_id").and_then(|v| v.as_str()).map(String::from)
                        .unwrap_or_else(|| format!("{parent_doc_id}-{idx_in_list}"));
                    turso_core::Value::from_text(line_id)
                } else {
                    json_to_column_value(item.get(col.name.as_str()), col.ty, col.nullable)
                };
                bind_value(&mut stmt, col_idx, value)?;
                col_idx += 1;
            }
            bind_value(&mut stmt, col_idx, turso_core::Value::from_i64(change_time))?;
            col_idx += 1;
            bind_value(&mut stmt, col_idx, turso_core::Value::from_text(node_id.to_string()))?;
            run_to_completion(&mut stmt, "replace_children:insert")?;
        }

        Ok(())
    }

    pub fn upsert_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        self.upsert_document_full(org_id, entity, doc_id, json_payload, current_millis(), "")
    }

    pub fn query(
        &self,
        org_id: &str,
        entity: &str,
        options: &QueryOptions,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let _lock = self.db_lock.lock().unwrap();
        let meta = syntrix_core::entity_meta(entity)?;
        let business_cols = syntrix_core::business_columns(meta);

        let mut select_cols: Vec<&str> = vec!["doc_id"];
        select_cols.extend(business_cols.iter().map(|c| c.name.as_str()));

        let mut sql = format!("SELECT {} FROM {} WHERE org_id=?1", select_cols.join(", "), meta.table);
        let mut param_idx = 2usize;
        let mut bind_plan: Vec<(usize, turso_core::Value)> = vec![];

        for filter in &options.filters {
            let (col, ty) = resolve_filterable_column(meta, &filter.field)?;
            sql.push_str(&format!(" AND {} = ?{}", col, param_idx));
            bind_plan.push((param_idx, json_to_column_value(Some(&serde_json::Value::String(filter.value.clone())), ty, true)));
            param_idx += 1;
        }

        if let Some(ref sort_field) = options.sort {
            let (col, _) = resolve_filterable_column(meta, sort_field)?;
            sql.push_str(&format!(" ORDER BY {}", col));
        }

        if let Some(limit) = options.limit {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
            bind_plan.push((param_idx, turso_core::Value::from_i64(limit as i64)));
            param_idx += 1;
        }

        if let Some(offset) = options.offset {
            sql.push_str(&format!(" OFFSET ?{}", param_idx));
            bind_plan.push((param_idx, turso_core::Value::from_i64(offset as i64)));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        for (idx, value) in bind_plan {
            bind_value(&mut stmt, idx, value)?;
        }

        let mut results = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let mut obj = serde_json::Map::new();
                        let doc_id_val: &turso_core::Value = row.get(0)?;
                        obj.insert("id".to_string(), column_value_to_json(doc_id_val));
                        for (i, col) in business_cols.iter().enumerate() {
                            let v: &turso_core::Value = row.get(i + 1)?;
                            obj.insert(col.name.clone(), column_value_to_json(v));
                        }
                        results.push(serde_json::Value::Object(obj));
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("query: database busy or interrupted");
                }
            }
        }

        Ok(results)
    }

    pub fn get_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let _lock = self.db_lock.lock().unwrap();
        let meta = syntrix_core::entity_meta(entity)?;
        let business_cols = syntrix_core::business_columns(meta);

        let select_cols: Vec<&str> = business_cols.iter().map(|c| c.name.as_str()).collect();
        let sql = format!("SELECT {} FROM {} WHERE org_id=?1 AND doc_id=?2", select_cols.join(", "), meta.table);

        let mut stmt = self.conn.prepare(&sql)?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut stmt, 2, turso_core::Value::from_text(doc_id.to_string()))?;

        let mut found: Option<serde_json::Map<String, serde_json::Value>> = None;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let mut obj = serde_json::Map::new();
                        obj.insert("id".to_string(), serde_json::Value::String(doc_id.to_string()));
                        for (i, col) in business_cols.iter().enumerate() {
                            let v: &turso_core::Value = row.get(i)?;
                            obj.insert(col.name.clone(), column_value_to_json(v));
                        }
                        found = Some(obj);
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_document: database busy or interrupted");
                }
            }
        }

        let mut obj = match found {
            Some(o) => o,
            None => return Ok(None),
        };

        if let Some(child) = meta.children.first() {
            let items = self.query_children(org_id, doc_id, child)?;
            obj.insert("items".to_string(), serde_json::Value::Array(items));
        }

        Ok(Some(serde_json::Value::Object(obj)))
    }

    fn query_children(
        &self,
        org_id: &str,
        parent_doc_id: &str,
        child: &ChildTableMeta,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let business_cols = syntrix_core::child_business_columns(child);
        let select_cols: Vec<&str> = business_cols.iter().map(|c| c.name.as_str()).collect();
        let sql = format!(
            "SELECT {} FROM {} WHERE org_id=?1 AND {}=?2 ORDER BY line_id",
            select_cols.join(", "),
            child.table,
            child.parent_key
        );
        let mut stmt = self.conn.prepare(&sql)?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut stmt, 2, turso_core::Value::from_text(parent_doc_id.to_string()))?;

        let mut items = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let mut obj = serde_json::Map::new();
                        for (i, col) in business_cols.iter().enumerate() {
                            let v: &turso_core::Value = row.get(i)?;
                            obj.insert(col.name.clone(), column_value_to_json(v));
                        }
                        items.push(serde_json::Value::Object(obj));
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("query_children: database busy or interrupted");
                }
            }
        }
        Ok(items)
    }

    pub fn delete_document(&self, org_id: &str, entity: &str, doc_id: &str) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let meta = syntrix_core::entity_meta(entity)?;

        for child in &meta.children {
            let sql = format!("DELETE FROM {} WHERE org_id=?1 AND {}=?2", child.table, child.parent_key);
            let mut stmt = self.conn.prepare(&sql)?;
            bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
            bind_value(&mut stmt, 2, turso_core::Value::from_text(doc_id.to_string()))?;
            run_to_completion(&mut stmt, "delete_document:children")?;
        }

        let sql = format!("DELETE FROM {} WHERE org_id=?1 AND doc_id=?2", meta.table);
        let mut stmt = self.conn.prepare(&sql)?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut stmt, 2, turso_core::Value::from_text(doc_id.to_string()))?;
        run_to_completion(&mut stmt, "delete_document")?;

        Ok(())
    }

    /// Last `turso_cdc.change_id` successfully published for this org (Fase 3, tarea 17).
    pub fn get_cdc_cursor(&self, org_id: &str) -> anyhow::Result<u64> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare("SELECT last_change_id FROM cdc_cursor WHERE org_id=?1")?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        let mut cursor = 0u64;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let v: i64 = row.get(0)?;
                        cursor = v as u64;
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_cdc_cursor: database busy or interrupted");
                }
            }
        }
        Ok(cursor)
    }

    pub fn set_cdc_cursor(&self, org_id: &str, change_id: u64) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare(
            "INSERT INTO cdc_cursor (org_id, last_change_id, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(org_id) DO UPDATE SET last_change_id=excluded.last_change_id, updated_at=excluded.updated_at",
        )?;
        bind_value(&mut stmt, 1, turso_core::Value::from_text(org_id.to_string()))?;
        bind_value(&mut stmt, 2, turso_core::Value::from_i64(change_id as i64))?;
        bind_value(&mut stmt, 3, turso_core::Value::from_i64(current_millis()))?;
        run_to_completion(&mut stmt, "set_cdc_cursor")
    }

    // ===== Lock-wrapped database operations for external callers =====

    /// Read CDC events from `turso_cdc`, acquiring `db_lock` for the duration.
    pub fn read_cdc_events(
        &self,
        since_change_id: u64,
        limit: usize,
        table_filter: Option<&str>,
    ) -> anyhow::Result<(Vec<syntrix_network::cdc::CdcEvent>, u64)> {
        let _lock = self.db_lock.lock().unwrap();
        syntrix_network::cdc::read_cdc_events(&self.conn, since_change_id, limit, table_filter)
    }

    /// Apply CDC events to the local database, acquiring `db_lock` for the duration.
    pub fn apply_cdc_events(
        &self,
        events: &[syntrix_network::cdc::CdcEvent],
        perm: &dyn syntrix_network::cdc::AuthorPermissionCheck,
    ) -> anyhow::Result<()> {
        let _lock = self.db_lock.lock().unwrap();
        syntrix_network::cdc::apply_cdc_events(&self.conn, events, perm)
    }

    /// Execute arbitrary SQL with params, acquiring `db_lock` for the duration.
    /// Returns rows as JSON value. Used by `drizzle_execute_impl`.
    pub fn drizzle_execute(&self, sql: &str, params: &[String]) -> Result<serde_json::Value, String> {
        use turso_core::StepResult;
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;

        for (i, p) in params.iter().enumerate() {
            stmt.bind_at(NonZero::new(i + 1).unwrap(), turso_core::Value::from_text(p.clone()))
                .map_err(|e| e.to_string())?;
        }

        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        loop {
            match stmt.step().map_err(|e| e.to_string())? {
                StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let mut cols: Vec<serde_json::Value> = Vec::new();
                        for idx in 0..32 {
                            let s: String = match row.get(idx) {
                                Ok(v) => v,
                                Err(_) => break,
                            };
                            cols.push(serde_json::Value::String(s));
                        }
                        rows.push(cols);
                    }
                }
                StepResult::Done => break,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step().map_err(|e| e.to_string())?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    return Err("drizzle_execute: database busy".into());
                }
            }
        }

        Ok(serde_json::json!({ "rows": rows }))
    }

    /// Execute live query SQL on the connection, acquiring `db_lock` for the duration.
    /// Returns rows as JSON objects. Used by live query notification.
    pub fn execute_sql_query(&self, sql: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let _lock = self.db_lock.lock().unwrap();
        let mut stmt = self.conn.prepare(sql)?;
        use turso_core::StepResult;

        let num_cols = stmt.num_columns();
        let col_names: Vec<String> = (0..num_cols).map(|i| stmt.get_column_name(i).to_string()).collect();

        let mut rows = Vec::new();
        loop {
            match stmt.step()? {
                StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let mut obj = serde_json::Map::new();
                        for i in 0..num_cols {
                            let col_name = col_names.get(i).cloned().unwrap_or_else(|| format!("col_{}", i));
                            if let Ok(val) = row.get::<String>(i) {
                                obj.insert(col_name, serde_json::Value::String(val));
                            } else if let Ok(val) = row.get::<i64>(i) {
                                obj.insert(col_name, serde_json::json!(val));
                            } else if let Ok(val) = row.get::<f64>(i) {
                                obj.insert(col_name, serde_json::json!(val));
                            } else {
                                obj.insert(col_name, serde_json::Value::Null);
                            }
                        }
                        rows.push(serde_json::Value::Object(obj));
                    }
                }
                StepResult::Done => break,
                StepResult::IO | StepResult::Yield => {
                    stmt._io().step()?;
                }
                StepResult::Interrupt | StepResult::Busy => {
                    anyhow::bail!("sql query interrupted or busy");
                }
            }
        }
        Ok(rows)
    }

    /// Snapshot all rows for an org as CDC events, acquiring `db_lock` for the duration.
    pub fn snapshot_org_rows(&self, org_id: &str) -> anyhow::Result<Vec<syntrix_network::cdc::CdcEvent>> {
        let _lock = self.db_lock.lock().unwrap();
        syntrix_network::cdc::snapshot_org_rows(&self.conn, org_id)
    }

    /// Search across entities using full-text search, acquiring `db_lock` for the duration.
    pub fn search(
        &self,
        org_id: &str,
        query: &str,
        entities: Option<Vec<String>>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::search::SearchResult>> {
        let _lock = self.db_lock.lock().unwrap();
        let engine = crate::search::SearchEngine::new(self.conn.clone());
        engine.search(org_id, query, entities, limit)
    }
}

/// Maps a real column name to the JSON key used in the reconstructed IPC document shape.
/// Only `doc_id` is renamed (to `id`, to match the frontend field registry); every other
/// column name is already the JSON key (snake_case matches on both sides).
fn business_json_key(col: &ColumnMeta) -> &str {
    col.name.as_str()
}

/// Resolves a frontend-facing field name (as used in QueryFilter/sort) to a real column name +
/// type, accepting `id` as an alias for `doc_id`.
fn resolve_filterable_column<'a>(meta: &'a EntityMeta, field: &str) -> anyhow::Result<(&'a str, ColumnType)> {
    if field == "id" {
        return Ok(("doc_id", ColumnType::Text));
    }
    meta.columns
        .iter()
        .find(|c| c.name == field)
        .map(|c| (c.name.as_str(), c.ty))
        .ok_or_else(|| anyhow::anyhow!("unknown field '{}' for entity table '{}'", field, meta.table))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_table_name_valid() {
        assert_eq!(entity_table_name("customers").unwrap(), "customers");
        assert_eq!(entity_table_name("customer").unwrap(), "customers");
        assert_eq!(entity_table_name("payroll").unwrap(), "payroll");
    }

    #[test]
    fn test_entity_table_name_invalid() {
        assert!(entity_table_name("unknown").is_err());
    }

    #[test]
    fn test_entity_from_event_type() {
        assert_eq!(entity_from_event_type("customer.created"), "customers");
        assert_eq!(entity_from_event_type("invoice.paid"), "invoices");
        assert_eq!(entity_from_event_type("payroll.processed"), "payroll");
        assert_eq!(entity_from_event_type("unknown.action"), "unknown");
    }

    #[test]
    fn test_hlc_timestamp_ordering() {
        let a = HlcTimestamp { ts: 100, count: 0, node: "a".into() };
        let b = HlcTimestamp { ts: 200, count: 0, node: "a".into() };
        assert!(a < b);

        let c = HlcTimestamp { ts: 100, count: 1, node: "a".into() };
        assert!(a < c);

        let d = HlcTimestamp { ts: 100, count: 0, node: "b".into() };
        assert!(a < d);
    }

    #[test]
    fn test_hlc_timestamp_eq() {
        let a = HlcTimestamp { ts: 100, count: 5, node: "node1".into() };
        let b = HlcTimestamp { ts: 100, count: 5, node: "node1".into() };
        assert_eq!(a, b);
    }

    #[test]
    fn test_query_options_default() {
        let opts = QueryOptions::default();
        assert!(opts.filters.is_empty());
        assert!(opts.sort.is_none());
        assert!(opts.limit.is_none());
        assert!(opts.offset.is_none());
    }

    fn test_engine() -> (impl Drop, SqlEngine) {
        let (dir, conn) = syntrix_testkit::temp_limbo_db();
        crate::storage::run_migrations(&conn).expect("run_migrations");
        (dir, SqlEngine::with_connection(conn))
    }

    #[test]
    fn upsert_and_get_document_roundtrip_typed_columns() {
        let (_dir, engine) = test_engine();
        let payload = serde_json::json!({
            "name": "Alice",
            "tax_id": "TAX1",
            "email": "alice@test.com",
        });
        engine.upsert_document("org1", "customers", "c1", &payload).unwrap();

        let doc = engine.get_document("org1", "customers", "c1").unwrap().unwrap();
        assert_eq!(doc["id"], "c1");
        assert_eq!(doc["name"], "Alice");
        assert_eq!(doc["tax_id"], "TAX1");
        assert_eq!(doc["email"], "alice@test.com");
        assert_eq!(doc["address"], serde_json::Value::Null);
    }

    #[test]
    fn upsert_invoice_with_line_items_delete_and_reinsert() {
        let (_dir, engine) = test_engine();
        let payload = serde_json::json!({
            "customer_id": "cust1",
            "amount": 150.0,
            "status": "open",
            "tax_rate": 0.16,
            "date": "2024-01-01",
            "items": [
                { "product_id": "p1", "qty": 2.0, "price": 50.0 },
                { "product_id": "p2", "qty": 1.0, "price": 50.0 },
            ],
        });
        engine.upsert_document("org1", "invoices", "inv1", &payload).unwrap();

        let doc = engine.get_document("org1", "invoices", "inv1").unwrap().unwrap();
        assert_eq!(doc["customer_id"], "cust1");
        assert_eq!(doc["amount"], 150.0);
        let items = doc["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["product_id"], "p1");
        assert_eq!(items[0]["qty"], 2.0);

        // Update with fewer items: old rows must be gone (delete+reinsert semantics).
        let payload2 = serde_json::json!({
            "customer_id": "cust1",
            "amount": 50.0,
            "status": "open",
            "tax_rate": 0.16,
            "date": "2024-01-01",
            "items": [ { "product_id": "p1", "qty": 1.0, "price": 50.0 } ],
        });
        engine.upsert_document("org1", "invoices", "inv1", &payload2).unwrap();
        let doc2 = engine.get_document("org1", "invoices", "inv1").unwrap().unwrap();
        let items2 = doc2["items"].as_array().unwrap();
        assert_eq!(items2.len(), 1);
        assert_eq!(items2[0]["product_id"], "p1");
    }

    #[test]
    fn query_filters_on_real_typed_columns_without_json_extract() {
        let (_dir, engine) = test_engine();
        engine.upsert_document("org1", "invoices", "inv1", &serde_json::json!({
            "customer_id": "cust1", "amount": 10.0, "status": "open", "date": "2024-01-01",
        })).unwrap();
        engine.upsert_document("org1", "invoices", "inv2", &serde_json::json!({
            "customer_id": "cust2", "amount": 20.0, "status": "paid", "date": "2024-01-02",
        })).unwrap();

        let options = QueryOptions {
            filters: vec![QueryFilter { field: "status".to_string(), value: "paid".to_string() }],
            ..Default::default()
        };
        let results = engine.query("org1", "invoices", &options).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["id"], "inv2");
        assert_eq!(results[0]["customer_id"], "cust2");
    }

    #[test]
    fn delete_document_removes_row_and_children() {
        let (_dir, engine) = test_engine();
        engine.upsert_document("org1", "invoices", "inv1", &serde_json::json!({
            "customer_id": "cust1", "amount": 10.0, "status": "open", "date": "2024-01-01",
            "items": [ { "product_id": "p1", "qty": 1.0, "price": 10.0 } ],
        })).unwrap();
        engine.delete_document("org1", "invoices", "inv1").unwrap();
        assert!(engine.get_document("org1", "invoices", "inv1").unwrap().is_none());
    }

    #[test]
    fn cdc_cursor_roundtrip_and_upsert() {
        let (_dir, engine) = test_engine();
        assert_eq!(engine.get_cdc_cursor("org1").unwrap(), 0, "no cursor yet defaults to 0");

        engine.set_cdc_cursor("org1", 42).unwrap();
        assert_eq!(engine.get_cdc_cursor("org1").unwrap(), 42);

        engine.set_cdc_cursor("org1", 100).unwrap();
        assert_eq!(engine.get_cdc_cursor("org1").unwrap(), 100, "cursor updates in place");

        assert_eq!(engine.get_cdc_cursor("org2").unwrap(), 0, "cursor is per-org");
    }
}
