// ============================================================================
// SEARCH INDEX MANAGER
// Manages Tantivy search index for messages
// ============================================================================

use crate::error::SearchError;
use crate::schema::*;
use crate::types::*;
use crate::Schema;
use chrono::DateTime;
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::{
    collector::TopDocs,
    query::*,
    schema::*,
    Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, Term,
};
use tantivy::schema::TantivyDocument;
use tokio::sync::RwLock;

pub struct SearchIndexManager {
    index: Index,
    writer: Arc<RwLock<Option<IndexWriter>>>,
    reader: Arc<RwLock<Option<Arc<IndexReader>>>>,
    schema: Schema,
}

impl SearchIndexManager {
    /// Create search index at specified directory
    pub fn new(index_dir: PathBuf) -> Result<Self, SearchError> {
        std::fs::create_dir_all(&index_dir)?;

        let schema = create_index_schema();
        let index = if index_dir.exists() && index_dir.read_dir()?.count() > 0 {
            Index::open_in_dir(&index_dir)?
        } else {
            Index::create_in_dir(&index_dir, schema.clone())?
        };

        Ok(Self {
            index,
            writer: Arc::new(RwLock::new(None)),
            reader: Arc::new(RwLock::new(None)),
            schema,
        })
    }

    /// Initialize writer and reader
    pub async fn initialize(&self) -> Result<(), SearchError> {
        let mut writer_lock = self.writer.write().await;
        *writer_lock = Some(self.index.writer(50_000_000)?); // 50MB buffer

        let mut reader_lock = self.reader.write().await;
        *reader_lock = Some(Arc::new(
            self.index
                .reader_builder()
                .reload_policy(ReloadPolicy::OnCommitWithDelay)
                .try_into()?,
        ));

        Ok(())
    }

    /// Add or update a message in the index
    pub async fn index_message(&self, doc: &IndexedDocument) -> Result<(), SearchError> {
        let mut writer = self.writer.write().await;
        let writer = writer
            .as_mut()
            .ok_or(SearchError::WriterNotInitialized)?;

        let mut doc_builder = TantivyDocument::default();

        doc_builder.add_text(self.schema.get_field("message_id")?, &doc.message_id);
        doc_builder.add_text(self.schema.get_field("session_id")?, &doc.session_id);
        doc_builder.add_text(self.schema.get_field("agent_id")?, &doc.agent_id);
        doc_builder.add_text(self.schema.get_field("agent_name")?, &doc.agent_name);
        doc_builder.add_text(self.schema.get_field("role")?, &doc.role);
        doc_builder.add_text(self.schema.get_field("content")?, &doc.content);
        doc_builder.add_i64(self.schema.get_field("timestamp")?, doc.timestamp);
        doc_builder.add_text(self.schema.get_field("source_type")?, &doc.source_type);

        if let Some(path) = &doc.source_path {
            doc_builder.add_text(self.schema.get_field("source_path")?, path);
        }

        writer.add_document(doc_builder)?;
        writer.commit()?;

        Ok(())
    }

    /// Batch index multiple messages
    pub async fn index_messages(&self, docs: &[IndexedDocument]) -> Result<(), SearchError> {
        let mut writer = self.writer.write().await;
        let writer = writer
            .as_mut()
            .ok_or(SearchError::WriterNotInitialized)?;

        for doc in docs {
            let mut doc_builder = TantivyDocument::default();
            doc_builder.add_text(self.schema.get_field("message_id")?, &doc.message_id);
            doc_builder.add_text(self.schema.get_field("session_id")?, &doc.session_id);
            doc_builder.add_text(self.schema.get_field("agent_id")?, &doc.agent_id);
            doc_builder.add_text(self.schema.get_field("agent_name")?, &doc.agent_name);
            doc_builder.add_text(self.schema.get_field("role")?, &doc.role);
            doc_builder.add_text(self.schema.get_field("content")?, &doc.content);
            doc_builder.add_i64(self.schema.get_field("timestamp")?, doc.timestamp);
            doc_builder.add_text(self.schema.get_field("source_type")?, &doc.source_type);

            if let Some(path) = &doc.source_path {
                doc_builder.add_text(self.schema.get_field("source_path")?, path);
            }

            writer.add_document(doc_builder)?;
        }

        writer.commit()?;
        Ok(())
    }

    /// Full-text search with optional filters
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError> {
        let reader = self.reader.read().await;
        let reader_ref = reader.as_ref().ok_or(SearchError::ReaderNotInitialized)?;
        let searcher = reader_ref.searcher();

        let content_field = self.schema.get_field("content")?;
        let agent_id_field = self.schema.get_field("agent_id")?;

        // Build query
        let query_parser = QueryParser::for_index(&self.index, vec![content_field]);
        let text_query = query_parser.parse_query(&query.query)?;

        let final_query: Box<dyn Query> = if let Some(agent) = &query.agent_id {
            let term = Term::from_field_text(agent_id_field, agent);
            let agent_query = TermQuery::new(term, IndexRecordOption::Basic);
            Box::new(BooleanQuery::intersection(vec![
                Box::new(text_query),
                Box::new(agent_query),
            ]))
        } else {
            text_query
        };

        let top_docs = searcher.search(&final_query, &TopDocs::with_limit(query.limit))?;

        let results = top_docs
            .into_iter()
            .filter_map(|(score, doc_address)| {
                self.convert_to_search_result(&searcher, doc_address, score)
                    .ok()
            })
            .collect();

        Ok(results)
    }

    /// Delete a single session from index
    pub async fn delete_session(&self, session_id: &str) -> Result<usize, SearchError> {
        let mut writer = self.writer.write().await;
        let writer = writer
            .as_mut()
            .ok_or(SearchError::WriterNotInitialized)?;

        let session_id_field = self.schema.get_field("session_id")?;
        let term = Term::from_field_text(session_id_field, session_id);

        let reader = self.reader.read().await;
        let reader_ref = reader.as_ref().ok_or(SearchError::ReaderNotInitialized)?;
        let searcher = reader_ref.searcher();

        let query = TermQuery::new(term, IndexRecordOption::Basic);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(10000))?;

        let mut count = 0;
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                if let Some(message_id_value) = doc.get_first(self.schema.get_field("message_id")?) {
                    if let Some(message_id) = message_id_value.as_str() {
                        let message_id_field = self.schema.get_field("message_id")?;
                        let message_id_term = Term::from_field_text(message_id_field, message_id);
                        writer.delete_term(message_id_term);
                        count += 1;
                    }
                }
            }
        }

        writer.commit()?;
        Ok(count)
    }

    /// Delete all messages for an agent
    pub async fn delete_agent(&self, agent_id: &str) -> Result<usize, SearchError> {
        let mut writer = self.writer.write().await;
        let writer = writer
            .as_mut()
            .ok_or(SearchError::WriterNotInitialized)?;

        let agent_id_field = self.schema.get_field("agent_id")?;
        let term = Term::from_field_text(agent_id_field, agent_id);

        let reader = self.reader.read().await;
        let reader_ref = reader.as_ref().ok_or(SearchError::ReaderNotInitialized)?;
        let searcher = reader_ref.searcher();

        let query = TermQuery::new(term, IndexRecordOption::Basic);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(100000))?;

        let mut count = 0;
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                if let Some(message_id_value) = doc.get_first(self.schema.get_field("message_id")?) {
                    if let Some(message_id) = message_id_value.as_str() {
                        let message_id_field = self.schema.get_field("message_id")?;
                        let message_id_term = Term::from_field_text(message_id_field, message_id);
                        writer.delete_term(message_id_term);
                        count += 1;
                    }
                }
            }
        }

        writer.commit()?;
        Ok(count)
    }

    /// Clear entire index
    pub async fn clear(&self) -> Result<(), SearchError> {
        let mut writer = self.writer.write().await;
        let writer = writer
            .as_mut()
            .ok_or(SearchError::WriterNotInitialized)?;

        writer.delete_all_documents()?;
        writer.commit()?;

        Ok(())
    }

    fn convert_to_search_result(
        &self,
        searcher: &Searcher,
        doc_address: tantivy::DocAddress,
        score: f32,
    ) -> Result<SearchResult, SearchError> {
        let doc = searcher.doc::<TantivyDocument>(doc_address)?;

        let get_field_str = |field_name: &str| -> Result<String, SearchError> {
            let field = self.schema.get_field(field_name)?;
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| SearchError::NotFound(field_name.to_string()))
        };

        let get_field_i64 = |field_name: &str| -> Result<i64, SearchError> {
            let field = self.schema.get_field(field_name)?;
            doc.get_first(field)
                .and_then(|v| v.as_i64())
                .ok_or_else(|| SearchError::NotFound(field_name.to_string()))
        };

        let message_id = get_field_str("message_id")?;
        let session_id = get_field_str("session_id")?;
        let agent_id = get_field_str("agent_id")?;
        let agent_name = get_field_str("agent_name")?;
        let role = get_field_str("role")?;
        let content = get_field_str("content")?;
        let timestamp = get_field_i64("timestamp")?;
        let source_type = get_field_str("source_type")?;

        let source_path = doc
            .get_first(self.schema.get_field("source_path")?)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let source = match source_type.as_str() {
            "sqlite" => MessageSource::Sqlite { session_id: session_id.clone() },
            "parquet" => MessageSource::Parquet {
                session_id: session_id.clone(),
                file_path: source_path.unwrap_or_default(),
            },
            _ => MessageSource::Sqlite { session_id: session_id.clone() },
        };

        let created_at = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or(DateTime::UNIX_EPOCH);

        Ok(SearchResult {
            message_id,
            session_id,
            agent_id,
            agent_name,
            role,
            content,
            created_at,
            score,
            source,
        })
    }
}
