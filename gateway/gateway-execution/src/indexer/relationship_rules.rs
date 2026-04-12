//! Field-name → relationship extraction rules for the Ward Artifact Indexer.
//!
//! Each rule inspects a JSON object and, when its field pattern matches,
//! emits candidate `(source_name, RelationshipType, target_name, target_type)`
//! tuples. The caller resolves names to entity IDs via `EntityResolver`.

// Rule stubs and public API are used by later tasks in the activation pack.
#![allow(dead_code)]

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
        rule(source_name, source_type.clone(), obj, &mut out);
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
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
fn rule_organization(
    source_name: &str,
    source_type: EntityType,
    obj: &Map<String, Value>,
    out: &mut Vec<RelationshipCandidate>,
) {
    let Some(target) = non_empty_string(obj.get("organization")) else {
        return;
    };
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
    let Some(target) = non_empty_string(obj.get("role")) else {
        return;
    };
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
    let Some(founder) = non_empty_string(obj.get("founder")) else {
        return;
    };
    // Direction inversion: person --founder_of--> org.
    out.push(RelationshipCandidate {
        source_name: founder,
        source_type: EntityType::Person,
        target_name: source_name.to_string(),
        target_type: source_type,
        relationship_type: RelationshipType::FounderOf,
    });
}
fn rule_founded_in(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}
fn rule_participants_reversed(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}
fn rule_date_year_during(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}
fn rule_author_reversed(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}
fn rule_born_in(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}
fn rule_died_in(
    _s: &str,
    _st: EntityType,
    _o: &Map<String, Value>,
    _out: &mut Vec<RelationshipCandidate>,
) {
}

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

    #[test]
    fn organization_emits_member_of() {
        let o = obj(json!({"organization": "Hindu Mahasabha"}));
        let out = extract("V.D. Savarkar", EntityType::Person, &o);
        assert!(out
            .iter()
            .any(|r| r.relationship_type == RelationshipType::MemberOf
                && r.target_name == "Hindu Mahasabha"
                && r.target_type == EntityType::Organization));
    }

    #[test]
    fn role_emits_held_role() {
        let o = obj(json!({"role": "President"}));
        let out = extract("V.D. Savarkar", EntityType::Person, &o);
        assert!(out
            .iter()
            .any(|r| r.relationship_type == RelationshipType::HeldRole
                && r.target_name == "President"
                && r.target_type == EntityType::Role));
    }

    #[test]
    fn founder_is_reversed_person_founder_of_org() {
        let o = obj(json!({"founder": "B.S. Moonje"}));
        let out = extract("Hindu Mahasabha", EntityType::Organization, &o);
        let r = out
            .iter()
            .find(|r| r.relationship_type == RelationshipType::FounderOf)
            .expect("founder_of relationship");
        assert_eq!(r.source_name, "B.S. Moonje");
        assert_eq!(r.source_type, EntityType::Person);
        assert_eq!(r.target_name, "Hindu Mahasabha");
        assert_eq!(r.target_type, EntityType::Organization);
    }
}
