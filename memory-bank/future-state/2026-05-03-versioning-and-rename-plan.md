# Date-based Versioning + Rename to `zbot` — Plan

**Status:** Plan (awaiting review)
**Date:** 2026-05-03
**Owner:** phanijapps

## Three asks bundled

1. **Date-based versioning** — replace the current `0.1.0` semver with a CalVer scheme.
2. **Install scripts** currently say `zerod` / `zero` / `agentzero`; they should say `zbot`.
3. **Plan a low-effort app rename** so the repo stops mixing `agentzero`, `zero`, and `zbot`.

These share a release flow, so handling them together is cheaper than three separate passes.

## Current state — audit

| Surface | Today | Layer |
|---|---|---|
| Workspace version (`Cargo.toml`) | `0.1.0` | Versioning |
| `apps/ui/package.json` version | independent | Versioning |
| Daemon binary | `zerod` (built from `gateway` crate) | Layer 1 |
| CLI binary | `zero` (`apps/cli/Cargo.toml` `name = "zero"`) | Layer 1 |
| Systemd unit | `agentzero.service` (template `scripts/agentzero.service.in`) | Layer 1 |
| mDNS hostname | `agentzero.local` | Layer 1 |
| mDNS instance default | `agentzero` | Layer 1 |
| Make targets | `systemctl --user start agentzero` etc. | Layer 1 |
| Vault dir | `~/Documents/zbot/` (already correct ✓) | n/a |
| GitHub repo | `phanijapps/zbot` (already correct ✓) | n/a |
| Brand string in user copy | mixed: `AgentZero`, `z-bot`, `zbot` | Layer 2 |
| README / docs | mixed | Layer 2 |
| Crate names | `agent-runtime`, `agent-tools`, `gateway` | Layer 3 |
| Type names referencing `AgentZero` | scattered (>100) | Layer 3 |

`rg` totals: 54 files mention `agentzero`, 84 mention `zbot`/`z-bot`, 143 mention bare `zero`.

## Part 1 — Versioning: `YYYY.MM.PATCH` (CalVer)

### Choice: `YYYY.MM.PATCH`

Examples: `2026.05.0`, `2026.05.1`, `2026.06.0`.

### Why this and not the alternatives

- **`YYYY.MM.PATCH`** ✓ chosen — three numeric segments, semver-compatible parsers happy, sortable as strings, "when was this cut" is obvious.
- `YY.MM.PATCH` (`26.5.0`): one less digit but ambiguous in 100 years and slightly weirder for tooling that expects 4-digit year.
- `YYYY.MM.DD`: every release becomes a unique date — fine until you ship two patches the same day, then needs a 4th segment.
- `YYYY.0M.MICRO` (zero-padded month): solves the "is `2026.5` < `2026.10` lexically?" problem; semver parsing of the segment is unaffected because they all parse as `u32`. **Worth considering** if any downstream tool (lockfiles, git tags) sorts versions as strings.

**Recommendation:** `YYYY.MM.PATCH`, no zero-padding. Bump rules:
- Start of month → `YYYY.MM.0`
- Patch within month → `YYYY.MM.1`, `YYYY.MM.2`, …
- Hotfix on an old minor → `YYYY.MM.PATCH+1` (no separate hotfix track)

### What carries the version

Single source of truth: `Cargo.toml [workspace.package] version`. Everything else reads from there:

- All workspace crates inherit via `version.workspace = true`
- `apps/ui/package.json` — `npm version 2026.05.0` once per cut
- Daemon `--version` and CLI `--version` — emitted by `env!("CARGO_PKG_VERSION")` (already wired)
- Systemd unit `Description=` — pull from a build-time env or skip
- Git tag — `vYYYY.MM.PATCH`

### Tooling: `scripts/release.sh` (later, not in the rename PR)

```bash
#!/usr/bin/env bash
# Usage: scripts/release.sh [patch|minor]
# Computes next CalVer from current date + last tag.
```

- Reads last tag, parses date components.
- If month changed since last tag → bump to `YYYY.MM.0`.
- Else → bump PATCH.
- `cargo set-version` (cargo-edit) updates `[workspace.package].version`.
- `npm version` updates `apps/ui/package.json`.
- Creates annotated tag, no push (review before push).

Out of scope for the first version-bump PR. Just hand-edit the two version strings the first time; automate later.

## Part 2 — Install script rename (immediate, narrow scope)

The smallest-blast-radius change. Touches:

- `Makefile` — replace `zerod` binary name with `zbotd`, `agentzero.service` with `zbot.service`, `agentzero` Make targets with `zbot`. ~10 lines.
- `scripts/install.sh` — `zerod` → `zbotd`, `agentzero.service` → `zbot.service`, `agentzero.local` → `zbot.local`, `systemctl --user … agentzero` → `systemctl --user … zbot`. ~15 lines.
- `scripts/uninstall.sh` — same. ~5 lines.
- `scripts/agentzero.service.in` → rename file to `scripts/zbot.service.in`; update `Makefile` to reference the new name. 1 line in Make + git mv.
- `apps/cli/Cargo.toml` — `[[bin]] name = "zero"` → `name = "zbot"`. 1 line.
- `gateway/Cargo.toml` — `[[bin]] name = "zerod"` → `name = "zbotd"`. 1 line.

**Migration for existing installs.** Add a one-time migration block in `scripts/install.sh`:

```bash
# Migrate old service name if present
if systemctl --user is-enabled agentzero.service >/dev/null 2>&1; then
    note "Migrating from agentzero.service → zbot.service"
    systemctl --user disable --now agentzero.service || true
    rm -f "${UNIT_DIR}/agentzero.service"
fi
```

Then proceed with the regular install flow which writes `zbot.service`.

## Part 3 — App rename plan (`agentzero`/`zero` → `zbot`)

### Three layers, only ship layers 1 + 2

**Layer 1 — Operationally visible (must rename):**
- Binary names (`zerod`, `zero`)
- Systemd unit and template
- mDNS hostname + instance default name
- Make targets and install/uninstall scripts
- Cargo `[[bin]]` name fields

**Layer 2 — User-visible copy (should rename for consistency):**
- README, CONTRIBUTING, install instructions
- HTTP error messages and version banner
- Wizard / settings page strings that show the brand
- Comments at the top of `Cargo.toml`, `package.json`, key modules
- The handful of "AgentZero" strings in user-facing UI copy → standardize on `z-bot` (or `zbot` — pick one and stick to it; my recommendation: `z-bot` for display, `zbot` for identifiers)

**Layer 3 — Source-internal naming (skip — high effort, no user impact):**
- Crate names: `agent-runtime`, `agent-tools`, `gateway-*` — keep. They describe technical concepts, not the product.
- Internal type/module names containing `AgentZero` — keep, mostly invisible.
- Cargo crate IDs in `Cargo.toml`: `name = "gateway"` etc. Keep.

### Standardization on identifiers

Pick once and stop drifting:
- **Identifier form** (paths, binaries, services, hostnames, env vars): `zbot` — lowercase, no separators.
- **Display form** (user copy, README headings, wizard text): `z-bot` — preserves the playful brand.
- **Never use:** `AgentZero`, `agentzero`, `zerod`, `zero` (as the product name; `zero` as a CLI command is also being renamed).

### Sequencing — three PRs, in order

**PR A — Layer 1 only** (~80 lines, mechanical):
- Cargo `[[bin]]` rename (zerod → zbotd, zero → zbot)
- `scripts/install.sh` + `scripts/uninstall.sh` updates
- `Makefile` updates  
- Service template rename (`scripts/agentzero.service.in` → `scripts/zbot.service.in`)
- mDNS hostname + instance defaults
- Migration block in install.sh for existing installs
- **Test:** fresh install on a Pi produces `zbotd`, `zbot.service`, `zbot.local`. Existing install upgrades cleanly.

**PR B — Versioning bump** (~5 lines):
- Cargo workspace version → `2026.05.0`
- `apps/ui/package.json` version → `2026.05.0`
- Tag `v2026.05.0` after merge
- **Out of scope:** `release.sh` automation. Hand-edit this once; automate next month.

**PR C — Layer 2 (user copy + README)** (~30 files):
- README, install docs, CONTRIBUTING
- Wizard / settings page strings
- HTTP banner / version display
- Standardize on `z-bot` (display) / `zbot` (identifier)
- **Test:** `rg -i "agentzero"` returns only intentional historical references (e.g., this plan doc).

### What does NOT get renamed

- `~/Documents/zbot/` vault dir (already correct)
- `phanijapps/zbot` GitHub repo (already correct)
- Crate names like `agent-runtime`, `agent-tools` (internal abstraction names, not product brand)
- The `agent_id` field on cron jobs / sessions / etc. (a generic concept name, not product brand)
- Stored data formats — no schema change needed; everything keys off `agent_id` strings, not the product name

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| Existing installs break on upgrade | One-time migration in install.sh disables old service before writing new one; idempotent |
| Users have `zerod` in their muscle memory / aliases | Document the rename in CHANGELOG; symlink `zerod → zbotd` for one release if needed (drop after) |
| External tooling pins on the binary name (CI, Docker, etc.) | None known; check before merge |
| `cargo install` users — change in binary name | New version note: `cargo uninstall zero && cargo install --path apps/cli` |
| Search for old name in logs | `grep "zerod\|agentzero"` post-rename helps catch stragglers |

## Suggested execution order

1. **PR #X** — File this plan doc (this PR) for review.
2. **PR A** — Layer 1 rename. Self-contained, mechanical.
3. **PR B** — Bump version to `2026.05.0`. Tiny.
4. **PR C** — Layer 2 user copy. Skim and merge.
5. **Future** — `scripts/release.sh` automation.
6. **Future** — Layer 3 if it ever feels worth it (probably never).

## Out of scope for now

- Rename of crate IDs (`gateway`, `agent-runtime`, etc.) — high effort, low value.
- A new release-cadence policy (monthly vs. ad-hoc) — pick when first cutting.
- Distribution beyond systemd (Flatpak, Homebrew, etc.) — separate plan.
- Renaming the `zbot` GitHub repo — already correct.
- Brand-mark / logo work — not engineering.
