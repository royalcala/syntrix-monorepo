use redb::{Database, TableDefinition, ReadableTable};
use std::path::PathBuf;
use serde_json::Value;
use std::sync::Arc;
use crate::search::SearchEngine;

// Table: Key = "doc:{org_id}:{entity}:{doc_id}", Value = JSON bytes
const DOCUMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("documents");

// Table: Key = "idx:{org_id}:{entity}:{field}:{value}:{doc_id}", Value = empty
const INDEXES: TableDefinition<&str, &[u8]> = TableDefinition::new("indexes");

pub struct RelationalEngine {
    db: Arc<Database>,
    pub search_engine: SearchEngine,
}


impl RelationalEngine {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("syntrix_indexes.redb");
        
        let db = Database::create(db_path)?;
        
        // Ensure tables exist
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(DOCUMENTS)?;
            let _ = write_txn.open_table(INDEXES)?;
        }
        write_txn.commit()?;

        let search_engine = SearchEngine::new(data_dir.clone())?;

        Ok(Self {
            db: Arc::new(db),
            search_engine,
        })
    }


    /// Upserts a document and automatically maintains its secondary indices
    pub fn upsert_document(
        &self, org_id: &str, entity: &str, doc_id: &str, json_payload: &Value
    ) -> anyhow::Result<()> {
        let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
        
        let write_txn = self.db.begin_write()?;
        {
            let mut docs_table = write_txn.open_table(DOCUMENTS)?;
            let mut idx_table = write_txn.open_table(INDEXES)?;

            // 1. Remove old indices if document existed
            if let Some(old_bytes) = docs_table.get(doc_key.as_str())? {
                if let Ok(old_json) = serde_json::from_slice::<Value>(old_bytes.value()) {
                    if let Some(obj) = old_json.as_object() {
                        for (k, v) in obj {
                            let val_str = value_to_string(v);
                            let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, k, val_str, doc_id);
                            idx_table.remove(idx_key.as_str())?;
                        }
                    }
                }
            }

            // 2. Insert new document
            let new_bytes = serde_json::to_vec(json_payload)?;
            docs_table.insert(doc_key.as_str(), new_bytes.as_slice())?;

            // 3. Insert new indices (indexing every top-level string, number, or boolean field)
            if let Some(obj) = json_payload.as_object() {
                for (k, v) in obj {
                    if v.is_string() || v.is_number() || v.is_boolean() {
                        let val_str = value_to_string(v);
                        let idx_key = format!("idx:{}:{}:{}:{}:{}", org_id, entity, k, val_str, doc_id);
                        idx_table.insert(idx_key.as_str(), &[] as &[u8])?;
                    }
                }
            }
        write_txn.commit()?;

        // 4. Index in Tantivy full-text search
        let mut title = doc_id.to_string();
        let mut body_parts = Vec::new();

        if let Some(obj) = json_payload.as_object() {
            for key in &["name", "title", "label"] {
                if let Some(v) = obj.get(*key) {
                    if let Some(s) = v.as_str() {
                        title = s.to_string();
                        break;
                    }
                }
            }

            for (_k, v) in obj {
                if v.is_string() || v.is_number() || v.is_boolean() {
                    let val_str = value_to_string(v);
                    body_parts.push(val_str);
                }
            }
        }

        let body = body_parts.join(" ");
        self.search_engine.index_document(org_id, entity, doc_id, &title, &body)?;

        Ok(())
    }


    /// Query documents using primary prefix or secondary indices
    pub fn query(
        &self, org_id: &str, entity: &str, filter_field: Option<&str>, filter_value: Option<&str>
    ) -> anyhow::Result<Vec<Value>> {
        let read_txn = self.db.begin_read()?;
        let docs_table = read_txn.open_table(DOCUMENTS)?;
        
        let mut results = Vec::new();

        if let (Some(field), Some(val)) = (filter_field, filter_value) {
            // Use secondary index
            let idx_table = read_txn.open_table(INDEXES)?;
            let prefix = format!("idx:{}:{}:{}:{}:", org_id, entity, field, val);
            
            let range = idx_table.range(prefix.as_str()..)?;
            for item in range {
                let (key, _) = item?;
                let k = key.value();
                if !k.starts_with(&prefix) { break; }
                
                // Extract doc_id from key (last segment)
                if let Some(doc_id) = k.split(':').last() {
                    let doc_key = format!("doc:{}:{}:{}", org_id, entity, doc_id);
                    if let Some(doc_bytes) = docs_table.get(doc_key.as_str())? {
                        if let Ok(json) = serde_json::from_slice::<Value>(doc_bytes.value()) {
                            results.push(json);
                        }
                    }
                }
            }
        } else {
            // Full table scan for this entity
            let prefix = format!("doc:{}:{}:", org_id, entity);
            let range = docs_table.range(prefix.as_str()..)?;
            for item in range {
                let (key, value) = item?;
                let k = key.value();
                if !k.starts_with(&prefix) { break; }
                
                if let Ok(json) = serde_json::from_slice::<Value>(value.value()) {
                    results.push(json);
                }
            }
        }

        Ok(results)
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_lowercase(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => "".to_string(),
    }
}
