# Rig/hooks/session/graph authoring review

Date: 2026-10-03
Brief: [rig-hooks-session-graph](../product/briefs/rig-hooks-session-graph.md) — Ready, user-approved.
Specs: Draft. Plans: Drafting. No implementation approval or runtime completion is implied.

## Reviewed artifacts

- [Rig migration](rig-043-migration/spec.md) and [plan](rig-043-migration/plan.md).
- [External hooks](external-hooks/spec.md) and [plan](external-hooks/plan.md).
- [Session completion](desktop-session-completion/spec.md) and [plan](desktop-session-completion/plan.md).
- [Graph completeness](observatory-graph-completeness/spec.md) and [plan](observatory-graph-completeness/plan.md).
- Hook config/event/response JSON schemas, graph-exploration and session-details OpenAPI, artifact backlinks, brief map, spec index and workspace provenance.

## Independent reviews

Adversarial spec review: **Clean — ready to commit.** Second pass after corrections.
Secure-design review: **Clean — ready to commit.** Second pass after corrections.

Resolved findings: operator-managed hook entry-point trust without ward/project
execution or a trust UI; concrete local graph guards before reads; recursive
known-credential projection; bounded graph display fields/pages/admission;
nonbinding Draft RFC citation; index registration; canonical benchmark thresholds;
and corrected YAML label description.

## Authoring validation

- Three JSON Schemas pass Draft 2020-12 schema validation and more than forty positive/negative config/protocol fixtures, including unsupported events/actions/fields, bounds and rejected ward cwd.
- graph-exploration.yaml, session-details.yaml and goal-artifacts.yaml pass OpenAPI 3.1 validation with openapi-spec-validator 0.7.2 in an isolated temporary environment.
- Graph projection schema fixtures reject oversized names/containers/strings and excess nesting, and accept flagged omission.
- All four spec/plan pairs pass Draft metadata, local links, AC-to-task mapping, explicit task dependencies/verification modes and Tests-before-Approach checks.
- Brief coverage resolves all four mapped Drafts; workspace evaluation has only expected unapproved-spec findings plus the Rig dependency on external-hooks.
- Scoped whitespace checks pass. Contract backlinks resolve.
- Global spec metadata lint still fails on the pre-existing invalid Drafting status in exec-consolidation-waves/spec.md. No new contract/link warning remains; that unrelated artifact is preserved.

## Practical limits

These checks validate contracts and authoring, not implementation. No Rust runtime,
provider request, live research session or configured command is run by this work.
No acceptance checkbox is marked complete. The working provider/intent path and
original desktop shell plan/cohort remain unchanged.

The task-owned vault editor schema is synchronized only after confirming it still
matches the previously installed copy; hooks.json is preserved. Runtime hook
registration remains implementation work. Numerical graph budgets are explicit
Draft criteria, not measurements already achieved. The dirty implementation
checkout is two commits behind main; implementation needs a fresh isolated base.
