//! Streaming ingestion pipeline — chunker, queue, extractor, backpressure.
//! Public entry: `IngestionQueue` + HTTP/tool wrappers.

pub mod chunker;
pub mod extractor;
pub mod queue;

pub use extractor::{Extractor, NoopExtractor};
pub use queue::IngestionQueue;
