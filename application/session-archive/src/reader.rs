//! # Archive Reader
//!
//! Reads session messages from Parquet files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::*;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tokio::fs;

use crate::error::{ArchiveError, ArchiveResult};
use crate::schema::ArchivedMessage;

/// Parquet reader for archived sessions
pub struct ArchiveReader {
    /// Archive directory
    archive_dir: PathBuf,
}

impl ArchiveReader {
    /// Create a new archive reader
    pub fn new(archive_dir: PathBuf) -> Self {
        Self { archive_dir }
    }

    /// List all archived sessions for an agent
    pub async fn list_archives(&self, agent_id: &str) -> ArchiveResult<Vec<PathBuf>> {
        let agent_dir = self.archive_dir.join(agent_id);

        if !agent_dir.exists() {
            return Ok(vec![]);
        }

        // Use spawn_blocking for directory traversal (blocking I/O)
        let agent_dir_clone = agent_dir.clone();
        let archives = tokio::task::spawn_blocking(move || {
            let mut archives = Vec::new();
            walk_parquet_files(&agent_dir_clone, &mut archives)?;
            Ok::<_, ArchiveError>(archives)
        })
        .await
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))??;

        Ok(archives)
    }

    /// Read messages from a specific archive file
    pub async fn read_archive(&self, file_path: &Path) -> ArchiveResult<Vec<ArchivedMessage>> {
        let file_path = file_path.to_path_buf();

        // Use spawn_blocking for Parquet reading (CPU-intensive)
        let messages = tokio::task::spawn_blocking(move || {
            Self::read_archive_sync(&file_path)
        })
        .await
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))??;

        Ok(messages)
    }

    /// Read messages for a specific session
    pub async fn read_session(&self, agent_id: &str, session_id: &str) -> ArchiveResult<Vec<ArchivedMessage>> {
        let agent_dir = self.archive_dir.join(agent_id);
        let session_id = session_id.to_string();

        // Keep copies for error reporting
        let agent_dir_clone = agent_dir.clone();
        let session_id_clone = session_id.clone();

        // Find the archive file for this session
        let file_path = tokio::task::spawn_blocking(move || {
            find_session_archive(&agent_dir, &session_id)
        })
        .await
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))??;

        match file_path {
            Some(path) => self.read_archive(&path).await,
            None => Err(ArchiveError::NotFound(agent_dir_clone.join(session_id_clone))),
        }
    }

    /// Synchronously read messages from a Parquet file
    fn read_archive_sync(file_path: &Path) -> ArchiveResult<Vec<ArchivedMessage>> {
        if !file_path.exists() {
            return Err(ArchiveError::NotFound(file_path.to_path_buf()));
        }

        let file = std::fs::File::open(file_path)
            .map_err(|e| ArchiveError::Io(e))?;

        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| ArchiveError::Parquet(e.to_string()))?;

        let mut reader = builder.build()
            .map_err(|e| ArchiveError::Parquet(e.to_string()))?;

        let mut messages = Vec::new();

        while let Some(batch_result) = reader.next() {
            let batch = batch_result
                .map_err(|e| ArchiveError::Parquet(e.to_string()))?;

            messages.extend(record_batch_to_messages(&batch)?);
        }

        Ok(messages)
    }
}

/// Builder for ArchiveReader
pub struct ArchiveReaderBuilder {
    archive_dir: PathBuf,
}

impl ArchiveReaderBuilder {
    pub fn new(archive_dir: PathBuf) -> Self {
        Self { archive_dir }
    }

    pub fn build(self) -> ArchiveReader {
        ArchiveReader { archive_dir: self.archive_dir }
    }
}

/// Convert a RecordBatch to ArchivedMessages
fn record_batch_to_messages(batch: &arrow::record_batch::RecordBatch) -> ArchiveResult<Vec<ArchivedMessage>> {
    let num_rows = batch.num_rows();

    let ids = batch.column(0).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'id' column".to_string())
    )?;
    let session_ids = batch.column(1).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'session_id' column".to_string())
    )?;
    let agent_ids = batch.column(2).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'agent_id' column".to_string())
    )?;
    let agent_names = batch.column(3).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'agent_name' column".to_string())
    )?;
    let roles = batch.column(4).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'role' column".to_string())
    )?;
    let contents = batch.column(5).as_any().downcast_ref::<LargeStringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'content' column".to_string())
    )?;
    let created_at = batch.column(6).as_any().downcast_ref::<TimestampMillisecondArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'created_at' column".to_string())
    )?;
    let token_counts = batch.column(7).as_any().downcast_ref::<Int64Array>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'token_count' column".to_string())
    )?;
    let tool_calls = batch.column(8).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'tool_calls' column".to_string())
    )?;
    let tool_results = batch.column(9).as_any().downcast_ref::<StringArray>().ok_or_else(||
        ArchiveError::InvalidFormat("Invalid 'tool_results' column".to_string())
    )?;

    let mut messages = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let created_at_ms = created_at.value(i);
        messages.push(ArchivedMessage {
            id: ids.value(i).to_string(),
            session_id: session_ids.value(i).to_string(),
            agent_id: agent_ids.value(i).to_string(),
            agent_name: agent_names.value(i).to_string(),
            role: roles.value(i).to_string(),
            content: contents.value(i).to_string(),
            created_at: chrono::DateTime::from_timestamp_millis(created_at_ms)
                .unwrap_or_else(|| chrono::Utc::now()),
            token_count: token_counts.is_null(i).then(|| token_counts.value(i)),
            tool_calls: (!tool_calls.is_null(i)).then(|| tool_calls.value(i).to_string()),
            tool_results: (!tool_results.is_null(i)).then(|| tool_results.value(i).to_string()),
        });
    }

    Ok(messages)
}

/// Recursively walk directory to find all Parquet files
fn walk_parquet_files(dir: &Path, archives: &mut Vec<PathBuf>) -> ArchiveResult<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| ArchiveError::Io(e))?;

    for entry in entries {
        let entry = entry.map_err(|e| ArchiveError::Io(e))?;
        let path = entry.path();

        if path.is_dir() {
            walk_parquet_files(&path, archives)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("parquet") {
            archives.push(path);
        }
    }

    Ok(())
}

/// Find the archive file for a specific session
fn find_session_archive(agent_dir: &Path, session_id: &str) -> ArchiveResult<Option<PathBuf>> {
    if !agent_dir.exists() {
        return Ok(None);
    }

    let mut archives = Vec::new();
    walk_parquet_files(agent_dir, &mut archives)?;

    // Find the archive that matches the session_id
    for archive in archives {
        let file_stem = archive.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if file_stem == session_id {
            return Ok(Some(archive));
        }
    }

    Ok(None)
}
