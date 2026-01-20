//! # Session Archive
//!
//! Parquet-based archival system for session messages.
//!
//! Provides efficient long-term storage with:
//! - Columnar compression (Parquet format)
//! - Fast predicate pushdown queries
//! - Integration with search index

pub mod error;
pub mod schema;
pub mod writer;
pub mod reader;
pub mod manager;

pub use error::{ArchiveError, ArchiveResult};
pub use schema::{ArchivedMessage, ArchiveMetadata};
pub use writer::{ArchiveWriter, ArchiveWriterBuilder};
pub use reader::{ArchiveReader, ArchiveReaderBuilder};
pub use manager::{ArchiveManager, ArchiveManagerBuilder};
