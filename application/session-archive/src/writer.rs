//! # Archive Writer
//!
//! Writes session messages to Parquet format.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::file::properties::WriterProperties;
use tokio::fs;

use crate::error::{ArchiveError, ArchiveResult};
use crate::schema::{ArchivedMessage, messages_to_record_batch};

/// Parquet writer for archived sessions
pub struct ArchiveWriter {
    /// Output directory for archives
    archive_dir: PathBuf,
    /// Compression level (default: Snappy)
    compression: parquet::basic::Compression,
    /// Row group size (number of rows per row group)
    row_group_size: usize,
}

impl ArchiveWriter {
    /// Create a new archive writer
    pub fn new(archive_dir: PathBuf) -> Self {
        Self {
            archive_dir,
            compression: parquet::basic::Compression::SNAPPY,
            row_group_size: 10_000, // 10K messages per row group
        }
    }

    /// Set compression type
    pub fn with_compression(mut self, compression: parquet::basic::Compression) -> Self {
        self.compression = compression;
        self
    }

    /// Set row group size
    pub fn with_row_group_size(mut self, size: usize) -> Self {
        self.row_group_size = size;
        self
    }

    /// Write messages to a Parquet file
    pub async fn write_messages(
        &self,
        agent_id: &str,
        session_id: &str,
        session_date: &str,
        messages: Vec<ArchivedMessage>,
    ) -> ArchiveResult<PathBuf> {
        if messages.is_empty() {
            return Err(ArchiveError::Config("Cannot write empty message list".to_string()));
        }

        // Create archive directory: {archive_dir}/{agent_id}/{year}/{month}/
        let (year, month) = parse_year_month(session_date)?;
        let agent_dir = self.archive_dir.join(agent_id);
        let date_dir = agent_dir.join(year).join(month);
        fs::create_dir_all(&date_dir).await
            .map_err(|e| ArchiveError::Io(e))?;

        // File name: {session_id}.parquet
        let file_name = format!("{}.parquet", sanitize_filename(session_id));
        let file_path = date_dir.join(&file_name);

        // Use spawn_blocking for the Parquet writing (CPU-intensive)
        let file_path_clone = file_path.clone();
        let compression = self.compression;
        let row_group_size = self.row_group_size;
        tokio::task::spawn_blocking(move || {
            // Create file
            let file = std::fs::File::create(&file_path_clone)
                .map_err(|e| ArchiveError::Io(e))?;

            // Convert messages to RecordBatch
            let record_batch = messages_to_record_batch(messages)?;

            // Create Parquet writer properties
            let mut writer_props = parquet::file::properties::WriterProperties::builder()
                .set_compression(compression)
                .set_max_row_group_size(row_group_size)
                .build();

            // Create Arrow writer
            let mut writer = ArrowWriter::try_new(
                file,
                record_batch.schema(),
                Some(writer_props.into()),
            ).map_err(|e| ArchiveError::Parquet(e.to_string()))?;

            // Write the record batch
            writer.write(&record_batch)
                .map_err(|e| ArchiveError::Parquet(e.to_string()))?;

            // Close the writer
            writer.close()
                .map_err(|e| ArchiveError::Parquet(e.to_string()))?;

            Ok::<_, ArchiveError>(file_path_clone)
        })
        .await
        .map_err(|e| ArchiveError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))??;

        Ok(file_path)
    }
}

/// Builder for ArchiveWriter
pub struct ArchiveWriterBuilder {
    archive_dir: PathBuf,
    compression: parquet::basic::Compression,
    row_group_size: usize,
}

impl ArchiveWriterBuilder {
    pub fn new(archive_dir: PathBuf) -> Self {
        Self {
            archive_dir,
            compression: parquet::basic::Compression::SNAPPY,
            row_group_size: 10_000,
        }
    }

    pub fn with_compression(mut self, compression: parquet::basic::Compression) -> Self {
        self.compression = compression;
        self
    }

    pub fn with_row_group_size(mut self, size: usize) -> Self {
        self.row_group_size = size;
        self
    }

    pub fn build(self) -> ArchiveWriter {
        ArchiveWriter {
            archive_dir: self.archive_dir,
            compression: self.compression,
            row_group_size: self.row_group_size,
        }
    }
}

/// Parse year and month from session date (YYYY-MM-DD)
fn parse_year_month(date: &str) -> ArchiveResult<(String, String)> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() >= 2 {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        Err(ArchiveError::InvalidFormat(format!("Invalid date format: {}", date)))
    }
}

/// Sanitize filename by removing potentially dangerous characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}
