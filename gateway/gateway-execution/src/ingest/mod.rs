//! Streaming ingestion pipeline — chunker, queue, extractor, backpressure.
//! Public entry: `IngestionQueue` + HTTP/tool wrappers.

pub mod backpressure;
pub mod chunker;
pub mod extractor;
pub mod json_shape;
pub mod queue;

pub use backpressure::{Backpressure, BackpressureConfig};
pub use extractor::{Extractor, NoopExtractor};
pub use queue::IngestionQueue;
