use redb::{Database, TableDefinition, ReadableTable};
use std::path::PathBuf;
use serde_json::Value;
use std::sync::Arc;
use crate::search::SearchEngine;
use syntrix_schema::{build_registry, SchemaRegistry, FieldType, encoded::encode_value};

const HLC_TRACKER: TableDefinition<&str, &[u8]> = TableDefinition::new("hlc_tracker");
const INDEXES: TableDefinition<&str, &[u8]> = TableDefinition::new("indexes");
const COMPOSITE: TableDefinition<&str, &[u8]> = TableDefinition::new("composite");
const DOCUMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("documents");
const EVENT_LOG: TableDefinition<&str, &[u8]> = TableDefinition::new("event_log");
const MEMBERS: TableDefinition<&str, &[u8]> = TableDefinition::new("members");
const ROLES: TableDefinition<&str, &[u8]> = TableDefinition::new("roles");
const HEARTBEATS: TableDefinition<&str, &[u8]> = TableDefinition::new("heartbeats");

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
    pub payload: Value,
}

pub struct RelationalEngine {
    db: Arc<Database>,
    pub search_engine: SearchEngine,
    pub schema_registry: SchemaRegistry,
}

impl RelationalEngine {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("syntrix_indexes.redb");
        let db = Database::create(db_path)?;

        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(DOCUMENTS)?;
            let _ = write_txn.open_table(INDEXES)?;
            let _ = write_txn.open_table(COMPOSITE)?;
            let _ = write_txn.open_table(HLC_TRACKER)?;
            let _ = write_txn.open_table(EVENT_LOG)?;
            let _ = write_txn.open_table(MEMBERS)?;
            let _ = write_txn.open_table(ROLES)?;
            let _ = write_txn.open_table(HEARTBEATS)?;
        }
        write_txn.commit()?;

        let schema_registry = build_registry();
        let search_engine = SearchEngine::new(data_dir.clone())?;

        Ok(Self { db: Arc::new(db), search_engine, schema_registry })
    }

    pub fn append_event(&self, org_id: &str, event_json: &Value) -> anyhow::Result<String> {
        let event_type = event_json["type"].as_str().unwrap_or("unknown");
        let _entity = entity_from_event_type(event_type);

        let hlc_val = &event_json["hlc"];
        let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
        let hlc_count = hlc_val["count"].as_u64().unwrap_or(0);
        let hlc_node = hlc_val["node"].as_str().unwrap_or("");

        let key = format!("evt:{}:{:020}:{:08}:{}", org_id, hlc_ts, hlc_count, hlc_node);

        let write_txn = self.db.begin_write()?;
        {
            let mut event_log = write_txn.open_table(EVENT_LOG)?;
            let bytes = serde_json::to_vec(event_json)?;
            event_log.insert(key.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;

        Ok(key)
    }

    pub fn query_events_since(
        &self,
        org_id: &str,
        cursor_ts: u64,
        limit: usize,
    ) -> anyhow::Result<Vec<EventEntry>> {
        let read_txn = self.db.begin_read()?;
        let event_log = read_txn.open_table(EVENT_LOG)?;

        let prefix = format!("evt:{}:", org_id);
        let range = event_log.range(prefix.as_str()..)?;

        let mut results = Vec::new();
        for item in range {
            let (key, value) = item?;
            let k = key.value();
            if !k.starts_with(&prefix) { break; }

            if let Ok(val) = serde_json::from_slice::<Value>(value.value()) {
                let hlc_ts = val["hlc"]["ts"].as_u64().unwrap_or(0);
                if hlc_ts <= cursor_ts { continue; }

                let event_type_str = val["type"].as_str().unwrap_or("");
                let entity = entity_from_event_type(event_type_str);
                let payload = val.get("payload").cloned().unwrap_or_default();

                results.push(EventEntry {
                    key: k.to_string(),
                    event_type: event_type_str.to_string(),
                    hlc: HlcTimestamp {
                        ts: hlc_ts,
                        count: val["hlc"]["count"].as_u64().unwrap_or(0) as u32,
                        node: val["hlc"]["node"].as_str().unwrap_or("").to_string(),
                    },
                    schema_version: val["schema_version"].as_u64().unwrap_or(1) as u32,
                    entity: entity.to_string(),
                    payload,
                });
            }

            if results.len() >= limit { break; }
        }

        Ok(results)
    }

    pub fn upsert_member(&self, org_id: &str, node_id: &str, member_json: &Value) -> anyhow::Result<()> {
        let key = format!("members:{}:{}", org_id, node_id);
        let write_txn = self.db.begin_write()?;
        {
            let mut members = write_txn.open_table(MEMBERS)?;
            members.insert(key.as_str(), serde_json::to_vec(member_json)?.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_members(&self, org_id: &str) -> anyhow::Result<Vec<Value>> {
        let read_txn = self.db.begin_read()?;
        let members = read_txn.open_table(MEMBERS)?;
        let prefix = format!("members:{}:", org_id);
        let range = members.range(prefix.as_str()..)?;

        let mut results = Vec::new();
        for item in range {
            let (key, value) = item?;
            if !key.value().starts_with(&prefix) { break; }
            if let Ok(val) = serde_json::from_slice::<Value>(value.value()) {
                results.push(val);
            }
        }
        Ok(results)
    }

    pub fn upsert_role_cfg(&self, org_id: &str, role_name: &str, role_json: &Value) -> anyhow::Result<()> {
        let key = format!("roles:{}:{}", org_id, role_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut roles = write_txn.open_table(ROLES)?;
            roles.insert(key.as_str(), serde_json::to_vec(role_json)?.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_roles(&self, org_id: &str) -> anyhow::Result<Vec<Value>> {
        let read_txn = self.db.begin_read()?;
        let roles = read_txn.open_table(ROLES)?;
        let prefix = format!("roles:{}:", org_id);
        let range = roles.range(prefix.as_str()..)?;

        let mut results = Vec::new();
        for item in range {
            let (key, value) = item?;
            if !key.value().starts_with(&prefix) { break; }
            if let Ok(val) = serde_json::from_slice::<Value>(value.value()) {
                results.push(val);
            }
        }
        Ok(results)
    }

    pub fn upsert_heartbeat(&self, org_id: &str, node_id: &str, hb_json: &Value) -> anyhow::Result<()> {
        let key = format!("heartbeat:{}:{}", org_id, node_id);
        let write_txn = self.db.begin_write()?;
        {
            let mut heartbeats = write_txn.open_table(HEARTBEATS)?;
            heartbeats.insert(key.as_str(), serde_json::to_vec(hb_json)?.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get_heartbeats(&self, org_id: &str) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        let read_txn = self.db.begin_read()?;
        let heartbeats = read_txn.open_table(HEARTBEATS)?;
        let prefix = format!("heartbeat:{}:", org_id);
        let range = heartbeats.range(prefix.as_str()..)?;

        let mut results = std::collections::HashMap::new();
        for item in range {
            let (key, value) = item?;
            let k = key.value();
            if !k.starts_with(&prefix) { break; }
            if let Some(node_id) = k.strip_prefix(&prefix) {
                if let Ok(val) = serde_json::from_slice::<Value>(value.value()) {
                    if let Some(ts) = val["ts"].as_i64() {
                        results.insert(node_id.to_string(), ts);
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn delete_member(&self, org_id: &str, node_id: &str) -> anyhow::Result<()> {
        let key = format!("members:{}:{}", org_id, node_id);
        let write_txn = self.db.begin_write()?;
        {
            let mut members = write_txn.open_table(MEMBERS)?;
            members.remove(key.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn delete_role(&self, org_id: &str, role_name: &str) -> anyhow::Result<()> {
        let key = format!("roles:{}:{}", org_id, role_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut roles = write_txn.open_table(ROLES)?;
            roles.remove(key.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn upsert_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &Value,
    ) -> anyhow::Result<()> {
        self.upsert_document_internal(org_id, entity, doc_id, json_payload, None)
    }

    pub fn upsert_document_with_hlc(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &Value,
        hlc: &HlcTimestamp,
    ) -> anyhow::Result<()> {
        let hlc_key = format!("hlc:{}:{}:{}", org_id, entity, doc_id);
        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(hlc_table) = read_txn.open_table(HLC_TRACKER) {
                if let Ok(Some(existing)) = hlc_table.get(hlc_key.as_str()) {
                    if let Ok(stored) = serde_json::from_slice::<HlcTimestamp>(existing.value()) {
                        if hlc <= &stored {
                            return Ok(());
                        }
                    }
                }
            }
        }
        self.upsert_document_internal(org_id, entity, doc_id, json_payload, Some(hlc))
    }

    fn upsert_document_internal(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &Value,
        hlc: Option<&HlcTimestamp>,
    ) -> anyhow::Result<()> {
        let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
        let schema = self.schema_registry.get(entity);

        let write_txn = self.db.begin_write()?;
        {
            let mut docs = write_txn.open_table(DOCUMENTS)?;
            let mut idx = write_txn.open_table(INDEXES)?;
            let mut comp = write_txn.open_table(COMPOSITE)?;
            let mut hlc_table = write_txn.open_table(HLC_TRACKER)?;

            if let Some(old_bytes) = docs.get(doc_key.as_str())? {
                if let Ok(old_json) = serde_json::from_slice::<Value>(old_bytes.value()) {
                    remove_old_indices(&mut idx, &mut comp, org_id, entity, doc_id, &old_json, schema);
                }
            }

            let new_bytes = serde_json::to_vec(json_payload)?;
            docs.insert(doc_key.as_str(), new_bytes.as_slice())?;

            if let Some(s) = schema {
                for field in &s.fields {
                    if !field.indexed { continue; }
                    if let Some(v) = json_payload.get(&field.name) {
                        let val_str = encode_value(v);
                        let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, field.name, val_str, doc_id);
                        let _ = idx.insert(idx_key.as_str(), &[] as &[u8]);
                    }
                }

                for index_def in &s.indexes {
                    let parts: Vec<String> = index_def.fields
                        .iter()
                        .filter_map(|f| json_payload.get(f))
                        .map(encode_value)
                        .collect();
                    if parts.len() == index_def.fields.len() {
                        let comp_key = format!("compidx:{}:{}:{}:{}:{}",
                            org_id, entity, index_def.name, parts.join(":"), doc_id);
                        let _ = comp.insert(comp_key.as_str(), &[] as &[u8]);
                    }
                }
            } else {
                if let Some(obj) = json_payload.as_object() {
                    for (k, v) in obj {
                        if v.is_string() || v.is_number() || v.is_boolean() {
                            let val_str = encode_value(v);
                            let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, k, val_str, doc_id);
                            let _ = idx.insert(idx_key.as_str(), &[] as &[u8]);
                        }
                    }
                }
            }

            if let Some(hlc_val) = hlc {
                let hlc_key = format!("hlc:{}:{}:{}", org_id, entity, doc_id);
                let _ = hlc_table.insert(hlc_key.as_str(), serde_json::to_vec(hlc_val)?.as_slice());
            }
        }
        write_txn.commit()?;

        if let Some(s) = schema {
            let searchable: Vec<&syntrix_schema::FieldSchema> = s.fields.iter().filter(|f| f.searchable).collect();
            let mut title = doc_id.to_string();
            let mut body_parts = Vec::new();

            for field in &searchable {
                if field.name == "name" || field.name == "title" {
                    if let Some(v) = json_payload.get(&field.name).and_then(|v| v.as_str()) {
                        title = v.to_string();
                        break;
                    }
                }
            }

            for field in &searchable {
                if let Some(v) = json_payload.get(&field.name) {
                    if v.is_string() || v.is_number() || v.is_boolean() {
                        body_parts.push(encode_value(v));
                    }
                }
            }

            let body = body_parts.join(" ");
            self.search_engine.index_document(org_id, entity, doc_id, &title, &body)?;
        } else {
            let mut title = doc_id.to_string();
            let mut body_parts = Vec::new();

            if let Some(obj) = json_payload.as_object() {
                for key in &["name", "title", "label"] {
                    if let Some(v) = obj.get(*key).and_then(|v| v.as_str()) {
                        title = v.to_string();
                        break;
                    }
                }
                for (_k, v) in obj {
                    if v.is_string() || v.is_number() || v.is_boolean() {
                        body_parts.push(encode_value(v));
                    }
                }
            }

            let body = body_parts.join(" ");
            self.search_engine.index_document(org_id, entity, doc_id, &title, &body)?;
        }

        Ok(())
    }

    pub fn query(
        &self,
        org_id: &str,
        entity: &str,
        options: &QueryOptions,
    ) -> anyhow::Result<Vec<Value>> {
        let read_txn = self.db.begin_read()?;
        let docs_table = read_txn.open_table(DOCUMENTS)?;
        let idx_table = read_txn.open_table(INDEXES)?;
        let comp_table = read_txn.open_table(COMPOSITE)?;

        let mut doc_ids: Vec<String> = Vec::new();

        if options.filters.is_empty() {
            let prefix = format!("doc:{}:{}:", org_id, entity);
            let range = docs_table.range(prefix.as_str()..)?;
            for item in range {
                let (key, _) = item?;
                let k = key.value();
                if !k.starts_with(&prefix) { break; }
                if let Some(did) = k.rsplit(':').next() {
                    doc_ids.push(did.to_string());
                }
            }
        } else if let Some(comp_match) = self.try_composite_index(org_id, entity, &options.filters) {
            let prefix = format!("compidx:{}:{}:{}:", org_id, entity, comp_match);
            let range = comp_table.range(prefix.as_str()..)?;
            for item in range {
                let (key, _) = item?;
                let k = key.value();
                if !k.starts_with(&prefix) { break; }
                if let Some(did) = k.rsplit(':').next() {
                    doc_ids.push(did.to_string());
                }
            }
        } else {
            for filter in &options.filters {
                let prefix = format!("idx:{}:{}:{}:{}:", org_id, entity, filter.field, filter.value);
                let mut ids = Vec::new();
                let range = idx_table.range(prefix.as_str()..)?;
                for item in range {
                    let (key, _) = item?;
                    let k = key.value();
                    if !k.starts_with(&prefix) { break; }
                    if let Some(did) = k.rsplit(':').next() {
                        ids.push(did.to_string());
                    }
                }
                if doc_ids.is_empty() {
                    doc_ids = ids;
                } else {
                    doc_ids = intersect_sorted(&doc_ids, &ids);
                }
                if doc_ids.is_empty() { break; }
            }
        }

        let mut results: Vec<Value> = Vec::new();
        for did in &doc_ids {
            let doc_key = format!("doc:{}:{}:{}", org_id, entity, did);
            if let Some(bytes) = docs_table.get(doc_key.as_str())? {
                if let Ok(json) = serde_json::from_slice::<Value>(bytes.value()) {
                    results.push(json);
                }
            }
        }

        if let Some(ref sort_field) = options.sort {
            let is_numeric = self.schema_registry.get(entity)
                .and_then(|s| s.field(&sort_field))
                .map(|f| f.field_type == FieldType::Number)
                .unwrap_or(false);
            if is_numeric {
                results.sort_by(|a, b| {
                    let a_val = a.get(sort_field).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let b_val = b.get(sort_field).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    a_val.total_cmp(&b_val)
                });
            } else {
                results.sort_by(|a, b| {
                    let a_val = a.get(sort_field).and_then(|v| v.as_str()).unwrap_or("");
                    let b_val = b.get(sort_field).and_then(|v| v.as_str()).unwrap_or("");
                    a_val.cmp(b_val)
                });
            }
        }

        let offset = options.offset.unwrap_or(0);
        let limit = options.limit.unwrap_or(usize::MAX);
        if offset > 0 && offset < results.len() {
            results = results.split_off(offset);
        }
        if results.len() > limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    fn try_composite_index(&self, _org_id: &str, entity: &str, filters: &[QueryFilter]) -> Option<String> {
        let schema = self.schema_registry.get(entity)?;
        let filter_fields: std::collections::HashSet<&str> = filters.iter().map(|f| f.field.as_str()).collect();
        for index_def in &schema.indexes {
            if index_def.fields.len() != filters.len() { continue; }
            let index_fields: std::collections::HashSet<&str> = index_def.fields.iter().map(|f| f.as_str()).collect();
            if filter_fields == index_fields {
                return Some(index_def.name.clone());
            }
        }
        None
    }

    pub fn get_document(&self, org_id: &str, entity: &str, doc_id: &str) -> anyhow::Result<Option<Value>> {
        let read_txn = self.db.begin_read()?;
        let docs_table = read_txn.open_table(DOCUMENTS)?;
        let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
        Ok(docs_table.get(doc_key.as_str())?
            .and_then(|v| serde_json::from_slice::<Value>(v.value()).ok()))
    }

    pub fn delete_document(&self, org_id: &str, entity: &str, doc_id: &str) -> anyhow::Result<()> {
        let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
        let schema = self.schema_registry.get(entity);

        let write_txn = self.db.begin_write()?;
        {
            let mut docs = write_txn.open_table(DOCUMENTS)?;
            let mut idx = write_txn.open_table(INDEXES)?;
            let mut comp = write_txn.open_table(COMPOSITE)?;

            if let Some(old_bytes) = docs.get(doc_key.as_str())? {
                if let Ok(old_json) = serde_json::from_slice::<Value>(old_bytes.value()) {
                    remove_old_indices(&mut idx, &mut comp, org_id, entity, doc_id, &old_json, schema);
                }
            }
            docs.remove(doc_key.as_str())?;
        }
        write_txn.commit()?;

        self.search_engine.delete_document(doc_id)?;
        Ok(())
    }
}

fn remove_old_indices(
    idx: &mut redb::Table<&str, &[u8]>,
    comp: &mut redb::Table<&str, &[u8]>,
    org_id: &str, entity: &str, doc_id: &str,
    old_json: &Value,
    schema: Option<&syntrix_schema::EntitySchema>,
) {
    match schema {
        Some(s) => {
            for field in &s.fields {
                if !field.indexed { continue; }
                if let Some(v) = old_json.get(&field.name) {
                    let val_str = encode_value(v);
                    let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, field.name, val_str, doc_id);
                    let _ = idx.remove(idx_key.as_str());
                }
            }
            for index_def in &s.indexes {
                let parts: Vec<String> = index_def.fields
                    .iter()
                    .filter_map(|f| old_json.get(f))
                    .map(encode_value)
                    .collect();
                if parts.len() == index_def.fields.len() {
                    let comp_key = format!("compidx:{}:{}:{}:{}:{}",
                        org_id, entity, index_def.name, parts.join(":"), doc_id);
                    let _ = comp.remove(comp_key.as_str());
                }
            }
        }
        None => {
            if let Some(obj) = old_json.as_object() {
                for (k, v) in obj {
                    if v.is_string() || v.is_number() || v.is_boolean() {
                        let val_str = encode_value(v);
                        let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, k, val_str, doc_id);
                        let _ = idx.remove(idx_key.as_str());
                    }
                }
            }
        }
    }
}

fn intersect_sorted(a: &[String], b: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                result.push(a[i].clone());
                i += 1;
                j += 1;
            }
        }
    }
    result
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
