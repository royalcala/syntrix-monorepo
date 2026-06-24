use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term, SnippetGenerator, TantivyDocument};
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::collector::TopDocs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    pub doc_id: String,
    pub entity: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Clone)]
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    org_id: Field,
    entity: Field,
    doc_id: Field,
    title: Field,
    body: Field,
}

impl SearchEngine {
    pub fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let index_dir = data_dir.join("search_index");
        std::fs::create_dir_all(&index_dir)?;

        let mut schema_builder = Schema::builder();
        let org_id = schema_builder.add_text_field("org_id", STRING | STORED);
        let entity = schema_builder.add_text_field("entity", STRING | STORED);
        let doc_id = schema_builder.add_text_field("doc_id", STRING | STORED);
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let body = schema_builder.add_text_field("body", TEXT | STORED);
        let schema = schema_builder.build();

        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(&index_dir)?, schema)?;
        
        // Initialize reader
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // Initialize single writer (15MB heap memory budget)
        let writer = index.writer(15_000_000)?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            org_id,
            entity,
            doc_id,
            title,
            body,
        })
    }

    /// Indexes or updates a document.
    pub fn index_document(
        &self,
        org_id: &str,
        entity: &str,
        doc_id: &str,
        title: &str,
        body: &str,
    ) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("writer lock failed: {}", e))?;

        // 1. Delete previous version (based on the unique key combination)
        // Tantivy deletes are done via Terms. We can build a term representing org_id + entity + doc_id.
        // Since we want to delete exactly this document, we delete by doc_id term.
        // In our case, doc_id is unique across the org/entity. Let's delete by doc_id.
        let term = Term::from_field_text(self.doc_id, doc_id);
        writer.delete_term(term);

        // 2. Insert new document
        writer.add_document(doc!(
            self.org_id => org_id,
            self.entity => entity,
            self.doc_id => doc_id,
            self.title => title,
            self.body => body,
        ))?;

        // Commit immediately so changes are visible in real-time
        writer.commit()?;
        Ok(())
    }

    /// Removes a document from the index.
    pub fn delete_document(&self, doc_id: &str) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("writer lock failed: {}", e))?;
        let term = Term::from_field_text(self.doc_id, doc_id);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    /// Performs search with exact/fuzzy terms and highlights
    pub fn search(
        &self,
        org_id: &str,
        query_str: &str,
        entities: Option<Vec<String>>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        let mut subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![];

        // 1. Must belong to the active organization
        subqueries.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(self.org_id, org_id),
                IndexRecordOption::Basic,
            )),
        ));

        // 2. Must belong to one of the specified entities (if provided)
        if let Some(ents) = entities {
            if !ents.is_empty() {
                let mut entity_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![];
                for ent in ents {
                    entity_queries.push((
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(self.entity, &ent),
                            IndexRecordOption::Basic,
                        )),
                    ));
                }
                subqueries.push((Occur::Must, Box::new(BooleanQuery::new(entity_queries))));
            }
        }

        // 3. User query matching
        let trimmed_query = query_str.trim();
        if !trimmed_query.is_empty() {
            let query_parser = QueryParser::for_index(&self.index, vec![self.title, self.body]);
            
            // Try parsing normal query syntax. If it fails, fallback to simple term search
            let user_query = if let Ok(q) = query_parser.parse_query(trimmed_query) {
                q
            } else {
                Box::new(TermQuery::new(
                    Term::from_field_text(self.body, trimmed_query),
                    IndexRecordOption::WithFreqsAndPositions,
                ))
            };
            subqueries.push((Occur::Must, user_query));
        }

        let final_query = BooleanQuery::new(subqueries);

        // Execute search
        let top_docs = searcher.search(&final_query, &TopDocs::with_limit(limit))?;
        let mut results = vec![];

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            
            let doc_id_val = retrieved_doc
                .get_first(self.doc_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let entity_val = retrieved_doc
                .get_first(self.entity)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title_val = retrieved_doc
                .get_first(self.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Generate highlighted snippet from the body field
            let snippet_generator = SnippetGenerator::create(&searcher, &final_query, self.body)?;
            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
            let snippet_html = snippet.to_html();

            results.push(SearchResult {
                doc_id: doc_id_val,
                entity: entity_val,
                title: title_val,
                snippet: snippet_html,
                score,
            });
        }

        Ok(results)
    }
}
