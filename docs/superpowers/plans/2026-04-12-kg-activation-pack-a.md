# KG Activation Pack A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the knowledge graph from sparse-and-dormant into connected-and-used by extracting relationships from ward artifacts, teaching agents to call `graph_query`, and verifying multi-hop defaults.

**Architecture:** Three loosely coupled changes — (1) extend `WardArtifactIndexer` with field-name → relationship rules that reuse the existing `EntityResolver` for endpoint deduplication; (2) edit two prompt shards to surface the `graph_query` tool with explicit triggers; (3) audit/confirm `GraphTraversalConfig` defaults are in effect, then add an admin endpoint to force re-indexing of existing wards.

**Tech Stack:** Rust 2024 (tokio, axum, rusqlite via existing repos), existing `EntityResolver` (`services/knowledge-graph/src/resolver.rs`), existing `store_knowledge` ID-remapping path (`services/knowledge-graph/src/storage.rs:39`). Markdown for prompt shards.

**Spec:** `docs/superpowers/specs/2026-04-12-kg-activation-pack-a-design.md`

---

## File Structure

**Created:**
- `gateway/gateway-execution/src/indexer/relationship_rules.rs` — schema-driven extraction rules; one private module reachable only from `ward_artifact_indexer.rs`.

**Modified:**
- `gateway/gateway-execution/src/ward_artifact_indexer.rs` — replace `relationships: vec![]` with rule-driven emission; add `force_reindex` flag; accept `relationship_rules` helper; re-home existing file under a `indexer/` submodule only if the file exceeds 500 lines after changes (otherwise keep flat).
- `gateway/src/http/graph.rs` — add `reindex_all_wards` handler.
- `gateway/src/http/mod.rs` — register `POST /api/graph/reindex` route.
- `gateway/templates/shards/tooling_skills.md` — add `graph_query` section with triggers.
- `gateway/templates/shards/memory_learning.md` — add two usage examples.
- `gateway/gateway-services/src/recall_config.rs` — audit-only; add a regression test asserting defaults remain correct.
- `memory-bank/components/memory-layer/knowledge-graph.md` — document the new rules + the admin endpoint.

**Tests added (all inline with source files following existing project convention):**
- `gateway/gateway-execution/src/indexer/relationship_rules.rs` — per-rule unit tests.
- `gateway/gateway-execution/src/ward_artifact_indexer.rs` — integration tests for indexer end-to-end.
- `gateway/gateway-services/src/recall_config.rs` — defaults-lock test.

---

## Task 1: Scaffold `relationship_rules` module

**Files:**
- Create: `gateway/gateway-execution/src/indexer/mod.rs`
- Create: `gateway/gateway-execution/src/indexer/relationship_rules.rs`
- Modify: `gateway/gateway-execution/src/lib.rs` (add `pub mod indexer;` line, keeping existing `pub mod ward_artifact_indexer;` for now)

- [ ] **Step 1: Write the failing test file skeleton**

Create `gateway/gateway-execution/src/indexer/relationship_rules.rs`:

```rust
//! Field-name → relationship extraction rules for the Ward Artifact Indexer.
//!
//! Each rule inspects a JSON object and, when its field pattern matches,
//! emits candidate `(source_name, RelationshipType, target_name, target_type)`
//! tuples. The caller resolves names to entity IDs via `EntityResolver`.

use knowledge_graph::{EntityType, RelationshipType};
use serde_json::{Map, Value};

/// A pending relationship with resolved-by-name endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipCandidate {
    pub source_name: String,
    pub source_type: EntityType,
    pub target_name: String,
    pub target_type: EntityType,
    pub relationship_type: RelationshipType,
}

/// Apply every rule to an object given its inferred source entity (name + type).
/// Returns zero or more relationship candidates. Unknown fields are ignored.
pub fn extract(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
) -> Vec<RelationshipCandidate> {
    let mut out = Vec::new();
    for rule in RULES {
        rule(source_name, source_type, obj, &mut out);
    }
    out
}

type Rule = fn(&str, EntityType, &Map<String, Value>, &mut Vec<RelationshipCandidate>);

const RULES: &[Rule] = &[
    rule_location,
    rule_organization,
    rule_role,
    rule_founder_reversed,
    rule_founded_in,
    rule_participants_reversed,
    rule_date_year_during,
    rule_author_reversed,
    rule_born_in,
    rule_died_in,
];

// --- Individual rules are added in later tasks. Stubs below keep the build green. ---

fn rule_location(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_organization(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_role(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_founder_reversed(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_founded_in(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_participants_reversed(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_date_year_during(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_author_reversed(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_born_in(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}
fn rule_died_in(_s: &str, _st: EntityType, _o: &Map<String, Value>, _out: &mut Vec<RelationshipCandidate>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn no_rules_fire_on_empty_object() {
        let o = obj(json!({}));
        assert!(extract("X", EntityType::Concept, &o).is_empty());
    }
}
```

Create `gateway/gateway-execution/src/indexer/mod.rs`:

```rust
//! Indexer submodule: helpers used by `ward_artifact_indexer.rs`.

pub(crate) mod relationship_rules;
```

- [ ] **Step 2: Register module in `lib.rs`**

Edit `gateway/gateway-execution/src/lib.rs` — add `pub mod indexer;` under the existing `pub mod` declarations. Do not remove `pub mod ward_artifact_indexer;`.

- [ ] **Step 3: Build and run the placeholder test**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests::no_rules_fire_on_empty_object`

Expected: PASS (no rules fire on empty object because every rule stub is a no-op).

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/indexer/ gateway/gateway-execution/src/lib.rs
git commit -m "feat(kg): scaffold relationship_rules module for ward indexer"
```

---

## Task 2: Implement `rule_location`

**Files:**
- Modify: `gateway/gateway-execution/src/indexer/relationship_rules.rs`

Semantics:
- If `obj.location` is a non-empty string, emit `source --held_at--> location` when `source_type` is `Event`; otherwise emit `source --located_in--> location`.
- Target type always `EntityType::Location`.

- [ ] **Step 1: Write failing tests**

Append to the `tests` module:

```rust
#[test]
fn location_on_event_emits_held_at() {
    let o = obj(json!({"location": "Ahmedabad"}));
    let out = extract("Session 1937", EntityType::Event, &o);
    assert_eq!(out.len(), 1);
    let r = &out[0];
    assert_eq!(r.source_name, "Session 1937");
    assert_eq!(r.target_name, "Ahmedabad");
    assert_eq!(r.target_type, EntityType::Location);
    assert_eq!(r.relationship_type, RelationshipType::HeldAt);
}

#[test]
fn location_on_non_event_emits_located_in() {
    let o = obj(json!({"location": "Mumbai"}));
    let out = extract("Acme Corp", EntityType::Organization, &o);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].relationship_type, RelationshipType::LocatedIn);
}

#[test]
fn empty_or_missing_location_emits_nothing() {
    assert!(extract("X", EntityType::Event, &obj(json!({}))).is_empty());
    assert!(extract("X", EntityType::Event, &obj(json!({"location": ""}))).is_empty());
    assert!(extract("X", EntityType::Event, &obj(json!({"location": null}))).is_empty());
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: the three new tests FAIL (current stub emits nothing, assertions compare len>0).

- [ ] **Step 3: Implement `rule_location`**

Replace the `rule_location` stub with:

```rust
fn rule_location(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("location")) else {
        return;
    };
    let rel = if matches!(source_type, EntityType::Event) {
        RelationshipType::HeldAt
    } else {
        RelationshipType::LocatedIn
    };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Location,
        relationship_type: rel,
    });
}

/// Return `Some(s.trim().to_owned())` only when the value is a non-empty string.
fn non_empty_string(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?.trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/indexer/relationship_rules.rs
git commit -m "feat(kg): relationship rule — location → held_at/located_in"
```

---

## Task 3: Implement `rule_organization`, `rule_role`, `rule_founder_reversed`

**Files:**
- Modify: `gateway/gateway-execution/src/indexer/relationship_rules.rs`

Semantics:
- `obj.organization` (string) → `source --member_of--> organization`. Target type `Organization`.
- `obj.role` (string) → `source --held_role--> role`. Target type `Role`.
- `obj.founder` (string) → `founder_name --founder_of--> source`. Reversed direction. Target-type of the final relationship's source is `Person`; final target retains `source_type` (the org).

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn organization_emits_member_of() {
    let o = obj(json!({"organization": "Hindu Mahasabha"}));
    let out = extract("V.D. Savarkar", EntityType::Person, &o);
    assert!(out.iter().any(|r| r.relationship_type == RelationshipType::MemberOf
        && r.target_name == "Hindu Mahasabha"
        && r.target_type == EntityType::Organization));
}

#[test]
fn role_emits_held_role() {
    let o = obj(json!({"role": "President"}));
    let out = extract("V.D. Savarkar", EntityType::Person, &o);
    assert!(out.iter().any(|r| r.relationship_type == RelationshipType::HeldRole
        && r.target_name == "President"
        && r.target_type == EntityType::Role));
}

#[test]
fn founder_is_reversed_person_founder_of_org() {
    let o = obj(json!({"founder": "B.S. Moonje"}));
    let out = extract("Hindu Mahasabha", EntityType::Organization, &o);
    let r = out.iter()
        .find(|r| r.relationship_type == RelationshipType::FounderOf)
        .expect("founder_of relationship");
    assert_eq!(r.source_name, "B.S. Moonje");
    assert_eq!(r.source_type, EntityType::Person);
    assert_eq!(r.target_name, "Hindu Mahasabha");
    assert_eq!(r.target_type, EntityType::Organization);
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: the three new tests FAIL.

- [ ] **Step 3: Implement the three rules**

Replace the three corresponding stubs:

```rust
fn rule_organization(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("organization")) else { return; };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Organization,
        relationship_type: RelationshipType::MemberOf,
    });
}

fn rule_role(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("role")) else { return; };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Role,
        relationship_type: RelationshipType::HeldRole,
    });
}

fn rule_founder_reversed(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(founder) = non_empty_string(obj.get("founder")) else { return; };
    // Direction inversion: person --founder_of--> org.
    out.push(RelationshipCandidate {
        source_name: founder,
        source_type: EntityType::Person,
        target_name: source_name.to_string(),
        target_type: source_type,
        relationship_type: RelationshipType::FounderOf,
    });
}
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/indexer/relationship_rules.rs
git commit -m "feat(kg): rules — organization, role, founder (reversed)"
```

---

## Task 4: Implement remaining rules (`founded_in`, `participants`, `date/year`, `author`, `born_in`, `died_in`)

**Files:**
- Modify: `gateway/gateway-execution/src/indexer/relationship_rules.rs`

Semantics:
- `obj.founded_in` (string) → `source --located_in--> location`. Target type `Location`.
- `obj.participants` (array of strings) → for each string `p`: emit `p --participant--> source` (inverted). Participant's `source_type` becomes `Person`; target retains incoming `source_type`. Uses the generic relationship `RelationshipType::Custom("participant".into())` since the enum has no direct variant.
- `obj.date` or `obj.year` (string or integer) → `source --during--> time_period` where `time_period` name is the stringified value. Target type `TimePeriod`.
- `obj.author` (string) → `author_name --author_of--> source`. Inverted, similar to founder.
- `obj.born_in` (string) → `source --born_in--> location`. Target type `Location`.
- `obj.died_in` (string) → `source --died_in--> location`. Target type `Location`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn founded_in_emits_located_in() {
    let o = obj(json!({"founded_in": "Pune"}));
    let out = extract("Hindu Mahasabha", EntityType::Organization, &o);
    assert!(out.iter().any(|r| r.relationship_type == RelationshipType::LocatedIn
        && r.target_name == "Pune"));
}

#[test]
fn participants_inverted_emits_one_edge_per_participant() {
    let o = obj(json!({"participants": ["Alice", "Bob"]}));
    let out = extract("Ahmedabad Session 1937", EntityType::Event, &o);
    let count = out.iter()
        .filter(|r| matches!(&r.relationship_type, RelationshipType::Custom(s) if s == "participant"))
        .count();
    assert_eq!(count, 2);
    let alice = out.iter()
        .find(|r| r.source_name == "Alice" && matches!(&r.relationship_type, RelationshipType::Custom(s) if s == "participant"))
        .expect("alice edge");
    assert_eq!(alice.target_name, "Ahmedabad Session 1937");
    assert_eq!(alice.source_type, EntityType::Person);
}

#[test]
fn date_string_emits_during_time_period() {
    let o = obj(json!({"date": "1937"}));
    let out = extract("Session", EntityType::Event, &o);
    let r = out.iter()
        .find(|r| r.relationship_type == RelationshipType::During)
        .expect("during edge");
    assert_eq!(r.target_name, "1937");
    assert_eq!(r.target_type, EntityType::TimePeriod);
}

#[test]
fn year_integer_emits_during() {
    let o = obj(json!({"year": 1937}));
    let out = extract("Session", EntityType::Event, &o);
    let r = out.iter()
        .find(|r| r.relationship_type == RelationshipType::During)
        .expect("during edge");
    assert_eq!(r.target_name, "1937");
}

#[test]
fn author_inverted_emits_author_of() {
    let o = obj(json!({"author": "V.D. Savarkar"}));
    let out = extract("Hindutva", EntityType::Document, &o);
    let r = out.iter()
        .find(|r| r.relationship_type == RelationshipType::AuthorOf)
        .expect("author_of edge");
    assert_eq!(r.source_name, "V.D. Savarkar");
    assert_eq!(r.source_type, EntityType::Person);
    assert_eq!(r.target_name, "Hindutva");
}

#[test]
fn born_died_emit_location_edges() {
    let o1 = obj(json!({"born_in": "Bhagur"}));
    let out1 = extract("V.D. Savarkar", EntityType::Person, &o1);
    assert!(out1.iter().any(|r| r.relationship_type == RelationshipType::BornIn
        && r.target_name == "Bhagur"));

    let o2 = obj(json!({"died_in": "Bombay"}));
    let out2 = extract("V.D. Savarkar", EntityType::Person, &o2);
    assert!(out2.iter().any(|r| r.relationship_type == RelationshipType::DiedIn
        && r.target_name == "Bombay"));
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: the new tests FAIL.

- [ ] **Step 3: Implement the six rules**

Replace the remaining stubs:

```rust
fn rule_founded_in(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("founded_in")) else { return; };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Location,
        relationship_type: RelationshipType::LocatedIn,
    });
}

fn rule_participants_reversed(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(arr) = obj.get("participants").and_then(|v| v.as_array()) else { return; };
    for item in arr {
        let Some(name) = item.as_str() else { continue; };
        let name = name.trim();
        if name.is_empty() { continue; }
        out.push(RelationshipCandidate {
            source_name: name.to_string(),
            source_type: EntityType::Person,
            target_name: source_name.to_string(),
            target_type: source_type,
            relationship_type: RelationshipType::Custom("participant".to_string()),
        });
    }
}

fn rule_date_year_during(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let raw = obj.get("date").or_else(|| obj.get("year"));
    let Some(raw) = raw else { return; };
    let label = match raw {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() { return; } else { t.to_string() }
        }
        Value::Number(n) => n.to_string(),
        _ => return,
    };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: label,
        target_type: EntityType::TimePeriod,
        relationship_type: RelationshipType::During,
    });
}

fn rule_author_reversed(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(author) = non_empty_string(obj.get("author")) else { return; };
    out.push(RelationshipCandidate {
        source_name: author,
        source_type: EntityType::Person,
        target_name: source_name.to_string(),
        target_type: source_type,
        relationship_type: RelationshipType::AuthorOf,
    });
}

fn rule_born_in(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("born_in")) else { return; };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Location,
        relationship_type: RelationshipType::BornIn,
    });
}

fn rule_died_in(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("died_in")) else { return; };
    out.push(RelationshipCandidate {
        source_name: source_name.to_string(),
        source_type,
        target_name: target,
        target_type: EntityType::Location,
        relationship_type: RelationshipType::DiedIn,
    });
}
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p gateway-execution --lib indexer::relationship_rules::tests`

Expected: all tests PASS.

- [ ] **Step 5: Verify EntityType variants exist**

If `cargo build -p gateway-execution` fails because `EntityType::Event`, `EntityType::TimePeriod`, `EntityType::Role`, `EntityType::Document`, or `EntityType::Location` is missing, open `services/knowledge-graph/src/types.rs` and confirm the 13-variant enum from the memory-bank spec is present. It should be (Phase 6a+ landed). If any variant is missing this is a precondition failure — stop and escalate.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/indexer/relationship_rules.rs
git commit -m "feat(kg): remaining rules — founded_in, participants, date, author, born/died_in"
```

---

## Task 5: Wire rules into `WardArtifactIndexer` + emit relationships

**Files:**
- Modify: `gateway/gateway-execution/src/ward_artifact_indexer.rs`

Goal: change `relationships: vec![]` at line 142 to a computed vec. Extract helper that, given an emitted `Entity` + the JSON object it came from, produces `Vec<knowledge_graph::Relationship>` by calling `relationship_rules::extract` and synthesizing target `Entity` stubs so `store_knowledge`'s id-remap path deduplicates.

Key constraint: `store_knowledge` (storage.rs:39) remaps `source_entity_id` and `target_entity_id` through `entity_id_map` built from `knowledge.entities`. Therefore **every target entity referenced by a relationship MUST also appear in `knowledge.entities`** so its ID lands in the map.

- [ ] **Step 1: Add helper that builds entity+rels for one object**

Add near the bottom of `ward_artifact_indexer.rs`, above `#[cfg(test)]`:

```rust
use crate::indexer::relationship_rules::{self, RelationshipCandidate};
use knowledge_graph::{Relationship, RelationshipType};

/// For one parsed object, return: the primary entity, plus any additional
/// target entities referenced by rule outputs, plus the relationships.
fn entity_with_relationships(
    primary: Entity,
    obj: &serde_json::Map<String, Value>,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
) -> (Vec<Entity>, Vec<Relationship>) {
    let candidates = relationship_rules::extract(
        &primary.name,
        primary.entity_type,
        obj,
    );

    let primary_id = primary.id.clone();
    let primary_name = primary.name.clone();
    let primary_type = primary.entity_type;

    let mut extra_entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<Relationship> = Vec::new();
    // name -> id map within this object (so the same target shares an ID
    // across multiple rules before `store_knowledge` dedups globally)
    let mut name_to_id: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    name_to_id.insert(
        (primary_name.clone(), entity_type_key(primary_type)),
        primary_id.clone(),
    );

    for cand in candidates {
        let source_id = ensure_entity(
            &cand.source_name,
            cand.source_type,
            agent_id,
            episode_id,
            source_ref,
            &mut name_to_id,
            &mut extra_entities,
        );
        let target_id = ensure_entity(
            &cand.target_name,
            cand.target_type,
            agent_id,
            episode_id,
            source_ref,
            &mut name_to_id,
            &mut extra_entities,
        );
        relationships.push(Relationship::new(
            agent_id.to_string(),
            source_id,
            target_id,
            cand.relationship_type,
        ));
    }

    let mut all_entities = vec![primary];
    all_entities.extend(extra_entities);
    (all_entities, relationships)
}

fn ensure_entity(
    name: &str,
    entity_type: EntityType,
    agent_id: &str,
    episode_id: &str,
    source_ref: &str,
    map: &mut std::collections::HashMap<(String, String), String>,
    extras: &mut Vec<Entity>,
) -> String {
    let key = (name.to_string(), entity_type_key(entity_type));
    if let Some(id) = map.get(&key) {
        return id.clone();
    }
    let empty_props = serde_json::Map::new();
    let entity = build_entity(name, entity_type, &empty_props, agent_id, episode_id, source_ref);
    let id = entity.id.clone();
    map.insert(key, id.clone());
    extras.push(entity);
    id
}

fn entity_type_key(t: EntityType) -> String {
    format!("{:?}", t)
}
```

- [ ] **Step 2: Replace the `index_one_file` emission site**

In `index_one_file`, replace the block that currently builds `extract_entities(...)` then wraps it in `ExtractedKnowledge { entities, relationships: vec![] }` with:

```rust
    let schema = detect_collection_schema(&value);
    let primary_entities = extract_entities(&value, schema, agent_id, &episode.id, &source_ref);

    let mut all_entities: Vec<Entity> = Vec::new();
    let mut all_rels: Vec<Relationship> = Vec::new();

    // Re-walk the value so we can pair each primary entity with its source object.
    // For NamedObjectArray and DatedObjectArray, iterate the array in order.
    // For NamedObjectMap, iterate entries in the same order as extract_named_map.
    let paired_objects: Vec<serde_json::Map<String, Value>> =
        object_iter_for_schema(&value, schema);

    // Defensive: zip stops at the shorter length; extract_entities and the walker
    // should produce the same count, but if they diverge we silently emit only
    // entities (relationships skipped) to preserve best-effort behavior.
    for (entity, obj) in primary_entities.into_iter().zip(paired_objects.into_iter()) {
        let (ents, rels) = entity_with_relationships(
            entity,
            &obj,
            agent_id,
            &episode.id,
            &source_ref,
        );
        all_entities.extend(ents);
        all_rels.extend(rels);
    }

    let count = all_entities.len();
    if count > 0 {
        let knowledge = ExtractedKnowledge {
            entities: all_entities,
            relationships: all_rels,
        };
        graph
            .store_knowledge(agent_id, knowledge)
            .await
            .map_err(|e| format!("Graph store failed: {e}"))?;
    }

    Ok(count)
```

Also add a helper that walks the value to produce objects in the same order as `extract_entities`:

```rust
/// Return the JSON objects in the same iteration order used by
/// `extract_entities(schema=...)`. Callers pair-zip entity with its object.
fn object_iter_for_schema(
    value: &Value,
    schema: CollectionSchema,
) -> Vec<serde_json::Map<String, Value>> {
    match schema {
        CollectionSchema::NamedObjectArray | CollectionSchema::DatedObjectArray => value
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_object().cloned())
                    .collect()
            })
            .unwrap_or_default(),
        CollectionSchema::NamedObjectMap => value
            .as_object()
            .map(|obj| {
                obj.values()
                    .filter_map(|v| v.as_object().cloned())
                    .collect()
            })
            .unwrap_or_default(),
        CollectionSchema::Unknown => Vec::new(),
    }
}
```

Filter on primary-entity extraction parity: in both `extract_named_array`, `extract_dated_array`, `extract_named_map` — confirm every code path that **skips** an array/map entry (e.g. `filter_map` returning `None` because no `name`) is also skipped by `object_iter_for_schema`. Today `extract_named_array` skips objects without a name key; `object_iter_for_schema` does not. Align them:

Change `object_iter_for_schema` for `NamedObjectArray` to:

```rust
        CollectionSchema::NamedObjectArray => value
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_object().cloned())
                    .filter(|obj| obj.get("name").or_else(|| obj.get("title")).or_else(|| obj.get("label")).and_then(|v| v.as_str()).is_some())
                    .collect()
            })
            .unwrap_or_default(),
```

For `DatedObjectArray`, mirror the gating of `derive_event_name` by filtering for objects that have at least a name/title OR a year/date+description. Simplest correct filter:

```rust
        CollectionSchema::DatedObjectArray => value
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_object().cloned())
                    .filter(|obj| {
                        obj.get("name").and_then(|v| v.as_str()).is_some()
                            || obj.get("title").and_then(|v| v.as_str()).is_some()
                            || (obj.get("year").is_some() || obj.get("date").is_some())
                    })
                    .collect()
            })
            .unwrap_or_default(),
```

- [ ] **Step 3: Add end-to-end indexer test**

Add a new test at the bottom of the existing `#[cfg(test)] mod tests` in `ward_artifact_indexer.rs`:

```rust
    #[test]
    fn index_one_file_produces_relationships_for_people_json() {
        use serde_json::json;
        // Simulate parse+extract pipeline without hitting disk/DB.
        let value: Value = json!([
            {"name": "V.D. Savarkar", "organization": "Hindu Mahasabha", "role": "President", "born_in": "Bhagur"}
        ]);
        let schema = detect_collection_schema(&value);
        assert_eq!(schema, CollectionSchema::NamedObjectArray);

        let primary = extract_entities(&value, schema, "root", "ep-1", "/ward/people.json");
        assert_eq!(primary.len(), 1);

        let objs = object_iter_for_schema(&value, schema);
        assert_eq!(objs.len(), 1);

        let (entities, relationships) = entity_with_relationships(
            primary.into_iter().next().unwrap(),
            &objs[0],
            "root",
            "ep-1",
            "/ward/people.json",
        );

        // Primary + 3 targets (organization, role, location)
        assert!(entities.len() >= 4, "expected 4+ entities, got {}", entities.len());
        assert_eq!(relationships.len(), 3);
        let kinds: std::collections::HashSet<_> =
            relationships.iter().map(|r| format!("{:?}", r.relationship_type)).collect();
        assert!(kinds.iter().any(|k| k.contains("MemberOf")));
        assert!(kinds.iter().any(|k| k.contains("HeldRole")));
        assert!(kinds.iter().any(|k| k.contains("BornIn")));
    }
```

- [ ] **Step 4: Run all indexer tests**

Run: `cargo test -p gateway-execution --lib ward_artifact_indexer`

Expected: all tests PASS. If the new test fails because `Relationship::new` or `EntityType` imports are missing, adjust `use` statements at the top of `ward_artifact_indexer.rs`.

- [ ] **Step 5: Run clippy on the workspace**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: no warnings. Fix any complaints (likely unused imports or `clippy::too_many_arguments` — split or add targeted `#[allow]`).

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/ward_artifact_indexer.rs
git commit -m "feat(kg): emit relationships from ward artifacts using rule engine"
```

---

## Task 6: Add `force_reindex` flag + admin endpoint

**Files:**
- Modify: `gateway/gateway-execution/src/ward_artifact_indexer.rs`
- Modify: `gateway/src/http/graph.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add `index_ward_with_options`**

At the top of `ward_artifact_indexer.rs`, add a new entry point and refactor the existing `index_ward` to call it:

```rust
/// Options for ward indexing.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndexOptions {
    /// When true, bypass the content-hash dedup in `kg_episodes` and
    /// re-process every file. Safe to re-run; relationships upsert via
    /// UNIQUE(source, target, type) and bump mention_count.
    pub force_reindex: bool,
}

pub async fn index_ward_with_options(
    ward_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
    opts: IndexOptions,
) -> usize {
    let mut created = 0_usize;
    let files = collect_structured_files(ward_path);

    for file_path in files {
        match index_one_file_with_options(&file_path, session_id, agent_id, episode_repo, graph, opts).await {
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
        force_reindex = opts.force_reindex,
        "Ward artifact indexing complete"
    );
    created
}
```

Change the existing `pub async fn index_ward` body to:

```rust
pub async fn index_ward(
    ward_path: &Path,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) -> usize {
    index_ward_with_options(ward_path, session_id, agent_id, episode_repo, graph, IndexOptions::default()).await
}
```

Rename `index_one_file` to `index_one_file_with_options` and add an `opts: IndexOptions` parameter. In the dedup-check block, gate the skip:

```rust
    if !opts.force_reindex
        && episode_repo
            .get_by_content_hash(&content_hash, EpisodeSource::WardFile.as_str())
            .map_err(|e| format!("Dedup check failed: {e}"))?
            .is_some()
    {
        tracing::debug!(file = ?file_path, "Skipping already-indexed ward file");
        return Ok(0);
    }
```

The `upsert_episode` call already handles the UNIQUE conflict on `(content_hash, source_type)`; no change needed there.

Keep any other internal callers of `index_one_file` on the new signature (if none, the rename is clean).

- [ ] **Step 2: Add reindex HTTP handler**

In `gateway/src/http/graph.rs`, append:

```rust
/// Response body for the reindex endpoint.
#[derive(Debug, Serialize)]
pub struct ReindexResponse {
    pub wards_processed: usize,
    pub entities_created: usize,
}

/// POST /api/graph/reindex — force re-indexing of every ward on disk.
/// Idempotent: relationships upsert via UNIQUE(source, target, type).
pub async fn reindex_all_wards(
    State(state): State<AppState>,
) -> Result<Json<ReindexResponse>, StatusCode> {
    use gateway_execution::ward_artifact_indexer::{
        index_ward_with_options, IndexOptions,
    };

    let ward_service = state.ward_service.clone();
    let wards = ward_service
        .list_wards()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut total_entities = 0_usize;
    let mut wards_processed = 0_usize;

    for ward in wards {
        let path = match ward_service.ward_path(&ward.id).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let n = index_ward_with_options(
            &path,
            "admin-reindex",
            "root",
            &state.kg_episode_repo,
            &state.graph_storage,
            IndexOptions { force_reindex: true },
        )
        .await;
        total_entities += n;
        wards_processed += 1;
    }

    Ok(Json(ReindexResponse {
        wards_processed,
        entities_created: total_entities,
    }))
}
```

Before committing this file, confirm the field names on `AppState` match (`ward_service`, `kg_episode_repo`, `graph_storage`). If a name differs, grep `AppState` in `gateway/src/state.rs` and adjust the call site:

Run: `rg 'pub struct AppState' gateway/src/state.rs -A 40`

If a field is absent, this task is blocked — escalate. Do NOT invent state.

- [ ] **Step 3: Register the route**

In `gateway/src/http/mod.rs`, in the block where other `/api/graph/...` routes are declared (around line 220), add:

```rust
        .route("/api/graph/reindex", post(graph::reindex_all_wards))
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check --workspace`

Expected: clean compile.

- [ ] **Step 5: Integration test for idempotency**

Append to the `#[cfg(test)] mod tests` in `ward_artifact_indexer.rs`:

```rust
    #[tokio::test]
    async fn force_reindex_does_not_duplicate_relationships() {
        use gateway_database::{DatabasePool, KgEpisodeRepository};
        use knowledge_graph::GraphStorage;
        use std::sync::Arc;
        use tempfile::tempdir;

        let tmp = tempdir().unwrap();
        let ward = tmp.path().join("test-ward");
        std::fs::create_dir_all(&ward).unwrap();
        std::fs::write(
            ward.join("people.json"),
            r#"[{"name": "Alpha", "organization": "Acme", "role": "Chair"}]"#,
        ).unwrap();

        // Use in-memory / tempdir-backed DBs for episodes and graph.
        let conv_db = DatabasePool::new_in_memory().await.expect("conv db");
        let kg_db = DatabasePool::new_in_memory().await.expect("kg db");
        let episode_repo = KgEpisodeRepository::new(conv_db.clone());
        let graph = Arc::new(GraphStorage::new(kg_db).await.expect("graph"));

        // First pass
        let n1 = index_ward_with_options(
            &ward, "sess-1", "root", &episode_repo, &graph,
            IndexOptions { force_reindex: false },
        ).await;
        assert!(n1 > 0, "first pass should create entities");

        // Count relationships after first pass.
        let rels1 = graph.get_relationships("root").await.expect("rels1");
        assert!(!rels1.is_empty(), "first pass should create relationships");
        let count1 = rels1.len();

        // Second pass with force_reindex — mention_count bumps, no new rows.
        let _n2 = index_ward_with_options(
            &ward, "sess-2", "root", &episode_repo, &graph,
            IndexOptions { force_reindex: true },
        ).await;
        let rels2 = graph.get_relationships("root").await.expect("rels2");
        assert_eq!(rels2.len(), count1, "relationship count must be unchanged");
        // At least one relationship should have mention_count >= 2.
        assert!(rels2.iter().any(|r| r.mention_count >= 2),
            "expected at least one relationship with bumped mention_count");
    }
```

Verify `DatabasePool::new_in_memory` and `GraphStorage::get_relationships(agent_id)` exist. If either signature differs:
- Run `rg 'pub async fn new_in_memory' gateway/gateway-database/src`
- Run `rg 'pub async fn get_relationships' services/knowledge-graph/src`

Adjust the test's API calls to whatever the real signatures are. If the primitives don't exist and creating them is out of scope, convert this to a narrower test that only exercises `entity_with_relationships` directly (already covered by Task 5 Step 3) and instead assert the `UNIQUE(source,target,type)` contract via a new small test that calls `GraphStorage::store_knowledge` twice with the same `(entity_id, entity_id, type)` triple in memory.

- [ ] **Step 6: Run the new test**

Run: `cargo test -p gateway-execution --lib force_reindex_does_not_duplicate_relationships`

Expected: PASS.

- [ ] **Step 7: Clippy + fmt**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-execution/src/ward_artifact_indexer.rs gateway/src/http/graph.rs gateway/src/http/mod.rs
git commit -m "feat(kg): force_reindex flag + POST /api/graph/reindex endpoint"
```

---

## Task 7: Teach agents about `graph_query`

**Files:**
- Modify: `gateway/templates/shards/tooling_skills.md`
- Modify: `gateway/templates/shards/memory_learning.md`

- [ ] **Step 1: Add `graph_query` section to tooling_skills.md**

Append at the end of the file:

```markdown
## Knowledge Graph

### graph_query
Query the knowledge graph of entities and relationships accumulated from prior sessions, ward artifacts, and tool results.

Three actions:
- `graph_query(action="search", query="<name>", entity_type?, limit?)` — find entities whose name contains the query string.
- `graph_query(action="neighbors", entity_name="<name>", direction?="both", depth?=1, limit?)` — list entities connected to this one. `depth=2` traverses 2 hops.
- `graph_query(action="context", query="<topic>", limit?)` — semantic search plus surrounding subgraph for a topic.

When to call:
- User mentions a named entity (person, organization, location, document, tool) you don't already have context on from the session's recall block → `search` it first.
- You need to understand how two or more entities relate, or identify the central figures in a domain → `neighbors` with `depth=2`.
- Starting a research task on a topic you've touched before → `context` to retrieve the relevant subgraph.

When NOT to call:
- For simple fact lookup that should live in `memory_facts` — use `memory(action="recall")` instead.
- More than 2 consecutive `graph_query` calls on the same turn — if you're still lost, delegate to a subagent with the information you have.
```

- [ ] **Step 2: Add two examples to memory_learning.md**

Append to the file, after existing `memory(recall)` / `memory(save_fact)` guidance:

```markdown
## Graph Query Examples

Before answering about a named entity, check the graph:

```
# User asks: "what do you know about Hindu Mahasabha?"
graph_query(action="search", query="Hindu Mahasabha")
# → returns entity with mention_count, neighbor snippet

graph_query(action="neighbors", entity_name="Hindu Mahasabha", depth=2)
# → returns 2-hop subgraph: founders, members, affiliated orgs, events held at
```

Before delegating a ward-scoped research task:

```
graph_query(action="context", query="portfolio analysis", limit=30)
# → semantic search + subgraph; include the relevant named entities in the
#   delegation task body so the subagent has a head start
```
```

- [ ] **Step 3: Verify template loader still parses the shards**

Run: `cargo test -p gateway-services --lib template` (or the crate that owns the template loader — grep `template_loader` or `load_shard` if unsure).

Expected: tests that read template shards PASS.

If no such test exists, instead write a one-shot unit test in whichever crate owns the shard loader that asserts both `tooling_skills.md` and `memory_learning.md` load without error.

- [ ] **Step 4: Commit**

```bash
git add gateway/templates/shards/tooling_skills.md gateway/templates/shards/memory_learning.md
git commit -m "docs(kg): teach agents when to call graph_query"
```

---

## Task 8: Lock in traversal defaults (Fix 6)

**Files:**
- Modify: `gateway/gateway-services/src/recall_config.rs` (test-only)

The defaults are already correct (`enabled=true, max_hops=2`). Add an assertion so nobody silently changes them without also changing this plan.

- [ ] **Step 1: Add a defaults-lock regression test**

At the bottom of the `#[cfg(test)] mod tests` block in `recall_config.rs`, add:

```rust
    #[test]
    fn graph_traversal_defaults_remain_enabled_depth_two() {
        let c = RecallConfig::default();
        assert!(c.graph_traversal.enabled,
            "graph_traversal.enabled default must remain true (Pack A contract)");
        assert_eq!(c.graph_traversal.max_hops, 2,
            "graph_traversal.max_hops default must remain 2 (Pack A contract)");
        assert!(c.graph_traversal.max_graph_facts >= 5,
            "graph_traversal.max_graph_facts default must be >= 5");
    }
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p gateway-services --lib graph_traversal_defaults_remain_enabled_depth_two`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs
git commit -m "test(kg): lock graph traversal defaults as Pack A contract"
```

---

## Task 9: Update memory-bank documentation

**Files:**
- Modify: `memory-bank/components/memory-layer/knowledge-graph.md`

- [ ] **Step 1: Add a new section documenting the rules**

Open `memory-bank/components/memory-layer/knowledge-graph.md`. Find the section titled `### 2. Ward Artifact Indexer (Phase 6a)` and insert a new subsection immediately after it:

```markdown
#### Relationship Extraction (Pack A, 2026-04-12)

In addition to entities, the indexer emits relationships from well-known field names. Rules live in `gateway/gateway-execution/src/indexer/relationship_rules.rs`.

| JSON field | Direction | Relationship | Target type |
|---|---|---|---|
| `location` (on Event) | forward | `held_at` | Location |
| `location` (on other) | forward | `located_in` | Location |
| `organization` | forward | `member_of` | Organization |
| `role` | forward | `held_role` | Role |
| `founder` | **reversed** (person → org) | `founder_of` | (source org) |
| `founded_in` | forward | `located_in` | Location |
| `participants[]` | **reversed** (person → event) | `participant` | (source event) |
| `date` or `year` | forward | `during` | TimePeriod |
| `author` | **reversed** (person → doc) | `author_of` | (source doc) |
| `born_in` | forward | `born_in` | Location |
| `died_in` | forward | `died_in` | Location |

Target entities are synthesized and run through the same `EntityResolver` cascade as primary entities, so name variants collapse. Relationship writes are idempotent via `UNIQUE(source_entity_id, target_entity_id, relationship_type)`.

#### Force Re-index (Pack A)

`POST /api/graph/reindex` force-re-indexes every ward, bypassing the `kg_episodes` content-hash dedup. Safe to re-run; entity and relationship writes are idempotent.
```

- [ ] **Step 2: Commit**

```bash
git add memory-bank/components/memory-layer/knowledge-graph.md
git commit -m "docs(kg): document Pack A rules and reindex endpoint"
```

---

## Task 10: Full workspace validation

- [ ] **Step 1: Format**

Run: `cargo fmt --all`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: no warnings across the workspace. Fix any that appear; do NOT add crate-level `#![allow]`.

- [ ] **Step 3: Full test suite**

Run: `cargo test --workspace`

Expected: green.

- [ ] **Step 4: Push for review**

```bash
git push -u origin feature/sentient
```

(Do not open a PR automatically — wait for the human to review the branch.)

---

## Self-Review Results

**Spec coverage check:**
- G1 (orphan ratio ≤ 30% on fixture) — covered by Tasks 2–5 (rule implementations) + Task 5 Step 3 test + Task 6 integration test. Final orphan-ratio measurement happens post-deployment on the real Hindu Mahasabha ward via `POST /api/graph/reindex` then DB query.
- G2 (agents call `graph_query`) — covered by Task 7 shard edits. Final smoke test happens manually post-deploy; plan notes that.
- G3 (multi-hop default on) — covered by Task 8.
- G4 (DB heals via idempotent re-index) — covered by Task 6.

**Placeholder scan:** None. Every step has concrete code or concrete commands.

**Type consistency:** `RelationshipCandidate` fields and `entity_with_relationships` signature are used consistently across Tasks 1–6. `IndexOptions { force_reindex }` is the only settings struct and is referenced the same way in Task 6. `entity_type_key` is defined once in Task 5 and only used locally.

**One known risk:** Task 5 assumes `extract_entities` and `object_iter_for_schema` produce identically-filtered iterations. Step 2 explicitly realigns them and Step 3 adds a test that asserts lengths match. If a future schema change breaks this pairing, the pair-zip gracefully truncates (comment in code calls this out).
