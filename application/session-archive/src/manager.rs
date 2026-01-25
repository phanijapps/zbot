//! # Archive Manager
//!
//! High-level API for managing session archives.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs;

use crate::error::{ArchiveError, ArchiveResult};
use crate::schema::{ArchivedMessage, ArchiveMetadata};
use crate::writer::{ArchiveWriter, ArchiveWriterBuilder};
use crate::reader::{ArchiveReader, ArchiveReaderBuilder};

/// High-level manager for session archives
pub struct ArchiveManager {
    /// Archive writer
    writer: Arc<ArchiveWriter>,
    /// Archive reader
    reader: Arc<ArchiveReader>,
    /// Archive directory
    archive_dir: PathBuf,
}

impl ArchiveManager {
    /// Create a new archive manager
    pub fn new(archive_dir: PathBuf) -> ArchiveResult<Self> {
        // Ensure archive directory exists
        std::fs::create_dir_all(&archive_dir)
            .map_err(|e| ArchiveError::Io(e))?;

        let writer = Arc::new(ArchiveWriter::new(archive_dir.clone()));
        let reader = Arc::new(ArchiveReader::new(archive_dir.clone()));

        Ok(Self {
            writer,
            reader,
            archive_dir,
        })
    }

    /// Archive messages from a session
    pub async fn archive_session(
        &self,
        agent_id: &str,
        session_id: &str,
        session_date: &str,
        messages: Vec<ArchivedMessage>,
    ) -> ArchiveResult<ArchiveMetadata> {
        let message_count = messages.len();
        let total_tokens: i64 = messages.iter().map(|m| m.token_count.unwrap_or(0)).sum();

        let earliest_message = messages.iter()
            .map(|m| m.created_at)
            .min()
            .ok_or_else(|| ArchiveError::Config("No messages to archive".to_string()))?;

        let latest_message = messages.iter()
            .map(|m| m.created_at)
            .max()
            .ok_or_else(|| ArchiveError::Config("No messages to archive".to_string()))?;

        let file_path = self.writer.write_messages(
            agent_id,
            session_id,
            session_date,
            messages,
        ).await?;

        let file_size = fs::metadata(&file_path).await
            .map_err(|e| ArchiveError::Io(e))?
            .len();

        Ok(ArchiveMetadata {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            session_date: session_date.to_string(),
            message_count,
            earliest_message,
            latest_message,
            total_tokens,
            file_size,
            file_path: file_path.to_string_lossy().to_string(),
        })
    }

    /// Read messages from an archived session
    pub async fn read_session(&self, agent_id: &str, session_id: &str) -> ArchiveResult<Vec<ArchivedMessage>> {
        self.reader.read_session(agent_id, session_id).await
    }

    /// Read messages from a specific archive file
    pub async fn read_archive(&self, file_path: &Path) -> ArchiveResult<Vec<ArchivedMessage>> {
        self.reader.read_archive(file_path).await
    }

    /// List all archives for an agent
    pub async fn list_archives(&self, agent_id: &str) -> ArchiveResult<Vec<PathBuf>> {
        self.reader.list_archives(agent_id).await
    }

    /// Delete an archive file
    pub async fn delete_archive(&self, file_path: &Path) -> ArchiveResult<()> {
        fs::remove_file(file_path).await
            .map_err(|e| ArchiveError::Io(e))
    }

    /// Delete all archives for an agent
    pub async fn delete_agent_archives(&self, agent_id: &str) -> ArchiveResult<()> {
        let agent_dir = self.archive_dir.join(agent_id);

        if !agent_dir.exists() {
            return Ok(());
        }

        fs::remove_dir_all(&agent_dir).await
            .map_err(|e| ArchiveError::Io(e))
    }

    /// Get the archive directory for an agent
    pub fn agent_archive_dir(&self, agent_id: &str) -> PathBuf {
        self.archive_dir.join(agent_id)
    }
}

/// Builder for ArchiveManager
pub struct ArchiveManagerBuilder {
    archive_dir: Option<PathBuf>,
    compression: Option<parquet::basic::Compression>,
    row_group_size: Option<usize>,
}

impl ArchiveManagerBuilder {
    pub fn new() -> Self {
        Self {
            archive_dir: None,
            compression: None,
            row_group_size: None,
        }
    }

    pub fn with_archive_dir(mut self, dir: PathBuf) -> Self {
        self.archive_dir = Some(dir);
        self
    }

    pub fn with_compression(mut self, compression: parquet::basic::Compression) -> Self {
        self.compression = Some(compression);
        self
    }

    pub fn with_row_group_size(mut self, size: usize) -> Self {
        self.row_group_size = Some(size);
        self
    }

    pub fn build(self) -> ArchiveResult<ArchiveManager> {
        let archive_dir = self.archive_dir
            .ok_or_else(|| ArchiveError::Config("Archive directory not set".to_string()))?;

        // Ensure archive directory exists
        std::fs::create_dir_all(&archive_dir)
            .map_err(|e| ArchiveError::Io(e))?;

        let mut writer_builder = ArchiveWriterBuilder::new(archive_dir.clone());
        if let Some(compression) = self.compression {
            writer_builder = writer_builder.with_compression(compression);
        }
        if let Some(size) = self.row_group_size {
            writer_builder = writer_builder.with_row_group_size(size);
        }

        let writer = Arc::new(writer_builder.build());
        let reader = Arc::new(ArchiveReader::new(archive_dir.clone()));

        Ok(ArchiveManager {
            writer,
            reader,
            archive_dir,
        })
    }
}

impl Default for ArchiveManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_archive_and_read() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = ArchiveManager::new(temp_dir.path().to_path_buf()).unwrap();

        let messages = vec![
            ArchivedMessage {
                id: "msg1".to_string(),
                session_id: "session_test".to_string(),
                agent_id: "agent_test".to_string(),
                agent_name: "Test Agent".to_string(),
                role: "user".to_string(),
                content: "Hello".to_string(),
                created_at: Utc::now(),
                token_count: Some(10),
                tool_calls: None,
                tool_results: None,
            },
        ];

        let metadata = manager.archive_session(
            "agent_test",
            "session_test",
            "2025-01-19",
            messages.clone(),
        ).await.unwrap();

        assert_eq!(metadata.message_count, 1);
        assert_eq!(metadata.agent_id, "agent_test");

        let read_messages = manager.read_session("agent_test", "session_test").await.unwrap();
        assert_eq!(read_messages.len(), 1);
        assert_eq!(read_messages[0].id, "msg1");
    }
}
