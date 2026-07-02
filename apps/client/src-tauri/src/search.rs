use std::num::NonZero;
use std::sync::Arc;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SearchResult {
    pub doc_id: String,
    pub entity: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

pub struct SearchEngine {
    pub conn: Arc<turso_core::Connection>,
}

impl SearchEngine {
    pub fn new(conn: Arc<turso_core::Connection>) -> Self {
        Self { conn }
    }

    /// FTS is driven directly by turso's `fts_match`/`fts_score` scalar functions over the real
    /// typed columns declared as `searchable` in schema.json (Fase 2, tarea 12) — turso's "fts"
    /// cargo feature integrates Tantivy as a set of SQL functions rather than SQLite's FTS5
    /// virtual-table syntax, so there is no separate shadow table to maintain; `index_document`/
    /// `delete_document` are no-ops kept only for API compatibility with earlier callers.
    pub fn index_document(
        &self,
        _org_id: &str,
        _entity: &str,
        _doc_id: &str,
        _title: &str,
        _body: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn delete_document(&self, _doc_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn search(
        &self,
        org_id: &str,
        query_str: &str,
        entities: Option<Vec<String>>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let tables: Vec<&str> = match entities {
            Some(ref ents) if !ents.is_empty() => ents.iter().map(|e| e.as_str()).collect(),
            _ => syntrix_core::all_entity_tables(),
        };

        let mut results: Vec<SearchResult> = Vec::new();

        for table in tables {
            let meta = match syntrix_core::entity_meta(table) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let searchable = syntrix_core::searchable_columns(meta);
            if searchable.is_empty() {
                continue;
            }

            let cols_csv = searchable.join(", ");
            // fts_match(col1, .., colN, query) returns 1 if any column matches; fts_score(...)
            // ranks results when routed through an FTS index (falls back to 0.0 without one).
            let sql = format!(
                "SELECT doc_id, {cols}, fts_score({cols}, ?1) AS score \
                 FROM {table} WHERE org_id = ?2 AND fts_match({cols}, ?1)",
                cols = cols_csv,
                table = meta.table,
            );

            let mut stmt = match self.conn.prepare(&sql) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("search: prepare failed for table {}: {}", table, e);
                    continue;
                }
            };

            if let Err(e) = stmt.bind_at(NonZero::new(1).unwrap(), turso_core::Value::from_text(query_str.to_string())) {
                tracing::warn!("search: bind query failed for table {}: {}", table, e);
                continue;
            }
            if let Err(e) = stmt.bind_at(NonZero::new(2).unwrap(), turso_core::Value::from_text(org_id.to_string())) {
                tracing::warn!("search: bind org_id failed for table {}: {}", table, e);
                continue;
            }

            loop {
                match stmt.step() {
                    Ok(turso_core::StepResult::Row) => {}
                    Ok(turso_core::StepResult::IO) | Ok(turso_core::StepResult::Yield) => continue,
                    Ok(turso_core::StepResult::Done) => break,
                    Ok(turso_core::StepResult::Interrupt) | Ok(turso_core::StepResult::Busy) => break,
                    Err(e) => {
                        tracing::warn!("search: step failed for table {}: {}", table, e);
                        break;
                    }
                }

                let row = match stmt.row() {
                    Some(r) => r,
                    None => continue,
                };

                let doc_id: String = match row.get(0) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Build a title/snippet from the searchable column values (column 1..=N).
                let mut parts: Vec<String> = Vec::with_capacity(searchable.len());
                for i in 0..searchable.len() {
                    let v: &turso_core::Value = match row.get(i + 1) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if let turso_core::Value::Text(t) = v {
                        if !t.as_str().is_empty() {
                            parts.push(t.as_str().to_string());
                        }
                    }
                }
                let title = parts.first().cloned().unwrap_or_default();
                let body = parts.join(" ");
                let snippet = if body.len() > 200 { format!("{}...", &body[..200]) } else { body };

                let score_idx = searchable.len() + 1;
                let score: f64 = row.get(score_idx).unwrap_or(0.0);

                results.push(SearchResult {
                    doc_id,
                    entity: table.to_string(),
                    title,
                    snippet,
                    score: score as f32,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> (impl Drop, Arc<turso_core::Connection>) {
        let (dir, conn) = syntrix_testkit::temp_limbo_db();
        crate::storage::run_migrations(&conn).expect("run_migrations");
        (dir, conn)
    }

    #[test]
    fn search_matches_real_typed_columns_via_fts_functions() {
        let (_dir, conn) = test_engine();
        let indexer = crate::indexes::SqlEngine::with_connection(conn.clone());
        indexer.upsert_document("org1", "customers", "c1", &serde_json::json!({
            "name": "Acme Corporation", "email": "billing@acme.test",
        })).unwrap();
        indexer.upsert_document("org1", "customers", "c2", &serde_json::json!({
            "name": "Globex Inc", "email": "billing@globex.test",
        })).unwrap();

        let engine = SearchEngine::new(conn);
        let results = engine.search("org1", "Acme", Some(vec!["customers".to_string()]), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "c1");
        assert_eq!(results[0].entity, "customers");
    }

    #[test]
    fn search_scopes_by_org_id() {
        let (_dir, conn) = test_engine();
        let indexer = crate::indexes::SqlEngine::with_connection(conn.clone());
        indexer.upsert_document("org1", "customers", "c1", &serde_json::json!({ "name": "Acme" })).unwrap();
        indexer.upsert_document("org2", "customers", "c2", &serde_json::json!({ "name": "Acme" })).unwrap();

        let engine = SearchEngine::new(conn);
        let results = engine.search("org1", "Acme", Some(vec!["customers".to_string()]), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "c1");
    }
}
