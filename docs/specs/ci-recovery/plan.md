# Plan: CI recovery

- **Spec:** [spec.md](spec.md)
- **Status:** Done

## Tasks

### T1: Restore the two diagnosed CI checks

**Depends on:** none

**Verification mode:** Goal-based check; existing failing fixture is the reproduction.

**Spec mapping:** AC1–3

**Tests:** Run the complete ResearchPage and frontend suites, cargo fmt --all --check, cargo check --workspace, cargo clippy --all-targets -- -D warnings, and affected crate tests. Record any unrelated baseline failure. Verify Cargo.lock updates only async-trait to 0.1.92, adds its required syn 3.0.6 parser, and leaves every other package record identical. No new runtime test framework or compiler workaround.

**Approach:** Set sessionId in the existing running Stop fixture and add the missing no-session negative case. Run cargo update -p async-trait --precise 0.1.92. Review the minimum diff and retain the existing Stop guard and all CI checks.

## Disposition

Assumptions: only the fixture and lockfile need executable changes; existing test/build commands are the proof; no provider, vault or runtime execution policy changes. The user approved this concrete scope. Declined global lint suppression and compiler downgrade because the published dependency patch removes the generated attribute. No domain assumption or contract-anchor change is needed. Project-knowledge not requested. Final findings and evidence belong in verification.md.

The resolver adds syn 3.0.6 as required by the approved upstream patch. This
routes verification/review to full mode for the build-trust boundary. Existing
runtime and macro public call sites remain unchanged; the task stays below
2,000 reviewable lines. The construction contract includes only this required
transitive addition, with all other package versions retained.
