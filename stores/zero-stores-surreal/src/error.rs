//! Error type for the SurrealDB store.
//!
//! Real implementation lands in Task 2.

use thiserror::Error;

/// Errors emitted by the SurrealDB store. Placeholder — populated in Task 2.
#[derive(Debug, Error)]
pub enum SurrealStoreError {
    /// Functionality is not implemented yet.
    #[error("not implemented")]
    NotImplemented,
}
