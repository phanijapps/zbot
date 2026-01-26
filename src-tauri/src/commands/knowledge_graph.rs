// ============================================================================
// KNOWLEDGE GRAPH COMMANDS
// Commands for fetching knowledge graph data for visualization
// ============================================================================

use serde_json::Value;

/// Get all entities for an agent
///
/// Returns entities from the knowledge graph for the specified agent.
#[tauri::command]
pub async fn get_knowledge_graph_entities(
    agent_id: String,
) -> Result<Value, String> {
    use knowledge_graph::GraphStorage;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");
    let storage = GraphStorage::new(db_path)
        .map_err(|e| format!("Failed to open knowledge graph: {}", e))?;

    let entities = storage.get_entities(&agent_id).await
        .map_err(|e| format!("Failed to get entities: {}", e))?;

    serde_json::to_value(entities)
        .map_err(|e| format!("Failed to serialize entities: {}", e))
}

/// Get all relationships for an agent
///
/// Returns relationships from the knowledge graph for the specified agent.
#[tauri::command]
pub async fn get_knowledge_graph_relationships(
    agent_id: String,
) -> Result<Value, String> {
    use knowledge_graph::GraphStorage;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");
    let storage = GraphStorage::new(db_path)
        .map_err(|e| format!("Failed to open knowledge graph: {}", e))?;

    let relationships = storage.get_relationships(&agent_id).await
        .map_err(|e| format!("Failed to get relationships: {}", e))?;

    serde_json::to_value(relationships)
        .map_err(|e| format!("Failed to serialize relationships: {}", e))
}

/// Get complete knowledge graph for an agent
///
/// Returns both entities and relationships for the specified agent.
#[tauri::command]
pub async fn get_knowledge_graph(
    agent_id: String,
) -> Result<Value, String> {
    use knowledge_graph::GraphStorage;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");
    let storage = GraphStorage::new(db_path)
        .map_err(|e| format!("Failed to open knowledge graph: {}", e))?;

    let entities = storage.get_entities(&agent_id).await
        .map_err(|e| format!("Failed to get entities: {}", e))?;

    let relationships = storage.get_relationships(&agent_id).await
        .map_err(|e| format!("Failed to get relationships: {}", e))?;

    serde_json::to_value(serde_json::json!({
        "entities": entities,
        "relationships": relationships
    }))
        .map_err(|e| format!("Failed to serialize graph: {}", e))
}

/// Clear all knowledge graph data
///
/// Deletes the entire knowledge graph database.
#[tauri::command]
pub async fn clear_knowledge_graph() -> Result<(), String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");

    if db_path.exists() {
        std::fs::remove_file(&db_path)
            .map_err(|e| format!("Failed to delete knowledge graph database: {}", e))?;
    }

    Ok(())
}
