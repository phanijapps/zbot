use async_trait::async_trait;

/// Backend-agnostic persistence for the knowledge graph subsystem.
///
/// Methods are domain-typed and async. Implementations live in sibling
/// `zero-stores-*` crates. Method bodies in this trait are filled in
/// during Task 2.
#[async_trait]
pub trait KnowledgeGraphStore: Send + Sync {
    // Methods added in Task 2.
}
