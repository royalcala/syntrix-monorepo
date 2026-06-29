use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Primitive field types that can be stored in entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Date,
    /// References another entity (foreign key)
    Relation,
    /// Monotonically increasing auto-generated key (e.g. folio numbers)
    AutoNumber,
}

/// Describes a relation (foreign key) from one entity field to another entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationDef {
    /// Target entity name (e.g. "customers")
    pub target: String,
    /// Field on the target entity to join on (e.g. "id")
    pub field: String,
}

/// Descriptor for a single field in an entity schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    pub field_type: FieldType,
    /// Create a secondary index in redb for this field (equality/prefix lookups)
    pub indexed: bool,
    /// Index this field in Tantivy for full-text/fuzzy search
    pub searchable: bool,
    /// Default sort column for list queries on this entity
    pub sort_key: bool,
    /// If this field references another entity, describe the relation
    pub relation: Option<RelationDef>,
}

/// Defines a secondary index, possibly composite (multiple fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDef {
    pub name: String,
    pub fields: Vec<String>,
}

/// Complete schema definition for a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub name: String,
    pub version: u32,
    pub fields: Vec<FieldSchema>,
    /// Explicit index definitions (including composite indexes)
    pub indexes: Vec<IndexDef>,
}

impl EntitySchema {
    /// Returns the field schema by name, if it exists.
    pub fn field(&self, name: &str) -> Option<&FieldSchema> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// List of fields that have secondary indexes.
    pub fn indexed_fields(&self) -> Vec<&FieldSchema> {
        self.fields.iter().filter(|f| f.indexed).collect()
    }

    /// List of fields that are full-text searchable via Tantivy.
    pub fn searchable_fields(&self) -> Vec<&FieldSchema> {
        self.fields.iter().filter(|f| f.searchable).collect()
    }

    /// The sort key field, if any.
    pub fn sort_key_field(&self) -> Option<&FieldSchema> {
        self.fields.iter().find(|f| f.sort_key)
    }
}

/// Central registry of all entity schemas in the system.
/// Schemas are defined once in Rust (compile-time) and this registry
/// can be queried at runtime for index generation, Tantivy schema building,
/// UI codegen, or export to JSON Schema.
pub struct SchemaRegistry {
    entities: HashMap<String, EntitySchema>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        SchemaRegistry {
            entities: HashMap::new(),
        }
    }

    pub fn register(&mut self, schema: EntitySchema) {
        self.entities.insert(schema.name.clone(), schema);
    }

    pub fn get(&self, name: &str) -> Option<&EntitySchema> {
        self.entities.get(name)
    }

    pub fn all(&self) -> Vec<&EntitySchema> {
        self.entities.values().collect()
    }

    /// Return entity names.
    pub fn entity_names(&self) -> Vec<&str> {
        self.entities.keys().map(|s| s.as_str()).collect()
    }

    /// Export the full registry as a JSON-serializable value (for Tauri IPC to frontend).
    pub fn export_json(&self) -> serde_json::Value {
        let all: Vec<&EntitySchema> = self.all();
        serde_json::to_value(all).unwrap_or_default()
    }
}

/// Check if an entity is in a permission list.
/// Returns true if the list contains the entity name or `"*"`.
pub fn can_access(allowed: &[String], entity: &str) -> bool {
    allowed.iter().any(|a| a == entity || a == "*")
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
