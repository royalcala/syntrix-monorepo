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

const ALL_FTS_TABLES: &[&str] = &[
    "customers",
    "suppliers",
    "products",
    "invoices",
    "orders",
    "payroll",
];

impl SearchEngine {
    pub fn new(conn: Arc<turso_core::Connection>) -> Self {
        Self { conn }
    }

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
            Some(ref ents) if !ents.is_empty() => {
                ents.iter().map(|e| e.as_str()).collect()
            }
            _ => ALL_FTS_TABLES.to_vec(),
        };

        let mut results: Vec<SearchResult> = Vec::new();

        for table in tables {
            let sql = format!(
                "SELECT doc_id, fts_title, fts_body FROM {} WHERE (fts_title, fts_body) MATCH ? AND org_id = ?",
                table
            );

            let mut stmt = match self.conn.prepare(&sql) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("search: prepare failed for table {}: {}", table, e);
                    continue;
                }
            };

            if let Err(e) = stmt.bind_at(
                NonZero::new(1).unwrap(),
                turso_core::Value::from_text(query_str.to_string()),
            ) {
                tracing::warn!("search: bind query failed for table {}: {}", table, e);
                continue;
            }

            if let Err(e) = stmt.bind_at(
                NonZero::new(2).unwrap(),
                turso_core::Value::from_text(org_id.to_string()),
            ) {
                tracing::warn!("search: bind org_id failed for table {}: {}", table, e);
                continue;
            }

            loop {
                match stmt.step() {
                    Ok(turso_core::StepResult::Row) => {}
                    Ok(turso_core::StepResult::IO) | Ok(turso_core::StepResult::Yield) => {
                        continue;
                    }
                    Ok(turso_core::StepResult::Done) => break,
                    Ok(turso_core::StepResult::Interrupt) | Ok(turso_core::StepResult::Busy) => {
                        break;
                    }
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
                let fts_title: String = match row.get(1) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let fts_body: String = match row.get(2) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let snippet = if fts_body.len() > 200 {
                    format!("{}...", &fts_body[..200])
                } else {
                    fts_body.clone()
                };

                results.push(SearchResult {
                    doc_id,
                    entity: table.to_string(),
                    title: fts_title,
                    snippet,
                    score: 1.0,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }
}
