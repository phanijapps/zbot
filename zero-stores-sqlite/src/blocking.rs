use zero_stores::error::StoreError;

/// Run a synchronous closure on the blocking thread pool, mapping any
/// panic or join error into `StoreError::Backend`.
// Used by Task 4+
#[allow(dead_code)]
pub(crate) async fn block<T, F>(f: F) -> Result<T, StoreError>
where
    F: FnOnce() -> Result<T, StoreError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| StoreError::Backend(format!("blocking task join error: {e}")))?
}

/// Map the existing `knowledge_graph::error::GraphError` into a `StoreError`.
/// Mapping is centralised here so Tasks 4–9 import a single conversion point.
// Used by Task 4+
#[allow(dead_code)]
pub(crate) fn map_graph_err(e: knowledge_graph::error::GraphError) -> StoreError {
    use knowledge_graph::error::GraphError as G;
    match e {
        G::EntityNotFound(_) => StoreError::NotFound,
        G::InvalidEntityType(msg) => StoreError::Invalid(msg),
        G::InvalidRelationshipType(msg) => StoreError::Invalid(msg),
        G::Database(msg) => StoreError::Backend(msg.to_string()),
        G::Serialization(msg) => StoreError::Backend(format!("serialization: {msg}")),
        G::Llm(msg) => StoreError::Backend(format!("llm: {msg}")),
        G::Config(msg) => StoreError::Backend(format!("config: {msg}")),
        G::Other(msg) => StoreError::Backend(msg),
    }
}
