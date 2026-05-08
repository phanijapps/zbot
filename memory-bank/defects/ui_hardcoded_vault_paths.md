# [FIXED 2026-05-03] UI hardcodes `~/Documents/zbot/` even when daemon's actual vault is elsewhere

**Severity:** Medium (cosmetic, no data loss)
**Discovered:** 2026-05-03 on Raspberry Pi (Pi OS Lite, no `~/Documents/`)
**Status:** Open, awaiting fix

## Symptom

On a Raspberry Pi (or any host without `~/Documents/`), the daemon falls back to `~/zbot/` (or `./zbot` in degenerate cases) for its vault directory. Several UI surfaces still show the literal string `~/Documents/zbot/...` in their copy, misleading the user about where their data actually lives.

## Root cause

**Daemon path resolution** at `gateway/src/server.rs:53-56`:

```rust
let data_dir = dirs::document_dir()
    .or_else(dirs::home_dir)
    .unwrap_or_else(|| PathBuf::from("."))
    .join("zbot");
```

So `<vault>` resolves to one of:

- `~/Documents/zbot/` — typical desktop with `XDG_DOCUMENTS_DIR` set or `~/Documents/` present.
- `~/zbot/` — Pi OS Lite headless, no Documents dir.
- `./zbot/` — fallback if neither home nor document dir exists.

The UI hardcodes `~/Documents/zbot/...` strings in display copy and never asks the daemon what the actual path is.

## Affected UI strings

Verified by `grep -rn "Documents/zbot" apps/ui/src --include="*.tsx" --include="*.ts"` — exactly four locations, no others:

| File | Line | Current text |
|---|---|---|
| `apps/ui/src/features/settings/customization/CustomizationTab.tsx` | 68 | `Changes save back to <code>~/Documents/zbot/config/</code>` |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | 394 | `Data <code>~/Documents/zbot/</code>` |
| `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` | 1079 | `Drop a plugin folder into ~/Documents/zbot/plugins/ and restart` |
| `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx` | 1099 | `To install a plugin, drop its folder into ~/Documents/zbot/plugins/ and restart zerod` |

## Backend already has the right paths — just doesn't expose them

`gateway-services/src/paths.rs` (`VaultPaths` struct, lines 81-148) exposes everything the UI needs:

- `vault_dir()`, `data_dir()`, `config_dir()`, `logs_dir()`
- `agents_dir()`, `skills_dir()`, `wards_dir()`, `plugins_dir()`
- `instructions()`, `distillation_prompt()`, etc. (specific files)

These flow through `state.paths` everywhere in the gateway. They're just never sent to the UI.

No `/api/paths` (or similar) endpoint exists today. `/api/health` returns `{ status, version }` only. `/api/status` adds `agent_count`, still no paths.

## Suggested fix

### Backend (~30 lines, one new endpoint)

Add `GET /api/paths`. No parameters, no caching needed (paths are fixed at process startup).

```rust
// gateway/src/http/paths.rs (new file)
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsResponse {
    pub vault_dir: String,
    pub config_dir: String,
    pub logs_dir: String,
    pub plugins_dir: String,
    pub agents_dir: String,
    /// Display variants — `$HOME` replaced by `~/`. Pure UI sugar.
    pub vault_dir_display: String,
    pub config_dir_display: String,
    pub logs_dir_display: String,
    pub plugins_dir_display: String,
}

pub async fn get_paths(State(state): State<AppState>) -> Json<PathsResponse> {
    fn pretty(p: &std::path::Path) -> String {
        let abs = p.to_string_lossy().to_string();
        if let Some(home) = dirs::home_dir() {
            let h = home.to_string_lossy().to_string();
            if abs.starts_with(&h) {
                return format!("~{}", &abs[h.len()..]);
            }
        }
        abs
    }

    let p = &state.paths;
    Json(PathsResponse {
        vault_dir: p.vault_dir().to_string_lossy().into_owned(),
        config_dir: p.config_dir().to_string_lossy().into_owned(),
        logs_dir: p.logs_dir().to_string_lossy().into_owned(),
        plugins_dir: p.plugins_dir().to_string_lossy().into_owned(),
        agents_dir: p.agents_dir().to_string_lossy().into_owned(),
        vault_dir_display: pretty(&p.vault_dir()),
        config_dir_display: pretty(&p.config_dir()),
        logs_dir_display: pretty(&p.logs_dir()),
        plugins_dir_display: pretty(&p.plugins_dir()),
    })
}
```

Register in `gateway/src/http/mod.rs` next to `/api/health`:

```rust
mod paths;
// …
.route("/api/paths", get(paths::get_paths))
```

The `pretty()` helper applies `path.replace($HOME, "~/")` so UI shows `~/zbot/plugins/` rather than `/home/pi/zbot/plugins/`. If `$HOME` doesn't appear in the path, display is the absolute form (rare edge case — degenerate `./zbot` fallback).

### Frontend (~50 lines)

1. **New hook** at `apps/ui/src/hooks/usePaths.ts`:

```ts
import { useEffect, useState } from "react";

export type Paths = {
  vaultDir: string;
  configDir: string;
  logsDir: string;
  pluginsDir: string;
  agentsDir: string;
  vaultDirDisplay: string;
  configDirDisplay: string;
  logsDirDisplay: string;
  pluginsDirDisplay: string;
};

let cached: Paths | null = null;
let pending: Promise<Paths | null> | null = null;

export function usePaths(): Paths | null {
  const [paths, setPaths] = useState<Paths | null>(cached);
  useEffect(() => {
    if (cached) return;
    if (!pending) {
      pending = fetch("/api/paths")
        .then((r) => (r.ok ? (r.json() as Promise<Paths>) : null))
        .then((p) => {
          cached = p;
          return p;
        })
        .catch(() => null);
    }
    void pending.then((p) => setPaths(p));
  }, []);
  return paths;
}
```

Module-level `cached` + `pending` give us a single fetch per page load with simple memoization (no React Query / Context needed for one tiny call).

2. **Replace the 4 hardcoded strings** with `usePaths()` consumers:

```tsx
// CustomizationTab.tsx:68
const paths = usePaths();
<code>{paths?.configDirDisplay ?? "~/Documents/zbot/config/"}</code>
```

```tsx
// WebSettingsPanel.tsx:394
const paths = usePaths();
<code>{paths?.vaultDirDisplay ?? "~/Documents/zbot/"}</code>
```

```tsx
// WebIntegrationsPanel.tsx:1079, 1099
const paths = usePaths();
const pluginsHint = paths?.pluginsDirDisplay ?? "~/Documents/zbot/plugins/";
// Drop a plugin folder into {pluginsHint} and restart
// To install a plugin, drop its folder into {pluginsHint} and restart zerod
```

The fallback string preserves prior behavior on transient fetch failure or while loading. UI never breaks even if `/api/paths` is unreachable.

## Tests

### Backend

Unit test `get_paths` with a `VaultPaths::new(/tmp/test-vault)`:

| Test | What it pins |
|---|---|
| `paths_response_includes_all_dirs` | All 5 absolute dirs present and end in expected suffixes |
| `display_variant_replaces_home_prefix` | `$HOME = /tmp/test-home`, `vault_dir = /tmp/test-home/zbot` → `vault_dir_display = ~/zbot` |
| `display_variant_passes_through_when_home_not_prefix` | Path that doesn't start with `$HOME` → display equals absolute |

### Frontend

| Test | What it pins |
|---|---|
| `usePaths returns paths after fetch` | Mocked `/api/paths` → hook returns response |
| `usePaths returns null while pending` | First render before fetch resolves → hook returns `null` |
| `Each affected component renders dynamic path when usePaths returns data` | 4 components updated, each shows the dynamic display string |
| `Each affected component renders fallback when usePaths returns null` | Same components show the original `~/Documents/zbot/...` fallback during loading |

## Acceptance criteria

- On a Pi where `<vault> = /home/pi/zbot`, the four UI surfaces show `~/zbot/...` (or absolute path if `$HOME` doesn't match), not `~/Documents/zbot/...`.
- On a desktop where `<vault> = ~/Documents/zbot`, the same surfaces continue to show `~/Documents/zbot/...` — no regression.
- `cargo test -p gateway` clean, `cargo clippy -p gateway --all-targets -- -D warnings` clean.
- `cd apps/ui && npm test`, `npm run lint`, `npm run build` all clean.

## Out of scope

- Writing paths from the UI back to the daemon — never needed; daemon owns paths.
- Internationalizing or further prettifying paths beyond `$HOME → ~/`.
- Reactivity to data-dir changes mid-session — vault dir is fixed at process start; if the user changes `--data-dir` they restart anyway.
- Path validation / sanitization on the response — daemon controls these strings entirely.
- A toast or error banner on `/api/paths` fetch failure — fallback string is sufficient.

## References

- `gateway/src/server.rs:53-56` — `data_dir` resolution.
- `gateway/gateway-services/src/paths.rs` — `VaultPaths` (all the right paths already).
- `gateway/src/http/mod.rs` — route registration site (next to `/api/health`).
- `apps/ui/src/features/settings/customization/CustomizationTab.tsx:68` — fix site #1.
- `apps/ui/src/features/settings/WebSettingsPanel.tsx:394` — fix site #2.
- `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx:1079, 1099` — fix sites #3 and #4.
