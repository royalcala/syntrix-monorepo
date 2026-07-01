use std::num::NonZero;
use std::path::PathBuf;
use std::sync::Arc;

fn entity_table_name(entity: &str) -> anyhow::Result<&'static str> {
    match entity {
        "customers" | "customer" => Ok("customers"),
        "suppliers" | "supplier" => Ok("suppliers"),
        "products" | "product" => Ok("products"),
        "invoices" | "invoice" => Ok("invoices"),
        "orders" | "order" => Ok("orders"),
        "payroll" => Ok("payroll"),
        _ => anyhow::bail!("unknown entity: {}", entity),
    }
}

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
}

impl SqlEngine {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let conn = crate::storage::open_limbo(&data_dir)?;
        crate::storage::run_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn with_connection(conn: Arc<turso_core::Connection>) -> Self {
        Self { conn }
    }

    pub fn append_event(&self, org_id: &str, event_json: &serde_json::Value) -> anyhow::Result<String> {
        let event_type = event_json["type"].as_str().unwrap_or("unknown");
        let entity = entity_from_event_type(event_type);

        let hlc_val = &event_json["hlc"];
        let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
        let hlc_count = hlc_val["count"].as_u64().unwrap_or(0);
        let hlc_node = hlc_val["node"].as_str().unwrap_or("");
        let schema_version = event_json["schema_version"].as_u64().unwrap_or(1);
        let payload = event_json.get("payload").cloned().unwrap_or_default();
        let payload_str = serde_json::to_string(&payload)?;

        let key = format!("evt:{}:{:020}:{:08}:{}", org_id, hlc_ts, hlc_count, hlc_node);

        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO event_log (key, org_id, event_type, hlc_ts, hlc_count, hlc_node, schema_version, entity, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(key.clone()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(event_type.to_string()))?;
        stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_i64(hlc_ts as i64))?;
        stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_i64(hlc_count as i64))?;
        stmt.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_text(hlc_node.to_string()))?;
        stmt.bind_at(NonZero::new(7).unwrap(), turso_core::Value::from_i64(schema_version as i64))?;
        stmt.bind_at(NonZero::new(8).unwrap(), turso_core::Value::from_text(entity.to_string()))?;
        stmt.bind_at(NonZero::new(9).unwrap(), turso_core::Value::from_text(payload_str.clone()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("append_event: database busy or interrupted");
                }
            }
        }

        Ok(key)
    }

    pub fn query_events_since(
        &self,
        org_id: &str,
        cursor_ts: u64,
        limit: usize,
    ) -> anyhow::Result<Vec<EventEntry>> {
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
        let data = serde_json::to_string(member_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO members (org_id, node_id, data) VALUES (?1, ?2, ?3)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(data.clone()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_member: database busy or interrupted");
                }
            }
        }
        Ok(())
    }

    pub fn get_members(&self, org_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
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

    pub fn delete_member(&self, org_id: &str, node_id: &str) -> anyhow::Result<()> {
        let mut stmt = self.conn.prepare("DELETE FROM members WHERE org_id=?1 AND node_id=?2")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("delete_member: database busy or interrupted");
                }
            }
        }
        Ok(())
    }

    pub fn upsert_role_cfg(&self, org_id: &str, role_name: &str, role_json: &serde_json::Value) -> anyhow::Result<()> {
        let data = serde_json::to_string(role_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO roles (org_id, role_name, data) VALUES (?1, ?2, ?3)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(role_name.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(data.clone()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_role_cfg: database busy or interrupted");
                }
            }
        }
        Ok(())
    }

    pub fn get_roles(&self, org_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
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
        let mut stmt = self.conn.prepare("DELETE FROM roles WHERE org_id=?1 AND role_name=?2")?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(role_name.to_string()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("delete_role: database busy or interrupted");
                }
            }
        }
        Ok(())
    }

    pub fn upsert_heartbeat(&self, org_id: &str, node_id: &str, hb_json: &serde_json::Value) -> anyhow::Result<()> {
        let ts = hb_json["ts"].as_i64().unwrap_or(0);
        let data = serde_json::to_string(hb_json)?;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO heartbeats (org_id, node_id, ts, data) VALUES (?1, ?2, ?3, ?4)",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(node_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_i64(ts))?;
        stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(data.clone()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_heartbeat: database busy or interrupted");
                }
            }
        }
        Ok(())
    }

    pub fn get_heartbeats(&self, org_id: &str) -> anyhow::Result<std::collections::HashMap<String, i64>> {
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

    pub fn upsert_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let table = entity_table_name(entity)?;
        let payload_str = serde_json::to_string(json_payload)?;
        let fts_title = extract_title(json_payload);
        let fts_body = extract_body(json_payload);

        let sql = format!(
            "INSERT OR REPLACE INTO {} (org_id, doc_id, payload, fts_title, fts_body) VALUES (?1, ?2, ?3, ?4, ?5)",
            table
        );

        let mut stmt = self.conn.prepare(&sql)?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(payload_str.clone()))?;
        stmt.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(fts_title.clone()))?;
        stmt.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(fts_body.clone()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_document: database busy or interrupted");
                }
            }
        }

        Ok(())
    }

    pub fn upsert_document_with_hlc(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &serde_json::Value,
        hlc: &HlcTimestamp,
    ) -> anyhow::Result<()> {
        // Check existing HLC from tracker
        let mut stmt = self.conn.prepare(
            "SELECT hlc_ts, hlc_count, hlc_node FROM hlc_tracker WHERE org_id=?1 AND entity=?2 AND doc_id=?3",
        )?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(entity.to_string()))?;
        stmt.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;

        let mut skip = false;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let stored_ts: i64 = row.get(0)?;
                        let stored_count: i64 = row.get(1)?;
                        let stored_node: String = row.get(2)?;
                        let stored = HlcTimestamp {
                            ts: stored_ts as u64,
                            count: stored_count as u32,
                            node: stored_node,
                        };
                        if hlc <= &stored {
                            skip = true;
                        }
                    }
                }
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_document_with_hlc: database busy or interrupted");
                }
            }
        }

        drop(stmt);

        if skip {
            return Ok(());
        }

        // Upsert document
        let table = entity_table_name(entity)?;
        let payload_str = serde_json::to_string(json_payload)?;
        let fts_title = extract_title(json_payload);
        let fts_body = extract_body(json_payload);

        let sql = format!(
            "INSERT OR REPLACE INTO {} (org_id, doc_id, payload, fts_title, fts_body) VALUES (?1, ?2, ?3, ?4, ?5)",
            table
        );
        let mut stmt2 = self.conn.prepare(&sql)?;
        stmt2.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt2.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;
        stmt2.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(payload_str.clone()))?;
        stmt2.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_text(fts_title.clone()))?;
        stmt2.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_text(fts_body.clone()))?;
        loop {
            match stmt2.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt2._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_document_with_hlc: database busy or interrupted");
                }
            }
        }
        drop(stmt2);

        // Update HLC tracker
        let mut stmt3 = self.conn.prepare(
            "INSERT OR REPLACE INTO hlc_tracker (org_id, entity, doc_id, hlc_ts, hlc_count, hlc_node) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        stmt3.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt3.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(entity.to_string()))?;
        stmt3.bind_at(NonZero::new(3).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;
        stmt3.bind_at(NonZero::new(4).unwrap(), turso_core::Value::from_i64(hlc.ts as i64))?;
        stmt3.bind_at(NonZero::new(5).unwrap(), turso_core::Value::from_i64(hlc.count as i64))?;
        stmt3.bind_at(NonZero::new(6).unwrap(), turso_core::Value::from_text(hlc.node.clone()))?;
        loop {
            match stmt3.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt3._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("upsert_document_with_hlc: database busy or interrupted");
                }
            }
        }

        Ok(())
    }

    pub fn query(
        &self,
        org_id: &str,
        entity: &str,
        options: &QueryOptions,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let table = entity_table_name(entity)?;

        let mut sql = format!("SELECT payload FROM {} WHERE org_id=?1", table);
        let mut param_idx = 2u32;

        let filter_clauses: Vec<String> = options.filters.iter().map(|f| {
            let clause = format!("json_extract(payload, '$.{}') = ?{}", f.field, param_idx);
            param_idx += 1;
            clause
        }).collect();

        for clause in &filter_clauses {
            sql.push_str(" AND ");
            sql.push_str(clause);
        }

        if let Some(ref sort_field) = options.sort {
            sql.push_str(&format!(" ORDER BY json_extract(payload, '$.{}')", sort_field));
        }

        if let Some(limit) = options.limit {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
            param_idx += 1;
        }

        if let Some(offset) = options.offset {
            sql.push_str(&format!(" OFFSET ?{}", param_idx));
            param_idx += 1;
        }

        let mut stmt = self.conn.prepare(&sql)?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;

        let mut bind_idx = 2u32;
        for filter in &options.filters {
            stmt.bind_at(
                NonZero::new(bind_idx as usize).unwrap(),
                turso_core::Value::from_text(filter.value.clone()),
            )?;
            bind_idx += 1;
        }

        if let Some(limit) = options.limit {
            stmt.bind_at(
                NonZero::new(bind_idx as usize).unwrap(),
                turso_core::Value::from_i64(limit as i64),
            )?;
            bind_idx += 1;
        }

        if let Some(offset) = options.offset {
            stmt.bind_at(
                NonZero::new(bind_idx as usize).unwrap(),
                turso_core::Value::from_i64(offset as i64),
            )?;
        }

        let mut results = Vec::new();
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let payload_str: String = row.get(0)?;
                        if let Ok(val) = serde_json::from_str(&payload_str) {
                            results.push(val);
                        }
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
        let table = entity_table_name(entity)?;
        let sql = format!("SELECT payload FROM {} WHERE org_id=?1 AND doc_id=?2", table);

        let mut stmt = self.conn.prepare(&sql)?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;

        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {
                    if let Some(row) = stmt.row() {
                        let payload_str: String = row.get(0)?;
                        if let Ok(val) = serde_json::from_str(&payload_str) {
                            return Ok(Some(val));
                        }
                    }
                    return Ok(None);
                }
                turso_core::StepResult::Done => return Ok(None),
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("get_document: database busy or interrupted");
                }
            }
        }
    }

    pub fn delete_document(&self, org_id: &str, entity: &str, doc_id: &str) -> anyhow::Result<()> {
        let table = entity_table_name(entity)?;
        let sql = format!("DELETE FROM {} WHERE org_id=?1 AND doc_id=?2", table);

        let mut stmt = self.conn.prepare(&sql)?;
        stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(org_id.to_string()))?;
        stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(doc_id.to_string()))?;
        loop {
            match stmt.step()? {
                turso_core::StepResult::Row => {}
                turso_core::StepResult::Done => break,
                turso_core::StepResult::IO | turso_core::StepResult::Yield => {
                    stmt._io().step()?;
                }
                turso_core::StepResult::Interrupt | turso_core::StepResult::Busy => {
                    anyhow::bail!("delete_document: database busy or interrupted");
                }
            }
        }

        Ok(())
    }
}

fn extract_title(payload: &serde_json::Value) -> String {
    if let Some(obj) = payload.as_object() {
        for key in &["name", "title", "label"] {
            if let Some(v) = obj.get(*key).and_then(|v| v.as_str()) {
                return v.to_string();
            }
        }
    }
    String::new()
}

fn extract_body(payload: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    if let Some(obj) = payload.as_object() {
        for (_k, v) in obj {
            if v.is_string() {
                parts.push(v.as_str().unwrap().to_string());
            } else if v.is_number() || v.is_boolean() {
                parts.push(v.to_string());
            }
        }
    }
    parts.join(" ")
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
    fn test_extract_title_customers() {
        let payload = serde_json::json!({"name": "Acme Corp"});
        assert_eq!(extract_title(&payload), "Acme Corp");
    }

    #[test]
    fn test_extract_title_fallback() {
        let payload = serde_json::json!({"title": "Dr."});
        assert_eq!(extract_title(&payload), "Dr.");
    }

    #[test]
    fn test_extract_body_with_strings() {
        let payload = serde_json::json!({"name": "Alice", "email": "a@b.com"});
        let body = extract_body(&payload);
        assert!(body.contains("Alice"));
        assert!(body.contains("a@b.com"));
    }

    #[test]
    fn test_query_options_default() {
        let opts = QueryOptions::default();
        assert!(opts.filters.is_empty());
        assert!(opts.sort.is_none());
        assert!(opts.limit.is_none());
        assert!(opts.offset.is_none());
    }
}
