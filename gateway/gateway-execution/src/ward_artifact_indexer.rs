//! Ward Artifact Indexer — scans a ward for structured files (JSON)
//! after a session completes, parses collection-of-objects schemas, and
//! emits entities tagged `epistemic_class = archival` with `source_ref`
//! pointing to the originating file.
//!
//! Zero LLM cost. Domain content that previously lived only in ward files
//! (timeline.json, people.json, etc.) now reaches the knowledge graph.

use gateway_database::{EpisodeSource, KgEpisode, KgEpisodeRepository};
use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, GraphStorage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Entry point: index every structured file in the ward directory.
///
/// Returns the number of entities created. Errors in individual files are
/// logged as warnings — indexing is best-effort, never crashes the pipeline.
pub async fn index_ward(
    ward_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) -> usize {
    let mut created = 0_usize;
    let files = collect_structured_files(ward_path);

    for file_path in files {
        match index_one_file(&file_path, session_id, agent_id, episode_repo, graph).await {
            Ok(n) => created += n,
            Err(e) => tracing::warn!(
                file = ?file_path,
                error = %e,
                "Failed to index ward artifact"
            ),
        }
    }

    tracing::info!(
        ward = ?ward_path,
        entities = created,
        "Ward artifact indexing complete"
    );
    created
}

/// Collect all structured JSON files in the ward recursively,
/// skipping common noise directories.
fn collect_structured_files(ward_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Err(e) = walk(ward_path, &mut files) {
        tracing::warn!(path = ?ward_path, error = %e, "Ward walk failed");
    }
    files
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            walk(&path, out)?;
        } else if is_structured_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("node_modules") | Some(".git") | Some("__pycache__") | Some(".venv") | Some("tmp")
    )
}

fn is_structured_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json") | Some("JSON")
    )
}

/// Index a single file. Returns the number of entities created.
async fn index_one_file(
    file_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) -> Result<usize, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {:?}: {e}", file_path))?;

    let content_hash = compute_hash(&content);

    // Dedup: skip if we've already indexed this exact content
    if episode_repo
        .get_by_content_hash(&content_hash, EpisodeSource::WardFile.as_str())
        .map_err(|e| format!("Dedup check failed: {e}"))?
        .is_some()
    {
        tracing::debug!(file = ?file_path, "Skipping already-indexed ward file");
        return Ok(0);
    }

    // Parse JSON
    let value: Value = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse failed for {:?}: {e}", file_path))?;

    // Create the episode record
    let source_ref = file_path.to_string_lossy().to_string();
    let episode = KgEpisode {
        id: format!("ep-{}", uuid::Uuid::new_v4()),
        source_type: EpisodeSource::WardFile.as_str().to_string(),
        source_ref: source_ref.clone(),
        content_hash,
        session_id: Some(session_id.to_string()),
        agent_id: agent_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    episode_repo
        .upsert_episode(&episode)
        .map_err(|e| format!("Episode insert failed: {e}"))?;

    // Extract entities based on the detected schema
    let schema = detect_collection_schema(&value);
    let entities = extract_entities(&value, schema, agent_id, &episode.id, &source_ref);
    let count = entities.len();

    if count > 0 {
        let knowledge = ExtractedKnowledge {
            entities,
            relationships: vec![],
        };
        graph
            .store_knowledge(agent_id, knowledge)
            .await
            .map_err(|e| format!("Graph store failed: {e}"))?;
    }

    Ok(count)
}

/// Hash file content for dedup.
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// What kind of collection is this JSON?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionSchema {
    /// `[{name: "...", ...}, ...]` — person or organization list
    NamedObjectArray,
    /// `[{date: "...", ...}, ...]` or `[{year: ..., ...}, ...]` — timeline
    DatedObjectArray,
    /// `{key1: {...}, key2: {...}}` — map of named objects
    NamedObjectMap,
    /// Not a known collection schema — skip
    Unknown,
}

/// Heuristically detect the collection schema.
fn detect_collection_schema(value: &Value) -> CollectionSchema {
    match value {
        Value::Array(items) => detect_array_schema(items),
        Value::Object(obj) => {
            if obj.len() >= 2 && obj.values().all(|v| v.is_object()) {
                CollectionSchema::NamedObjectMap
            } else {
                CollectionSchema::Unknown
            }
        }
        _ => CollectionSchema::Unknown,
    }
}

fn detect_array_schema(items: &[Value]) -> CollectionSchema {
    if items.is_empty() {
        return CollectionSchema::Unknown;
    }
    let sample: Vec<&Value> = items.iter().take(5).collect();
    if !sample.iter().all(|v| v.is_object()) {
        return CollectionSchema::Unknown;
    }

    let has_date_field = sample.iter().any(|v| object_has_key(v, is_date_key));
    let has_name_field = sample.iter().any(|v| object_has_key(v, is_name_key));

    if has_date_field {
        CollectionSchema::DatedObjectArray
    } else if has_name_field {
        CollectionSchema::NamedObjectArray
    } else {
        CollectionSchema::Unknown
    }
}

fn object_has_key(v: &Value, pred: fn(&str) -> bool) -> bool {
    v.as_object()
        .map(|o| o.keys().any(|k| pred(k)))
        .unwrap_or(false)
}

fn is_date_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "date" | "year" | "start_date" | "when" | "timestamp"
    )
}

fn is_name_key(key: &str) -> bool {
    matches!(key.to_lowercase().as_str(), "name" | "title" | "label")
}

/// Extract entities from a parsed JSON value given the detected schema.
fn extract_entities(
    value: &Value,
    schema: CollectionSchema,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    match schema {
        CollectionSchema::NamedObjectArray => {
            extract_named_array(value, agent_id, episode_id, source_ref)
        }
        CollectionSchema::DatedObjectArray => {
            extract_dated_array(value, agent_id, episode_id, source_ref)
        }
        CollectionSchema::NamedObjectMap => {
            extract_named_map(value, agent_id, episode_id, source_ref)
        }
        CollectionSchema::Unknown => Vec::new(),
    }
}

fn extract_named_array(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| item.as_object())
        .filter_map(|obj| {
            let name = obj
                .get("name")
                .or_else(|| obj.get("title"))
                .or_else(|| obj.get("label"))
                .and_then(|v| v.as_str())?;
            Some(build_entity(
                name,
                guess_type_from_source_ref(source_ref),
                obj,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

fn extract_dated_array(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| item.as_object())
        .filter_map(|obj| {
            let name = derive_event_name(obj)?;
            // TODO(Phase 6b): upgrade to EntityType::Event
            Some(build_entity(
                &name,
                EntityType::Concept,
                obj,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

fn extract_named_map(
    value: &Value,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Vec<Entity> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .filter_map(|(key, val)| {
            let props = val.as_object()?;
            Some(build_entity(
                key,
                guess_type_from_source_ref(source_ref),
                props,
                agent_id,
                episode_id,
                source_ref,
            ))
        })
        .collect()
}

/// Build a name for a dated entry. Prefer explicit name/title; otherwise
/// synthesize "<year>: <brief description>".
fn derive_event_name(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(name) = obj
        .get("name")
        .or_else(|| obj.get("title"))
        .and_then(|v| v.as_str())
    {
        return Some(name.to_string());
    }
    let year = extract_year_string(obj);
    let brief = extract_brief(obj);
    if brief.is_empty() && year.is_empty() {
        return None;
    }
    Some(format!("{}: {}", year, brief).trim().to_string())
}

fn extract_year_string(obj: &serde_json::Map<String, Value>) -> String {
    let Some(v) = obj.get("year").or_else(|| obj.get("date")) else {
        return String::new();
    };
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(n) = v.as_i64() {
        return n.to_string();
    }
    String::new()
}

fn extract_brief(obj: &serde_json::Map<String, Value>) -> String {
    obj.get("description")
        .or_else(|| obj.get("event"))
        .or_else(|| obj.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(40)
        .collect()
}

fn guess_type_from_source_ref(source_ref: &str) -> EntityType {
    let lower = source_ref.to_lowercase();
    if lower.contains("people") || lower.contains("person") {
        EntityType::Person
    } else if lower.contains("org") || lower.contains("company") {
        EntityType::Organization
    } else if lower.contains("place") || lower.contains("location") || lower.contains("geo") {
        EntityType::Location
    } else {
        // Event / generic — Phase 6b adds a dedicated Event variant.
        // TODO(Phase 6b): map timeline/event files → EntityType::Event.
        EntityType::Concept
    }
}

fn build_entity(
    name: &str,
    entity_type: EntityType,
    properties: &serde_json::Map<String, Value>,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> Entity {
    let mut entity = Entity::new(agent_id.to_string(), entity_type, name.to_string());
    for (k, v) in properties {
        entity.properties.insert(k.clone(), v.clone());
    }
    entity.properties.insert(
        "_source_episode_id".to_string(),
        Value::String(episode_id.to_string()),
    );
    entity.properties.insert(
        "_source_ref".to_string(),
        Value::String(source_ref.to_string()),
    );
    entity.properties.insert(
        "_epistemic_class".to_string(),
        Value::String("archival".to_string()),
    );
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_named_object_array() {
        let v: Value = serde_json::from_str(r#"[{"name": "A"}, {"name": "B"}]"#).unwrap();
        assert_eq!(
            detect_collection_schema(&v),
            CollectionSchema::NamedObjectArray
        );
    }

    #[test]
    fn detect_dated_object_array() {
        let v: Value = serde_json::from_str(r#"[{"date": "1937", "event": "x"}]"#).unwrap();
        assert_eq!(
            detect_collection_schema(&v),
            CollectionSchema::DatedObjectArray
        );
    }

    #[test]
    fn detect_named_object_map() {
        let v: Value = serde_json::from_str(r#"{"A": {"x": 1}, "B": {"y": 2}}"#).unwrap();
        assert_eq!(
            detect_collection_schema(&v),
            CollectionSchema::NamedObjectMap
        );
    }

    #[test]
    fn detect_unknown_primitive() {
        let v: Value = serde_json::from_str(r#""just a string""#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::Unknown);
    }

    #[test]
    fn detect_unknown_array_of_primitives() {
        let v: Value = serde_json::from_str(r#"[1, 2, 3]"#).unwrap();
        assert_eq!(detect_collection_schema(&v), CollectionSchema::Unknown);
    }

    #[test]
    fn extract_named_array_produces_entities() {
        let v: Value = serde_json::from_str(
            r#"[{"name": "Alice", "role": "founder"}, {"name": "Bob", "role": "CEO"}]"#,
        )
        .unwrap();
        let entities = extract_entities(
            &v,
            CollectionSchema::NamedObjectArray,
            "root",
            "ep-1",
            "people.json",
        );
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].name, "Alice");
        assert_eq!(entities[1].name, "Bob");
        assert!(entities[0].properties.contains_key("role"));
        assert!(entities[0].properties.contains_key("_source_ref"));
        assert_eq!(
            entities[0]
                .properties
                .get("_epistemic_class")
                .unwrap()
                .as_str(),
            Some("archival")
        );
    }

    #[test]
    fn extract_dated_array_produces_events() {
        let v: Value =
            serde_json::from_str(r#"[{"date": "1937", "event": "Ahmedabad Session"}]"#).unwrap();
        let entities = extract_entities(
            &v,
            CollectionSchema::DatedObjectArray,
            "root",
            "ep-1",
            "timeline.json",
        );
        assert_eq!(entities.len(), 1);
    }

    #[test]
    fn guess_type_from_people_filename() {
        assert!(matches!(
            guess_type_from_source_ref("/ward/foo/people.json"),
            EntityType::Person
        ));
    }

    #[test]
    fn guess_type_from_places_filename() {
        assert!(matches!(
            guess_type_from_source_ref("/ward/foo/places.json"),
            EntityType::Location
        ));
        assert!(matches!(
            guess_type_from_source_ref("/ward/foo/locations.json"),
            EntityType::Location
        ));
    }

    #[test]
    fn is_date_key_recognizes_common_names() {
        assert!(is_date_key("date"));
        assert!(is_date_key("Year"));
        assert!(is_date_key("start_date"));
        assert!(!is_date_key("description"));
    }

    #[test]
    fn compute_hash_is_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        let h3 = compute_hash("different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
