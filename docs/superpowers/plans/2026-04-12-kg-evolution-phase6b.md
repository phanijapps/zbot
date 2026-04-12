# Knowledge Graph Evolution — Phase 6b Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Expand the knowledge graph ontology with richer entity and relationship types, prescribed properties per type, and an entity resolver that merges name variants (e.g., `Savarkar` ↔ `V.D. Savarkar`) on write.

**Architecture:** Four components:
1. Expanded `EntityType` enum (add `Event`, `TimePeriod`, `Document`, `Role`, `Artifact`, `Ward`)
2. Expanded `RelationshipType` vocabulary (temporal, spatial, causal, role-based, hierarchical)
3. `EntityResolver` — on write, normalize name, fuzzy-match, embedding-match, merge or create
4. Updated distillation prompt with type guidance, property schemas, and few-shot examples

**Tech Stack:** Rust (knowledge-graph, gateway-execution), SQLite, regex for fuzzy matching, EmbeddingClient trait for semantic similarity.

**Spec:** `docs/superpowers/specs/2026-04-12-knowledge-graph-evolution-design.md` — Phase 6b

**Branch:** `feature/sentient` (continues from Phase 6a)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| MODIFY | `services/knowledge-graph/src/types.rs` | Expand EntityType + RelationshipType enums |
| CREATE | `services/knowledge-graph/src/resolver.rs` | EntityResolver: normalize + match + merge |
| MODIFY | `services/knowledge-graph/src/lib.rs` | Export EntityResolver |
| MODIFY | `services/knowledge-graph/src/storage.rs` | Use resolver on entity writes; add aliases support |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` | Expanded prompt with few-shot + property schemas |

---

### Task 1: Expanded EntityType and RelationshipType Enums

**Files:**
- Modify: `services/knowledge-graph/src/types.rs`

- [ ] **Step 1: Read current enum definitions**

Open `services/knowledge-graph/src/types.rs`. Note the existing variants:
- EntityType: Person, Organization, Location, Concept, Tool, Project, File, Custom(String)
- RelationshipType: WorksFor, LocatedIn, RelatedTo, Created, Uses, PartOf, Mentions, Custom(String)

- [ ] **Step 2: Add new EntityType variants**

Add these variants to `EntityType` (keep Custom last):

```rust
pub enum EntityType {
    // Existing
    Person,
    Organization,
    Location,
    Concept,
    Tool,
    Project,
    File,
    // New in Phase 6b
    Event,      // historical events, meetings, sessions
    TimePeriod, // years, eras, date ranges
    Document,   // books, articles, PDFs, URLs
    Role,       // "president", "CEO", role held by a person at a time
    Artifact,   // generated files, reports, data outputs
    Ward,       // workspace entity (made explicit)
    Custom(String),
}
```

Update the `from_str` impl:

```rust
pub fn from_str(s: &str) -> Self {
    match s.to_lowercase().as_str() {
        "person" => EntityType::Person,
        "organization" | "org" | "company" => EntityType::Organization,
        "location" | "place" | "geography" => EntityType::Location,
        "concept" | "topic" | "idea" => EntityType::Concept,
        "tool" | "technology" | "library" => EntityType::Tool,
        "project" => EntityType::Project,
        "file" => EntityType::File,
        "event" | "meeting" | "session" => EntityType::Event,
        "time_period" | "timeperiod" | "era" | "year" => EntityType::TimePeriod,
        "document" | "book" | "article" | "pdf" => EntityType::Document,
        "role" | "position" | "title" => EntityType::Role,
        "artifact" | "report" | "output" => EntityType::Artifact,
        "ward" | "workspace" => EntityType::Ward,
        other => EntityType::Custom(other.to_string()),
    }
}
```

Update `as_str` to include the new variants.

- [ ] **Step 3: Add new RelationshipType variants**

Add grouped variants:

```rust
pub enum RelationshipType {
    // Existing
    WorksFor,
    LocatedIn,
    RelatedTo,
    Created,
    Uses,
    PartOf,
    Mentions,
    // Phase 6b: Temporal
    Before,
    After,
    During,
    ConcurrentWith,
    SucceededBy,
    PrecededBy,
    // Phase 6b: Role
    PresidentOf,
    FounderOf,
    MemberOf,
    AuthorOf,
    HeldRole,
    EmployedBy,
    // Phase 6b: Spatial
    HeldAt,
    BornIn,
    DiedIn,
    // Phase 6b: Causal
    Caused,
    Enabled,
    Prevented,
    TriggeredBy,
    // Phase 6b: Hierarchical
    Contains,
    InstanceOf,
    SubtypeOf,
    // Fallback
    Custom(String),
}
```

Update `from_str` with case-insensitive + underscore-stripped matching for each new variant:

```rust
pub fn from_str(s: &str) -> Self {
    match s.to_lowercase().replace('_', "").as_str() {
        // Existing
        "worksfor" => RelationshipType::WorksFor,
        "locatedin" | "isin" => RelationshipType::LocatedIn,
        "relatedto" | "related" => RelationshipType::RelatedTo,
        "created" => RelationshipType::Created,
        "uses" => RelationshipType::Uses,
        "partof" => RelationshipType::PartOf,
        "mentions" => RelationshipType::Mentions,
        // Temporal
        "before" => RelationshipType::Before,
        "after" => RelationshipType::After,
        "during" => RelationshipType::During,
        "concurrentwith" | "concurrent" => RelationshipType::ConcurrentWith,
        "succeededby" | "succeeded" => RelationshipType::SucceededBy,
        "precededby" | "preceded" => RelationshipType::PrecededBy,
        // Role
        "presidentof" | "president" => RelationshipType::PresidentOf,
        "founderof" | "founded" => RelationshipType::FounderOf,
        "memberof" => RelationshipType::MemberOf,
        "authorof" | "authored" | "wrote" => RelationshipType::AuthorOf,
        "heldrole" => RelationshipType::HeldRole,
        "employedby" => RelationshipType::EmployedBy,
        // Spatial
        "heldat" => RelationshipType::HeldAt,
        "bornin" => RelationshipType::BornIn,
        "diedin" => RelationshipType::DiedIn,
        // Causal
        "caused" | "causes" => RelationshipType::Caused,
        "enabled" | "enables" => RelationshipType::Enabled,
        "prevented" | "prevents" => RelationshipType::Prevented,
        "triggeredby" | "triggered" => RelationshipType::TriggeredBy,
        // Hierarchical
        "contains" => RelationshipType::Contains,
        "instanceof" => RelationshipType::InstanceOf,
        "subtypeof" => RelationshipType::SubtypeOf,
        other => RelationshipType::Custom(other.to_string()),
    }
}
```

Update `as_str` for all new variants (snake_case output).

- [ ] **Step 4: Add unit tests for the new variants**

In the existing test module (or create one):

```rust
#[test]
fn entity_type_roundtrip_new_variants() {
    for name in ["event", "time_period", "document", "role", "artifact", "ward"] {
        let et = EntityType::from_str(name);
        assert!(!matches!(et, EntityType::Custom(_)), "Expected variant for {name}");
    }
}

#[test]
fn relationship_type_roundtrip_temporal() {
    for name in ["before", "after", "during", "concurrent_with", "succeeded_by"] {
        let rt = RelationshipType::from_str(name);
        assert!(!matches!(rt, RelationshipType::Custom(_)), "Expected variant for {name}");
    }
}

#[test]
fn relationship_type_roundtrip_role_based() {
    for name in ["president_of", "founder_of", "member_of", "author_of"] {
        let rt = RelationshipType::from_str(name);
        assert!(!matches!(rt, RelationshipType::Custom(_)), "Expected variant for {name}");
    }
}

#[test]
fn relationship_type_case_insensitive() {
    assert!(!matches!(
        RelationshipType::from_str("PresidentOf"),
        RelationshipType::Custom(_)
    ));
    assert!(!matches!(
        RelationshipType::from_str("president_of"),
        RelationshipType::Custom(_)
    ));
}
```

- [ ] **Step 5: Verify + commit**

Run: `cargo test --package knowledge-graph -- types`
Expected: 4+ new tests pass plus existing tests continue passing.

Run: `cargo fmt --all && cargo clippy --package knowledge-graph -- -D warnings`
Expected: Clean.

```bash
git add services/knowledge-graph/src/types.rs
git commit -m "feat(kg): expand EntityType + RelationshipType with 6b ontology vocabulary"
```

---

### Task 2: EntityResolver — Normalize, Fuzzy Match, Embedding Match, Merge

**Files:**
- Create: `services/knowledge-graph/src/resolver.rs`
- Modify: `services/knowledge-graph/src/lib.rs`

- [ ] **Step 1: Create resolver.rs**

This module handles entity deduplication on write. Every new entity goes through it; the resolver either returns an existing entity ID (merge) or signals "create new".

Design guardrails (cognitive complexity ≤ 15):
- `resolve()` — orchestrator, delegates to helpers (target ≤ 10)
- `normalize_name()` — string transformation (target ≤ 5)
- `exact_match()` — SQL lookup (target ≤ 5)
- `fuzzy_match()` — Levenshtein against recent entities (target ≤ 10)
- `embedding_match()` — cosine similarity against embedded names (target ≤ 12)

```rust
//! EntityResolver — merges entity variants (e.g., "Savarkar" ↔ "V.D. Savarkar")
//! on write. Runs a cascade of matchers; the first match wins.
//!
//! Cascade order (cheapest first):
//!   1. Exact normalized match (lowercase + strip honorifics)
//!   2. Fuzzy name match (Levenshtein distance ≤ 3) within same type
//!   3. Embedding similarity (cosine ≥ 0.87) on name+description within same type

use crate::{Entity, EntityType};
use rusqlite::{params, Connection};

/// Outcome of resolving a candidate entity against existing ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveOutcome {
    /// Existing entity found; candidate should be merged into it.
    /// Caller should add candidate.name to the existing entity's aliases.
    Merge { existing_id: String, reason: MatchReason },
    /// No match; candidate should be created as a new entity.
    Create,
}

/// Why a match was selected — useful for observability and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchReason {
    ExactNormalized,
    FuzzyName,
    EmbeddingSimilarity,
}

/// Resolve a candidate entity against existing entities in the same agent+type scope.
///
/// This is a blocking function that operates on a SQLite Connection — the
/// caller is responsible for holding the connection lock.
pub fn resolve(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
    candidate_embedding: Option<&[f32]>,
) -> Result<ResolveOutcome, String> {
    // 1. Exact normalized match
    if let Some(existing_id) = exact_match(conn, agent_id, candidate)? {
        return Ok(ResolveOutcome::Merge {
            existing_id,
            reason: MatchReason::ExactNormalized,
        });
    }

    // 2. Fuzzy name match
    if let Some(existing_id) = fuzzy_match(conn, agent_id, candidate)? {
        return Ok(ResolveOutcome::Merge {
            existing_id,
            reason: MatchReason::FuzzyName,
        });
    }

    // 3. Embedding similarity (only if embedding provided)
    if let Some(emb) = candidate_embedding {
        if let Some(existing_id) = embedding_match(conn, agent_id, candidate, emb)? {
            return Ok(ResolveOutcome::Merge {
                existing_id,
                reason: MatchReason::EmbeddingSimilarity,
            });
        }
    }

    Ok(ResolveOutcome::Create)
}

/// Normalize a name for exact-match comparison: lowercase, trim, strip
/// common honorifics and punctuation.
pub fn normalize_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let stripped = strip_honorifics(&lower);
    stripped.replace(['.', ','], "")
}

fn strip_honorifics(name: &str) -> String {
    const HONORIFICS: &[&str] = &[
        "dr ", "dr. ", "mr ", "mr. ", "mrs ", "mrs. ", "ms ", "ms. ",
        "prof ", "prof. ", "sir ", "shri ", "smt ",
    ];
    let mut out = name.to_string();
    for h in HONORIFICS {
        if out.starts_with(h) {
            out = out[h.len()..].to_string();
            break;
        }
    }
    out
}

fn exact_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
) -> Result<Option<String>, String> {
    let normalized = normalize_name(&candidate.name);
    let type_str = candidate.entity_type.as_str();

    let mut stmt = conn
        .prepare(
            "SELECT id, name, aliases FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let aliases: Option<String> = row.get(2)?;
            Ok((id, name, aliases))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    for row in rows {
        let (id, name, aliases) = row.map_err(|e| format!("row read failed: {e}"))?;
        if normalize_name(&name) == normalized {
            return Ok(Some(id));
        }
        if let Some(aliases_json) = aliases {
            if alias_list_contains(&aliases_json, &normalized) {
                return Ok(Some(id));
            }
        }
    }
    Ok(None)
}

fn alias_list_contains(aliases_json: &str, normalized_target: &str) -> bool {
    serde_json::from_str::<Vec<String>>(aliases_json)
        .map(|list| list.iter().any(|a| normalize_name(a) == normalized_target))
        .unwrap_or(false)
}

/// Fuzzy name match: Levenshtein distance ≤ 3 within the same type.
/// Only applied when candidate name is long enough to avoid false matches
/// on short strings (e.g., "A" and "B" have distance 1).
fn fuzzy_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
) -> Result<Option<String>, String> {
    let candidate_norm = normalize_name(&candidate.name);
    if candidate_norm.len() < 6 {
        return Ok(None); // too short for reliable fuzzy matching
    }

    let type_str = candidate.entity_type.as_str();
    let mut stmt = conn
        .prepare(
            "SELECT id, name FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2 \
             ORDER BY mention_count DESC LIMIT 100",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    for row in rows {
        let (id, name) = row.map_err(|e| format!("row read failed: {e}"))?;
        let name_norm = normalize_name(&name);
        if name_norm.len() < 6 {
            continue;
        }
        if levenshtein(&candidate_norm, &name_norm) <= 3 {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Compute Levenshtein distance between two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let (m, n) = (a_chars.len(), b_chars.len());

    if m == 0 { return n; }
    if n == 0 { return m; }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0_usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = *[
                curr[j - 1] + 1,       // insert
                prev[j] + 1,           // delete
                prev[j - 1] + cost,    // substitute
            ]
            .iter()
            .min()
            .unwrap();
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Embedding similarity match — requires candidate_embedding to be provided.
/// Uses cosine similarity ≥ 0.87 within the same type.
fn embedding_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
    candidate_emb: &[f32],
) -> Result<Option<String>, String> {
    let type_str = candidate.entity_type.as_str();
    let mut stmt = conn
        .prepare(
            "SELECT id, properties FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2 \
             AND properties LIKE '%_name_embedding%' \
             ORDER BY mention_count DESC LIMIT 50",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    let mut best: Option<(String, f64)> = None;
    for row in rows {
        let (id, props_json) = row.map_err(|e| format!("row read failed: {e}"))?;
        let Some(props_str) = props_json else { continue };
        let Ok(props) = serde_json::from_str::<serde_json::Value>(&props_str) else {
            continue;
        };
        let Some(emb_arr) = props.get("_name_embedding").and_then(|v| v.as_array()) else {
            continue;
        };
        let existing_emb: Vec<f32> = emb_arr
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        if existing_emb.len() != candidate_emb.len() {
            continue;
        }
        let sim = cosine_similarity(candidate_emb, &existing_emb);
        if sim >= 0.87 {
            match &best {
                Some((_, prev_sim)) if *prev_sim >= sim => {}
                _ => best = Some((id, sim)),
            }
        }
    }
    Ok(best.map(|(id, _)| id))
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Add a new alias to an existing entity's alias list (JSON array).
/// Caller handles the actual DB update; this just computes the new JSON.
pub fn merge_alias(existing_aliases_json: Option<&str>, new_alias: &str) -> String {
    let mut list: Vec<String> = existing_aliases_json
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();

    let norm_new = normalize_name(new_alias);
    if !list.iter().any(|a| normalize_name(a) == norm_new) {
        list.push(new_alias.to_string());
    }
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_lowercases_and_trims() {
        assert_eq!(normalize_name("  Savarkar  "), "savarkar");
        assert_eq!(normalize_name("V.D. Savarkar"), "vd savarkar");
    }

    #[test]
    fn normalize_name_strips_honorifics() {
        assert_eq!(normalize_name("Dr. Ambedkar"), "ambedkar");
        assert_eq!(normalize_name("Mrs. Gandhi"), "gandhi");
        assert_eq!(normalize_name("Shri Patel"), "patel");
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn levenshtein_handles_savarkar_variants() {
        // "savarkar" vs "v.d. savarkar" → distance 5 (too much for fuzzy)
        // but "savarkar" vs "savarker" → distance 1 (matches)
        assert_eq!(levenshtein("savarkar", "savarker"), 1);
    }

    #[test]
    fn cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_is_zero() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[]), 0.0);
    }

    #[test]
    fn merge_alias_dedups_normalized() {
        let existing = Some(r#"["V.D. Savarkar"]"#);
        let merged = merge_alias(existing, "v.d. savarkar"); // case diff only
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list.len(), 1, "Case/punct variants should dedup");
    }

    #[test]
    fn merge_alias_adds_new() {
        let existing = Some(r#"["Savarkar"]"#);
        let merged = merge_alias(existing, "V.D. Savarkar");
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn merge_alias_from_empty() {
        let merged = merge_alias(None, "Savarkar");
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list, vec!["Savarkar"]);
    }

    #[test]
    fn alias_list_contains_matches_normalized() {
        assert!(alias_list_contains(r#"["V.D. Savarkar"]"#, "vd savarkar"));
        assert!(!alias_list_contains(r#"["V.D. Savarkar"]"#, "gandhi"));
    }
}
```

- [ ] **Step 2: Export from lib.rs**

In `services/knowledge-graph/src/lib.rs`:

```rust
pub mod resolver;
pub use resolver::{levenshtein, merge_alias, normalize_name, resolve, MatchReason, ResolveOutcome};
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package knowledge-graph -- resolver`
Expected: 10 tests pass.

- [ ] **Step 4: Quality checks**

Run: `cargo fmt --all && cargo clippy --package knowledge-graph -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/resolver.rs services/knowledge-graph/src/lib.rs
git commit -m "feat(kg): EntityResolver — 3-stage cascade (exact / fuzzy / embedding) for entity dedup"
```

---

### Task 3: Wire Resolver Into Storage Writes

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`

- [ ] **Step 1: Read the existing store_knowledge method**

Find `pub async fn store_knowledge(&self, agent_id: &str, knowledge: ExtractedKnowledge) -> GraphResult<()>`. Note how entities are inserted.

- [ ] **Step 2: Add resolver call in the insert path**

Before INSERT, call `resolver::resolve()`. On `Merge`, update the existing entity's mention count + add alias. On `Create`, proceed with INSERT.

Add a helper method to storage:

```rust
/// Insert or merge an entity using EntityResolver.
/// Returns the final entity ID (existing or new) and whether it was merged.
async fn upsert_entity_with_resolver(
    &self,
    agent_id: &str,
    entity: &Entity,
) -> GraphResult<(String, bool)> {
    let conn = self.conn.lock().await;
    let outcome = resolver::resolve(&conn, agent_id, entity, None)
        .map_err(|e| GraphError::Other(e))?;

    match outcome {
        resolver::ResolveOutcome::Merge { existing_id, reason } => {
            tracing::debug!(
                new_name = %entity.name,
                existing_id = %existing_id,
                reason = ?reason,
                "Merging entity variant into existing"
            );
            // Add the new name as an alias
            let current_aliases: Option<String> = conn
                .query_row(
                    "SELECT aliases FROM kg_entities WHERE id = ?1",
                    params![existing_id],
                    |r| r.get(0),
                )
                .ok();
            let new_aliases = resolver::merge_alias(current_aliases.as_deref(), &entity.name);
            conn.execute(
                "UPDATE kg_entities SET aliases = ?1, mention_count = mention_count + 1, \
                 last_seen_at = ?2 WHERE id = ?3",
                params![new_aliases, chrono::Utc::now().to_rfc3339(), existing_id],
            )?;
            Ok((existing_id, true))
        }
        resolver::ResolveOutcome::Create => {
            // Existing INSERT code path — reuse the current logic
            // that inserts a new entity row
            self.insert_entity_row(&conn, agent_id, entity)?;
            Ok((entity.id.clone(), false))
        }
    }
}
```

Update `store_knowledge` to call this helper for each entity, collecting the final ID (merged or new) into the entity_map for relationship resolution.

- [ ] **Step 3: Extract insert_entity_row as a private helper**

If not already extracted, move the INSERT SQL into a private method:

```rust
fn insert_entity_row(&self, conn: &Connection, agent_id: &str, entity: &Entity) -> GraphResult<()> {
    // Existing INSERT SQL
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --package knowledge-graph`

- [ ] **Step 5: Run tests**

Run: `cargo test --package knowledge-graph`
Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add services/knowledge-graph/src/storage.rs
git commit -m "feat(kg): wire EntityResolver into storage writes with alias accumulation"
```

---

### Task 4: Expanded Distillation Prompt

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

The existing prompt (already modified in prior phases for procedure/graph direction) needs:
- New entity type examples + property schemas per type
- New relationship vocabulary grouped by category with directional examples
- A concrete few-shot extraction example

- [ ] **Step 1: Locate the prompt constant**

In `distillation.rs`, find `DEFAULT_DISTILLATION_PROMPT`. Note existing sections.

- [ ] **Step 2: Replace the "Entity Types" section**

Replace the existing entity types section with:

```
## Entity Types

Choose the most specific type that fits:

- `person` — individuals by name. Properties: {birth_date, death_date, nationality, occupation}
- `organization` — companies, parties, groups. Properties: {founding_date, dissolution_date, type, location}
- `location` — countries, cities, regions, coordinates. Properties: {country, region, type}
- `event` — historical events, meetings, conferences, sessions. Properties: {start_date, end_date, location, outcome}
- `time_period` — years, eras, date ranges. Properties: {start, end, era}
- `document` — books, articles, PDFs, URLs. Properties: {author, publisher, publication_date, source_url}
- `role` — position title held by a person at a time. Properties: {organization, start_date, end_date}
- `artifact` — generated files, reports, data outputs. Properties: {format, generator}
- `ward` — workspace/container. Properties: {purpose}
- `concept` — abstract ideas, methodologies, topics. Properties: {domain}
- `tool` — libraries, frameworks, technologies. Properties: {version, language}
- `project` — software projects or initiatives. Properties: {language, framework}
- `file` — important ward files. Properties: {path, exports, purpose}

Include `properties` populated appropriately for the type. Use ISO 8601 for dates when available.
```

- [ ] **Step 3: Replace the "Relationship Types" section**

```
## Relationship Types (directional — `source --type--> target`)

**Temporal**:
- `before(A, B)` — A happened before B
- `after(A, B)` — A happened after B
- `during(A, B)` — A happened during B
- `concurrent_with(A, B)` — A and B happened at the same time
- `succeeded_by(A, B)` — A was succeeded by B
- `preceded_by(A, B)` — A was preceded by B

**Role-based**:
- `president_of(P, O)` — P is/was president of O
- `founder_of(P, O)` — P founded O
- `member_of(P, O)` — P is a member of O
- `author_of(P, D)` — P authored document D
- `held_role(P, R)` — P held role R
- `employed_by(P, O)` — P is employed by O

**Spatial**:
- `located_in(X, L)` — X is located in L
- `held_at(E, L)` — event E was held at L
- `born_in(P, L)` — P was born in L
- `died_in(P, L)` — P died in L

**Causal**:
- `caused(A, B)` — A caused B
- `enabled(A, B)` — A enabled B
- `prevented(A, B)` — A prevented B
- `triggered_by(A, B)` — A was triggered by B

**Hierarchical**:
- `part_of(A, B)` — A is part of B
- `contains(A, B)` — A contains B
- `instance_of(A, T)` — A is an instance of type T
- `subtype_of(T1, T2)` — T1 is a subtype of T2

**Generic** (use when no specific type fits):
- `uses, created, related_to, exports, has_module, analyzed_by, prefers, mentions`

## Relationship Rules

- ALWAYS use the most specific relationship type that fits.
- NEVER use both `A uses B` and `B uses A` for the same pair.
- For role/presidency: emit `PersonX president_of OrgY`, NOT the reverse.
- Date-qualified relationships: if a relationship had a time range, mention it in the entity's properties (e.g., the Role entity's start_date/end_date).
```

- [ ] **Step 4: Add a few-shot example**

Before the "Rules" section, add:

```
## Example Extraction (for grounding)

Given this transcript snippet:
> "V.D. Savarkar was the president of Hindu Mahasabha from 1937 to 1943, during which time the Ahmedabad Session of 1937 was held."

A high-quality extraction looks like:

{
  "facts": [
    {"category": "domain", "key": "hindu_mahasabha.savarkar.presidency",
     "content": "V.D. Savarkar served as president of Hindu Mahasabha from 1937 to 1943",
     "confidence": 0.95}
  ],
  "entities": [
    {"name": "V.D. Savarkar", "type": "person",
     "properties": {"role": "Indian independence activist"}},
    {"name": "Hindu Mahasabha", "type": "organization",
     "properties": {"type": "political", "founding_date": "1915"}},
    {"name": "Ahmedabad Session 1937", "type": "event",
     "properties": {"start_date": "1937", "location": "Ahmedabad"}},
    {"name": "Ahmedabad", "type": "location",
     "properties": {"country": "India", "type": "city"}}
  ],
  "relationships": [
    {"source": "V.D. Savarkar", "target": "Hindu Mahasabha", "type": "president_of"},
    {"source": "Ahmedabad Session 1937", "target": "Ahmedabad", "type": "held_at"},
    {"source": "Ahmedabad Session 1937", "target": "Hindu Mahasabha", "type": "part_of"}
  ]
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --package gateway-execution`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): expanded ontology prompt with property schemas + few-shot example"
```

---

### Task 5: Final Checks

- [ ] **Step 1: Cognitive complexity audit**

Run: `cargo clippy --package knowledge-graph --package gateway-execution --lib --tests -- -D warnings -W clippy::cognitive_complexity 2>&1 | grep "cognitive complexity"`

Expected: no flags for new functions. Any existing code flags are pre-existing and acceptable.

- [ ] **Step 2: Full test run**

Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass. Count: 37+ test suites.

- [ ] **Step 3: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: Clean.

- [ ] **Step 4: UI**

Run: `cd apps/ui && npm run build && npm run lint`
Expected: Clean.

- [ ] **Step 5: Push**

```bash
git push
```
