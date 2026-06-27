use redb::{Database, TableDefinition, ReadableTable};
use std::path::PathBuf;
use serde_json::Value;
use std::sync::Arc;
use crate::search::SearchEngine;
use syntrix_schema::{build_registry, SchemaRegistry, FieldType, encoded::encode_value};

/// HLC tracker for LWW conflict resolution.
/// Key = `hlc:{org_id}:{entity}:{doc_id}` → JSON of Hlc { ts, count, node }
const HLC_TRACKER: TableDefinition<&str, &[u8]> = TableDefinition::new("hlc_tracker");

/// Secondary (single-field) index.
/// Key = `idx:{org_id}:{entity}:{field}:{encoded_value}:{doc_id}` → empty
const INDEXES: TableDefinition<&str, &[u8]> = TableDefinition::new("indexes");

/// Composite index.
/// Key = `compidx:{org_id}:{entity}:{name}:{v1_enc}:{v2_enc}:...:{doc_id}` → empty
const COMPOSITE: TableDefinition<&str, &[u8]> = TableDefinition::new("composite");

/// Document store.
/// Key = `doc:{org_id}:{entity}:{doc_id}` → JSON bytes
const DOCUMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("documents");

/// Query filter: exact match on a field.
#[derive(Debug, Clone)]
pub struct QueryFilter {
    pub field: String,
    pub value: String,
}

/// Extended query options.
#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub filters: Vec<QueryFilter>,
    /// Sort by this field name (requires the field to be indexed)
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self { filters: vec![], sort: None, limit: None, offset: None }
    }
}

/// HLC timestamp for LWW conflict resolution (mirrors events::Hlc without importing it).
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
        }
        write_txn.commit()?;

        let schema_registry = build_registry();
        let search_engine = SearchEngine::new(data_dir.clone())?;

        Ok(Self { db: Arc::new(db), search_engine, schema_registry })
    }

    /// Upsert a document (legacy: no HLC conflict resolution).
    /// New callers should prefer `upsert_document_with_hlc`.
    pub fn upsert_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &Value,
    ) -> anyhow::Result<()> {
        self.upsert_document_internal(org_id, entity, doc_id, json_payload, None)
    }

    /// Upsert a document with HLC-based conflict resolution (LWW).
    /// If the incoming HLC is not greater than the stored HLC for the same
    /// (org, entity, doc_id), the write is skipped.
    pub fn upsert_document_with_hlc(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        json_payload: &Value,
        hlc: &HlcTimestamp,
    ) -> anyhow::Result<()> {
        let hlc_key = format!("hlc:{}:{}:{}", org_id, entity, doc_id);
        // Check existing HLC — skip if incoming is not strictly greater
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

    /// Internal upsert: writes DOCUMENTS + INDEXES + COMPOSITE + Tantivy
    /// using the schema registry to determine which fields to index.
    /// When `hlc` is `Some`, persists the HLC for LWW conflict resolution.
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

            // 1. Remove old indices if the document existed
            if let Some(old_bytes) = docs.get(doc_key.as_str())? {
                if let Ok(old_json) = serde_json::from_slice::<Value>(old_bytes.value()) {
                    remove_old_indices(&mut idx, &mut comp, org_id, entity, doc_id, &old_json, schema);
                }
            }

            // 2. Store the new document
            let new_bytes = serde_json::to_vec(json_payload)?;
            docs.insert(doc_key.as_str(), new_bytes.as_slice())?;

            // 3. Insert secondary indexes for declared #[indexed] fields
            if let Some(s) = schema {
                for field in &s.fields {
                    if !field.indexed { continue; }
                    if let Some(v) = json_payload.get(&field.name) {
                        let val_str = if field.field_type == FieldType::Number {
                            // Always encode numbers through our sortable path
                            encode_value(v)
                        } else {
                            encode_value(v)
                        };
                        let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, field.name, val_str, doc_id);
                        let _ = idx.insert(idx_key.as_str(), &[] as &[u8]);
                    }
                }

                // 4. Composite indexes
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
                // Legacy: index all top-level string/number/bool fields
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
            // 5. Persist HLC for LWW tracking (if provided)
            if let Some(hlc_val) = hlc {
                let hlc_key = format!("hlc:{}:{}:{}", org_id, entity, doc_id);
                let _ = hlc_table.insert(hlc_key.as_str(), serde_json::to_vec(hlc_val)?.as_slice());
            }
        }
        write_txn.commit()?;

        // 6. Tantivy indexing: only #[searchable] fields
        if let Some(s) = schema {
            let searchable: Vec<&syntrix_schema::FieldSchema> = s.fields.iter().filter(|f| f.searchable).collect();
            let mut title = doc_id.to_string();
            let mut body_parts = Vec::new();

            // Title heuristic: first searchable field named "name" or "title"
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
            // Legacy fallback: index entire body
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

    /// Query documents using secondary indices.
    ///
    /// Supports single-field filter, multi-filter via composite index,
    /// optional sort (in-memory for now), and pagination.
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

        // Collect candidate doc_ids
        let mut doc_ids: Vec<String> = Vec::new();

        if options.filters.is_empty() {
            // Full table scan for this entity
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
            // Use composite index (fastest path)
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
            // Single filter or intersection: use INDEXES table
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
                    // Intersect with previous results
                    doc_ids = intersect_sorted(&doc_ids, &ids);
                }
                if doc_ids.is_empty() { break; }
            }
        }

        // Resolve doc_ids to full documents
        let mut results: Vec<Value> = Vec::new();
        for did in &doc_ids {
            let doc_key = format!("doc:{}:{}:{}", org_id, entity, did);
            if let Some(bytes) = docs_table.get(doc_key.as_str())? {
                if let Ok(json) = serde_json::from_slice::<Value>(bytes.value()) {
                    results.push(json);
                }
            }
        }

        // Sort in-memory if sort field is specified
        if let Some(ref sort_field) = options.sort {
            let is_numeric = self.schema_registry.get(entity)
                .and_then(|s| s.field(sort_field))
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

        // Pagination
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

    /// Check if the filters match a composite index; returns the index name if so.
    /// Order-insensitive: matches when filter fields are the same set as index fields.
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

    /// Get a single document by its entity + doc_id (O(1)).
    pub fn get_document(&self, org_id: &str, entity: &str, doc_id: &str) -> anyhow::Result<Option<Value>> {
        let read_txn = self.db.begin_read()?;
        let docs_table = read_txn.open_table(DOCUMENTS)?;
        let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
        Ok(docs_table.get(doc_key.as_str())?
            .and_then(|v| serde_json::from_slice::<Value>(v.value()).ok()))
    }

    /// Delete a document and all its index entries.
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

/// Remove all index entries for an old document.
fn remove_old_indices(
    idx: &mut redb::Table<&str, &[u8]>,
    comp: &mut redb::Table<&str, &[u8]>,
    org_id: &str, entity: &str, doc_id: &str,
    old_json: &Value,
    schema: Option<&syntrix_schema::EntitySchema>,
) {
    match schema {
        Some(s) => {
            // Remove single-field indices for declared #[indexed] fields
            for field in &s.fields {
                if !field.indexed { continue; }
                if let Some(v) = old_json.get(&field.name) {
                    let val_str = encode_value(v);
                    let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, field.name, val_str, doc_id);
                    let _ = idx.remove(idx_key.as_str());
                }
            }
            // Remove composite index entries
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
            // Legacy: remove all top-level fields
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

/// Simple sorted intersection (assumes both lists are sorted by string ordering).
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
