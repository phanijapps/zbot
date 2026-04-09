# Ward Scaffolding — Component Overview

## What It Is

Ward scaffolding is a **skill-driven directory scaffolding system** that structures wards as reusable apps. Skills declare their ward structure in `ward_setup` frontmatter; the runtime creates directories and AGENTS.md on ward creation.

## When It Runs

- **Post-execution** (after root agent completes): `scaffold_ward_from_skills` reads recommended skills' `ward_setup` and creates directories
- **Post-execution** (after scaffolding): `auto_update_agents_md` re-indexes core modules using language configs
- **Scaffolding is idempotent** — safe to run every execution, skips existing dirs/files

## What It Does

1. **Reads skill `ward_setup`** — from recommended skills' SKILL.md frontmatter (`directories`, `language_skills`, `spec_guidance`, `agents_md`)
2. **Creates directories** — `core/`, `output/`, `specs/`, `specs/archive/`, `memory-bank/` (or whatever the skill declares)
3. **Generates AGENTS.md** — from skill's `agents_md` config (purpose, conventions, directory layout)
4. **Indexes core modules** — scans `core/` using language configs from `config/wards/*.yaml`, updates AGENTS.md with function/class signatures
5. **Injects ward rules** — via `format_intent_injection()` into agent prompt (spec-first workflow, reuse existing code, archive completed specs)

## Key Design Decisions

- **Skills drive structure** — not hardcoded. Different skills scaffold different directories.
- **Language configs externalized** — `config/wards/*.yaml` for signature extraction patterns. Users add languages without touching Rust.
- **Spec-first workflow** — injected as prompt guidance; graph tasks must start with a spec-writing node.
- **AGENTS.md is the living README** — auto-updated with core module API index after each session.
- **Specs are ephemeral** — active in `specs/`, archived to `specs/archive/` after implementation.
- **Fallback to Python** — when no language config matches a file extension, hardcoded Python patterns are used.

## Related Files

| File | Purpose |
|------|---------|
| `gateway/gateway-execution/src/middleware/ward_scaffold.rs` | Scaffolding: `scaffold_ward()` creates dirs + AGENTS.md |
| `gateway/gateway-services/src/lang_config.rs` | Language config loader + signature extraction |
| `gateway/gateway-services/src/skills.rs` | `WardSetup`, `WardAgentsMdConfig` types; `get_ward_setup()` |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Ward rules + spec guidance injection in `format_intent_injection()` |
| `gateway/gateway-execution/src/runner.rs` | `scaffold_ward_from_skills()`, `auto_update_agents_md_with_lang_configs()` |
| `~/Documents/zbot/config/wards/*.yaml` | Language pattern configs (user-editable) |
