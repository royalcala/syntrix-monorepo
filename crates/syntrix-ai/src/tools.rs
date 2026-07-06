use serde::{Deserialize, Serialize};

use syntrix_core::schema;
use syntrix_core::schema::{entity_meta, EntityMeta};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntity {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub children: Vec<ChildTableInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub ty: String,
    pub nullable: bool,
    pub searchable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildTableInfo {
    pub table: String,
    pub parent_key: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    pub id: String,
    pub org_id: String,
    pub sql: String,
    pub entity: String,
    pub components: serde_json::Value,
    pub root: String,
    pub meta: ViewMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewMeta {
    pub natural_language_query: String,
    pub created_by: String,
    pub created_at: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entity: String,
    pub doc_id: String,
    pub score: f64,
    pub snippet: String,
}

pub trait AiContext: Send + Sync {
    fn check_read_access(&self, org_id: &str, entity: &str) -> Result<(), String>;

    fn query_entity(
        &self,
        org_id: &str,
        sql: &str,
        params: &[String],
    ) -> Result<QueryResult, String>;

    fn search_entity(
        &self,
        org_id: &str,
        query: &str,
        entities: Option<Vec<String>>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, String>;

    fn save_view(&self, view: &ViewDefinition) -> Result<(), String>;

    fn list_views(&self, org_id: &str, tags: Option<&[String]>) -> Result<Vec<ViewDefinition>, String>;
}

fn collect_entity_info(name: &str) -> Result<SchemaEntity, String> {
    let meta: &EntityMeta = entity_meta(name).map_err(|e| format!("entity not found: {e}"))?;
    let cols: Vec<ColumnInfo> = meta
        .columns
        .iter()
        .map(|c| ColumnInfo {
            name: c.name.clone(),
            ty: format!("{:?}", c.ty).to_lowercase(),
            nullable: c.nullable,
            searchable: c.searchable,
        })
        .collect();
    let children: Vec<ChildTableInfo> = meta
        .children
        .iter()
        .map(|ch| ChildTableInfo {
            table: ch.table.clone(),
            parent_key: ch.parent_key.clone(),
            columns: ch
                .columns
                .iter()
                .map(|c| ColumnInfo {
                    name: c.name.clone(),
                    ty: format!("{:?}", c.ty).to_lowercase(),
                    nullable: c.nullable,
                    searchable: c.searchable,
                })
                .collect(),
        })
        .collect();
    Ok(SchemaEntity {
        name: meta.table.clone(),
        columns: cols,
        children,
    })
}

pub fn get_schema_impl(entity_filter: Option<&str>) -> Result<Vec<SchemaEntity>, String> {
    match entity_filter {
        Some(name) => Ok(vec![collect_entity_info(name)?]),
        None => {
            let tables = schema::all_entity_tables();
            tables.iter().map(|t| collect_entity_info(t)).collect()
        }
    }
}

pub fn query_entity_tool(ctx: &dyn AiContext, org_id: &str, sql: &str, params: &[String]) -> Result<QueryResult, String> {
    if sql.trim().to_uppercase().starts_with("SELECT")
        || sql.trim().to_uppercase().starts_with("WITH")
    {
        ctx.query_entity(org_id, sql, params)
    } else {
        Err("Only SELECT/WITH queries are allowed. Writes are not permitted via AI tools.".to_string())
    }
}

pub fn search_entity_tool(
    ctx: &dyn AiContext,
    org_id: &str,
    query: &str,
    entities: Option<Vec<String>>,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let table = ctx
        .query_entity(org_id, "SELECT name FROM sqlite_master WHERE type='table' LIMIT 1", &[])
        .map_err(|e| format!("no database access: {e}"))?;
    if table.rows.is_empty() {
        return Err("No database connection".to_string());
    }
    ctx.search_entity(org_id, query, entities, limit)
}

pub fn save_view_tool(ctx: &dyn AiContext, view: &ViewDefinition) -> Result<(), String> {
    ctx.check_read_access(&view.org_id, &view.entity)?;
    ctx.save_view(view)
}

pub fn list_views_tool(
    ctx: &dyn AiContext,
    org_id: &str,
    tags: Option<&[String]>,
) -> Result<Vec<ViewDefinition>, String> {
    ctx.check_read_access(org_id, "view_definitions")?;
    ctx.list_views(org_id, tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockContext {
        views: std::sync::Mutex<Vec<ViewDefinition>>,
        allow_access: bool,
    }

    impl MockContext {
        fn new(allow_access: bool) -> Self {
            Self {
                views: std::sync::Mutex::new(vec![]),
                allow_access,
            }
        }
    }

    impl AiContext for MockContext {
        fn check_read_access(&self, _org_id: &str, _entity: &str) -> Result<(), String> {
            if self.allow_access {
                Ok(())
            } else {
                Err("access denied".to_string())
            }
        }

        fn query_entity(&self, _org_id: &str, sql: &str, _params: &[String]) -> Result<QueryResult, String> {
            if sql.contains("sqlite_master") {
                return Ok(QueryResult {
                    columns: vec!["name".to_string()],
                    rows: vec![vec![serde_json::Value::String("test".to_string())]],
                });
            }
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
            })
        }

        fn search_entity(
            &self,
            _org_id: &str,
            _query: &str,
            _entities: Option<Vec<String>>,
            _limit: usize,
        ) -> Result<Vec<SearchResult>, String> {
            Ok(vec![])
        }

        fn save_view(&self, view: &ViewDefinition) -> Result<(), String> {
            let mut v = self.views.lock().map_err(|e| e.to_string())?;
            v.push(view.clone());
            Ok(())
        }

        fn list_views(&self, org_id: &str, _tags: Option<&[String]>) -> Result<Vec<ViewDefinition>, String> {
            let v = self.views.lock().map_err(|e| e.to_string())?;
            Ok(v.iter().filter(|vd| vd.org_id == org_id).cloned().collect())
        }
    }

    #[test]
    fn test_get_schema_returns_all_entities() {
        let entities = get_schema_impl(None).unwrap();
        assert!(entities.len() >= 6);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"customers"));
        assert!(names.contains(&"invoices"));
    }

    #[test]
    fn test_get_schema_filters_by_entity() {
        let entities = get_schema_impl(Some("invoices")).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "invoices");
    }

    #[test]
    fn test_get_schema_returns_children() {
        let entities = get_schema_impl(Some("invoices")).unwrap();
        assert!(!entities[0].children.is_empty());
        assert_eq!(entities[0].children[0].table, "invoice_items");
    }

    #[test]
    fn test_query_entity_validates_select_only() {
        let ctx = MockContext::new(true);
        let result = query_entity_tool(&ctx, "org", "DELETE FROM customers", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Only SELECT/WITH"));
    }

    #[test]
    fn test_query_entity_allows_select() {
        let ctx = MockContext::new(true);
        let result = query_entity_tool(&ctx, "org", "SELECT * FROM customers", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_view_checks_access() {
        let ctx = MockContext::new(false);
        let view = ViewDefinition {
            id: "test".to_string(),
            org_id: "org-1".to_string(),
            sql: "SELECT * FROM customers".to_string(),
            entity: "customers".to_string(),
            components: serde_json::Value::Null,
            root: "root".to_string(),
            meta: ViewMeta {
                natural_language_query: "test".to_string(),
                created_by: "node-1".to_string(),
                created_at: 1000,
                tags: vec![],
            },
        };
        let result = save_view_tool(&ctx, &view);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_view_and_list() {
        let ctx = MockContext::new(true);
        let view = ViewDefinition {
            id: "v1".to_string(),
            org_id: "org-1".to_string(),
            sql: "SELECT * FROM customers".to_string(),
            entity: "customers".to_string(),
            components: serde_json::Value::Null,
            root: "root".to_string(),
            meta: ViewMeta {
                natural_language_query: "test".to_string(),
                created_by: "node-1".to_string(),
                created_at: 1000,
                tags: vec!["clientes".to_string()],
            },
        };
        save_view_tool(&ctx, &view).unwrap();

        let views = list_views_tool(&ctx, "org-1", None).unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, "v1");
    }

    #[test]
    fn test_list_views_checks_access() {
        let ctx = MockContext::new(false);
        let result = list_views_tool(&ctx, "org-1", None);
        assert!(result.is_err());
    }
}
