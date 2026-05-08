# Raspberry Pi Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Makefile + bootstrap shell scripts so a fresh Raspberry Pi user can run `./scripts/install.sh` to validate prereqs, build, and install AgentZero as an auto-starting `systemd --user` daemon with stdout/stderr suppressed.

**Architecture:** Three new files (`Makefile`, `scripts/install.sh`, `scripts/uninstall.sh`) plus one templated systemd unit (`scripts/agentzero.service.in`). The Makefile owns deterministic file operations; the shell scripts own UX (prereq detection, suggestions, friendly summaries). All paths are user-scoped XDG (`~/.local/{bin,share}`, `~/.config/systemd/user`); no `sudo` anywhere.

**Tech Stack:** GNU Make, Bash 4+ with `set -euo pipefail`, systemd `--user` units, `loginctl enable-linger`. Static analysis via `shellcheck`. No new Rust or TypeScript code.

**Spec:** `memory-bank/future-state/2026-05-02-rpi-packaging-design.md`

**Quality bar:** `shellcheck` passes on both shell scripts with no warnings. Manual integration smoke on a real Pi (or x86 Linux acting as one) for the 8 scenarios in the spec's test checklist.

**Branching:** Create a fresh branch off `origin/develop` named `feature/rpi-packaging` before Task 1.

```bash
git fetch origin develop
git checkout -b feature/rpi-packaging origin/develop
```

---

## File structure

### New files

```
agentzero/
├── Makefile                           ← NEW: GNU Make, ~50 lines
├── scripts/                           ← NEW directory
│   ├── install.sh                     ← NEW: prereq + bootstrap, ~150 lines
│   ├── uninstall.sh                   ← NEW: symmetric removal, ~40 lines
│   └── agentzero.service.in           ← NEW: systemd unit template, ~25 lines
```

### Modified files

```
README.md                              ← MODIFIED: add install instructions
```

No changes to Rust or TypeScript code.

---

## Tasks

Tasks land in dependency order: systemd unit template first (referenced by Makefile), then Makefile targets, then install/uninstall scripts, then docs. Each task ends with a working artifact and a commit.

---

### Task 1: Add the systemd unit template

**Files:**
- Create: `scripts/agentzero.service.in`

- [ ] **Step 1: Create the unit template**

Create `scripts/agentzero.service.in` with this exact content:

```ini
[Unit]
Description=AgentZero daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=@@BIN@@ \
    --log-no-stdout \
    --log-rotation daily \
    --log-max-files 4 \
    --static-dir @@DIST@@

# Belt-and-suspenders: silence anything not routed through tracing
StandardOutput=null
StandardError=null

# Auto-restart on crash, with a circuit breaker against pathological loops
Restart=on-failure
RestartSec=5
StartLimitIntervalSec=60
StartLimitBurst=3

[Install]
WantedBy=default.target
```

- [ ] **Step 2: Verify the template parses**

Render a test version and ask `systemd-analyze` to verify it's well-formed.

Run:
```bash
sed 's|@@BIN@@|/tmp/zerod|g; s|@@DIST@@|/tmp/dist|g' scripts/agentzero.service.in > /tmp/test.service
systemd-analyze verify --user /tmp/test.service
echo "exit: $?"
```

Expected: no errors, exit 0. (One warning is acceptable: `Failed to load unit dependency network-online.target` if the ambient systemd doesn't have it loaded — that's environmental, not a template issue.)

- [ ] **Step 3: Commit**

```bash
git add scripts/agentzero.service.in
git commit -m "feat(packaging): add systemd user unit template for AgentZero daemon"
```

---

### Task 2: Add Makefile with `build` target

**Files:**
- Create: `Makefile`

- [ ] **Step 1: Create the Makefile with `build` target**

Create `Makefile` at the repo root with this content:

```make
# AgentZero - native install for Raspberry Pi / Linux
#
# Build, install, and manage the AgentZero daemon as a systemd user service.
# All paths are user-scoped XDG; no sudo required.

PREFIX     ?= $(HOME)/.local
BIN_DIR    ?= $(PREFIX)/bin
DIST_DIR   ?= $(PREFIX)/share/agentzero/dist
UNIT_DIR   ?= $(HOME)/.config/systemd/user

# UI dist output is at the repo root per apps/ui/vite.config.ts (outDir: "../../dist")
UI_BUILD_DIR := dist

.PHONY: build install uninstall start stop restart status logs clean help

help:
	@echo "AgentZero Makefile targets:"
	@echo "  make build      - cargo build --release && npm run build"
	@echo "  make install    - install daemon + UI + systemd unit, enable + start"
	@echo "  make uninstall  - stop + disable + remove installed files (preserves user data)"
	@echo "  make start      - systemctl --user start agentzero"
	@echo "  make stop       - systemctl --user stop agentzero"
	@echo "  make restart    - systemctl --user restart agentzero"
	@echo "  make status     - systemctl --user status agentzero"
	@echo "  make logs       - tail -F ~/Documents/zbot/logs/*.log"
	@echo "  make clean      - cargo clean + rm -rf $(UI_BUILD_DIR)"

build:
	cargo build --release
	cd apps/ui && npm install && npm run build
```

- [ ] **Step 2: Verify `make help` works**

Run: `make help`
Expected: prints the help block listing all targets.

- [ ] **Step 3: Verify `make build` runs to completion**

Run: `make build`
Expected: `cargo build --release` and `npm install && npm run build` both finish without error. The binary appears at `target/release/zerod`. The UI dist appears at `dist/` (repo root).

If `make build` fails because `apps/ui/node_modules` was already populated by a prior install, that's fine — `npm install` is idempotent.

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "feat(packaging): add Makefile with build target"
```

---

### Task 3: Add Makefile `install` and `uninstall` targets

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add `install` and `uninstall` to the Makefile**

Append these targets to the bottom of the Makefile (after `build`):

```make
install: build
	install -d $(BIN_DIR) $(DIST_DIR) $(UNIT_DIR)
	install -m 755 target/release/zerod $(BIN_DIR)/zerod
	rm -rf $(DIST_DIR)/* && cp -r $(UI_BUILD_DIR)/* $(DIST_DIR)/
	@sed 's|@@BIN@@|$(BIN_DIR)/zerod|g; s|@@DIST@@|$(DIST_DIR)|g' \
	    scripts/agentzero.service.in > $(UNIT_DIR)/agentzero.service
	systemctl --user daemon-reload
	systemctl --user enable --now agentzero
	@echo ""
	@echo "Installed. Status: systemctl --user status agentzero"

uninstall:
	-systemctl --user stop agentzero
	-systemctl --user disable agentzero
	rm -f $(UNIT_DIR)/agentzero.service
	rm -f $(BIN_DIR)/zerod
	rm -rf $(PREFIX)/share/agentzero
	systemctl --user daemon-reload
	@echo ""
	@echo "Uninstalled. User data in ~/Documents/zbot is preserved."
```

The `-` prefix on `systemctl stop` and `disable` lines tells Make to ignore failure — important because uninstall must succeed even if the unit was never installed.

- [ ] **Step 2: Verify `install` is idempotent (dry-run check)**

Run: `make -n install`
Expected: prints the install commands without executing. No errors.

- [ ] **Step 3: Run `make install` end-to-end**

Run: `make install`
Expected:
- Files appear at `~/.local/bin/zerod`, `~/.local/share/agentzero/dist/`, `~/.config/systemd/user/agentzero.service`.
- `systemctl --user status agentzero` shows the unit as `active (running)`.

If you don't have systemd `--user` running locally (e.g., Docker container, WSL without systemd), this step may fail. Note that as DONE_WITH_CONCERNS and proceed — the integration smoke test (Task 11) will validate on a real Pi.

- [ ] **Step 4: Run `make install` again to verify upgrade idempotency**

Run: `make install`
Expected: same outcome — no errors, daemon still running. The `enable --now` call is a no-op when already enabled, and `daemon-reload` followed by the implicit unit refresh handles the unit-file update.

- [ ] **Step 5: Run `make uninstall`**

Run: `make uninstall`
Expected:
- `~/.local/bin/zerod` is gone.
- `~/.local/share/agentzero/` is gone.
- `~/.config/systemd/user/agentzero.service` is gone.
- `~/Documents/zbot/` is untouched (verify with `ls ~/Documents/zbot/config/`).

- [ ] **Step 6: Commit**

```bash
git add Makefile
git commit -m "feat(packaging): add install + uninstall Makefile targets"
```

---

### Task 4: Add Makefile lifecycle targets

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Append lifecycle targets**

Append to the bottom of the Makefile:

```make
start:
	systemctl --user start agentzero

stop:
	systemctl --user stop agentzero

restart:
	systemctl --user restart agentzero

status:
	systemctl --user status agentzero

logs:
	@tail -F $(HOME)/Documents/zbot/logs/*.log

clean:
	cargo clean
	rm -rf $(UI_BUILD_DIR)
```

- [ ] **Step 2: Verify each target dispatches correctly**

Run each as a dry-run:
```bash
make -n start
make -n stop
make -n restart
make -n status
make -n logs
make -n clean
```

Expected: each prints the corresponding shell command without executing.

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "feat(packaging): add lifecycle targets (start/stop/restart/status/logs/clean)"
```

---

### Task 5: Create install.sh skeleton with platform check and color helpers

**Files:**
- Create: `scripts/install.sh`

- [ ] **Step 1: Create the script skeleton**

Create `scripts/install.sh`:

```bash
#!/usr/bin/env bash
#
# AgentZero installer for Raspberry Pi / Linux.
#
# Validates prerequisites, builds the daemon and UI, installs as a
# systemd --user service, and enables linger so the daemon survives
# SSH logout and reboots.
#
# Usage:
#   ./scripts/install.sh
#
# This script is idempotent: re-run after `git pull` to upgrade.
# It never uses sudo. If a prereq is missing, it prints the apt or
# rustup command for you to run yourself.

set -euo pipefail

# ---------------------------------------------------------------------------
# Color output (with NO_COLOR support)
# ---------------------------------------------------------------------------

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]] && command -v tput >/dev/null 2>&1; then
    GREEN=$(tput setaf 2)
    RED=$(tput setaf 1)
    YELLOW=$(tput setaf 3)
    BOLD=$(tput bold)
    RESET=$(tput sgr0)
else
    GREEN=""; RED=""; YELLOW=""; BOLD=""; RESET=""
fi

ok()    { printf "  %s✓%s %s\n" "$GREEN"  "$RESET" "$1"; }
fail()  { printf "  %s✗%s %s\n" "$RED"    "$RESET" "$1"; }
note()  { printf "%s\n" "$1"; }
header(){ printf "\n%s%s%s\n\n" "$BOLD" "$1" "$RESET"; }

# ---------------------------------------------------------------------------
# Platform check (Linux-only)
# ---------------------------------------------------------------------------

require_linux() {
    if [[ "$(uname -s)" != "Linux" ]]; then
        fail "This installer targets Linux (Raspberry Pi OS, Debian, Ubuntu)."
        note ""
        note "  Detected: $(uname -s)"
        note "  macOS / Windows installers are out of scope for v1."
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    header "AgentZero installer"
    require_linux
    note "Platform check passed: Linux"
    note ""
    note "(prereq checks and bootstrap not yet implemented)"
}

main "$@"
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/install.sh`

- [ ] **Step 3: Run on the dev machine (Linux)**

Run: `./scripts/install.sh`
Expected: prints "AgentZero installer", then "Platform check passed: Linux", then the placeholder line. Exit 0.

- [ ] **Step 4: Verify shellcheck is clean**

Run: `shellcheck scripts/install.sh`
Expected: no output, exit 0. (If shellcheck isn't installed: `sudo apt install -y shellcheck`. If it warns about `command -v tput >/dev/null` quoting, that's already correct in the snippet above; otherwise note the warning in the report.)

- [ ] **Step 5: Commit**

```bash
git add scripts/install.sh
git commit -m "feat(packaging): scaffold install.sh with platform check and color helpers"
```

---

### Task 6: Add prereq check functions

**Files:**
- Modify: `scripts/install.sh`

- [ ] **Step 1: Add prereq check functions before `main()`**

Insert this block in `scripts/install.sh` after the `require_linux` function and before `main()`:

```bash
# ---------------------------------------------------------------------------
# Prereq checks
#
# Each check_* function:
#   - prints a single status line (✓ or ✗)
#   - returns 0 if the prereq is satisfied, 1 otherwise
#   - emits its fix suggestion to MISSING_FIXES (deferred to end-of-run)
# ---------------------------------------------------------------------------

declare -a MISSING_FIXES

check_rust() {
    if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
        ok "rustc $(rustc --version | awk '{print $2}'), cargo $(cargo --version | awk '{print $2}')"
        return 0
    fi
    fail "rustc + cargo not found"
    MISSING_FIXES+=("Rust toolchain (rustc + cargo):
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source \$HOME/.cargo/env")
    return 1
}

check_node() {
    if command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1; then
        ok "node $(node --version), npm $(npm --version)"
        return 0
    fi
    fail "node + npm not found"
    MISSING_FIXES+=("Node.js + npm (for UI build):
    sudo apt update && sudo apt install -y nodejs npm")
    return 1
}

check_gcc() {
    if command -v gcc >/dev/null 2>&1; then
        ok "gcc $(gcc --version | head -1 | awk '{print $NF}')"
        return 0
    fi
    fail "gcc not found (cargo needs a C linker)"
    MISSING_FIXES+=("GCC + build-essential:
    sudo apt install -y build-essential pkg-config")
    return 1
}

check_systemd_user() {
    if command -v systemctl >/dev/null 2>&1 && systemctl --user status >/dev/null 2>&1; then
        ok "systemctl --user is functional"
        return 0
    fi
    fail "systemctl --user is not functional"
    MISSING_FIXES+=("systemd user mode:
    On Pi OS / Debian this should work out of the box. If your distro
    has stripped it down, you may need to enable user lingering or
    install systemd-container. Distro-specific — please consult docs.")
    return 1
}

check_loginctl() {
    if command -v loginctl >/dev/null 2>&1; then
        ok "loginctl available"
        return 0
    fi
    fail "loginctl not found"
    MISSING_FIXES+=("loginctl (part of systemd, used to enable user-service linger):
    Should be present alongside systemctl. If missing, your distro is
    unusual — please check installation.")
    return 1
}

check_disk_space() {
    # cargo build cache for the workspace lands around 1.5 GB.
    # Require 2 GB free in the user's home as a safety margin.
    local available_kb required_kb=2097152
    available_kb=$(df -P "$HOME" | awk 'NR==2 {print $4}')
    if [[ "$available_kb" -ge "$required_kb" ]]; then
        ok "disk free: $((available_kb / 1024 / 1024)) GB"
        return 0
    fi
    fail "disk free: only $((available_kb / 1024 / 1024)) GB (need 2 GB)"
    MISSING_FIXES+=("Disk space:
    Free up at least $((required_kb / 1024 / 1024 - available_kb / 1024 / 1024)) GB
    (cargo's build cache will need ~2 GB).
    Try: cargo clean (if ~/.cargo is large)")
    return 1
}

run_all_checks() {
    local failures=0
    check_rust          || failures=$((failures+1))
    check_node          || failures=$((failures+1))
    check_gcc           || failures=$((failures+1))
    check_systemd_user  || failures=$((failures+1))
    check_loginctl      || failures=$((failures+1))
    check_disk_space    || failures=$((failures+1))
    return "$failures"
}
```

- [ ] **Step 2: Update `main()` to call the checks**

Replace the existing `main()` function with:

```bash
main() {
    header "Checking prerequisites for AgentZero..."
    require_linux
    ok "Linux ($(. /etc/os-release && echo "$PRETTY_NAME"))"

    local failures=0
    run_all_checks || failures=$?

    if [[ "$failures" -gt 0 ]]; then
        header "To install missing prerequisites:"
        for fix in "${MISSING_FIXES[@]}"; do
            note "  $fix"
            note ""
        done
        note "${YELLOW}Re-run ./scripts/install.sh once these are resolved.${RESET}"
        exit 1
    fi

    note ""
    note "(bootstrap not yet implemented)"
}
```

- [ ] **Step 3: Run on the dev machine**

Run: `./scripts/install.sh`
Expected: prints the prereq check table. If everything is installed (rust, node, gcc, systemd, loginctl, disk), all rows show ✓ and the script ends with "(bootstrap not yet implemented)". If something is missing, it shows ✗ and prints the corresponding fix block.

- [ ] **Step 4: Verify shellcheck is clean**

Run: `shellcheck scripts/install.sh`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add scripts/install.sh
git commit -m "feat(packaging): add prereq checks (rust/node/gcc/systemd/loginctl/disk)"
```

---

### Task 7: Add bootstrap orchestration to install.sh

**Files:**
- Modify: `scripts/install.sh`

- [ ] **Step 1: Add the bootstrap function**

Insert this block in `scripts/install.sh` between the prereq check functions and `main()`:

```bash
# ---------------------------------------------------------------------------
# Bootstrap (build, install, enable linger, summarize)
# ---------------------------------------------------------------------------

bootstrap() {
    local upgrade_mode=false
    if systemctl --user is-enabled agentzero.service >/dev/null 2>&1; then
        upgrade_mode=true
    fi

    if "$upgrade_mode"; then
        header "Upgrading AgentZero (this takes ~15 min on a Pi 4)..."
    else
        header "Building AgentZero (this takes ~15 min on a Pi 4)..."
    fi

    note "  → make build"
    make build
    note "  → make install (binary, UI dist, systemd unit)"
    make install >/dev/null

    note "  → loginctl enable-linger ${USER}"
    loginctl enable-linger "${USER}"

    if "$upgrade_mode"; then
        note "  → systemctl --user restart agentzero"
        systemctl --user restart agentzero
    fi

    note ""
    note "${GREEN}${BOLD}✓ AgentZero is running.${RESET}"
    note ""
    note "  Status:  systemctl --user status agentzero"
    note "  Logs:    tail -F ~/Documents/zbot/logs/*.log"
    note "  URL:     http://agentzero.local:18791  (or http://<your-ip>:18791)"
    note ""
    note "  To stop:    make stop"
    note "  To remove:  ./scripts/uninstall.sh"
}
```

- [ ] **Step 2: Replace the `(bootstrap not yet implemented)` line in `main()`**

Find this in `main()`:

```bash
    note ""
    note "(bootstrap not yet implemented)"
```

Replace with:

```bash
    bootstrap
```

- [ ] **Step 3: Run the full installer end-to-end**

Run: `./scripts/install.sh`
Expected:
- Prereq checks all green.
- Build runs (cargo + npm).
- `make install` populates installed paths.
- Linger gets enabled.
- Friendly summary appears at the end.

If you can't run this on a real systemd-user environment, note it as DONE_WITH_CONCERNS — the manual smoke (Task 11) will validate on a Pi.

- [ ] **Step 4: Run again to verify upgrade messaging**

Run: `./scripts/install.sh`
Expected: header now says "Upgrading AgentZero..." instead of "Building AgentZero...". Service restarts at the end.

- [ ] **Step 5: Verify shellcheck is clean**

Run: `shellcheck scripts/install.sh`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add scripts/install.sh
git commit -m "feat(packaging): wire bootstrap into install.sh (build, install, linger, summary)"
```

---

### Task 8: Create scripts/uninstall.sh

**Files:**
- Create: `scripts/uninstall.sh`

- [ ] **Step 1: Create the uninstall script**

Create `scripts/uninstall.sh`:

```bash
#!/usr/bin/env bash
#
# AgentZero uninstaller. Symmetric with install.sh.
#
# Removes:
#   - systemd unit
#   - binary at ~/.local/bin/zerod
#   - UI dist at ~/.local/share/agentzero/
#
# Preserves:
#   - User data at ~/Documents/zbot/ (config, providers, agents, sessions)
#   - User-service linger (other services may depend on it)

set -euo pipefail

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]] && command -v tput >/dev/null 2>&1; then
    GREEN=$(tput setaf 2)
    BOLD=$(tput bold)
    RESET=$(tput sgr0)
else
    GREEN=""; BOLD=""; RESET=""
fi

note()   { printf "%s\n" "$1"; }
header() { printf "\n%s%s%s\n\n" "$BOLD" "$1" "$RESET"; }

header "Removing AgentZero..."

if systemctl --user is-active agentzero.service >/dev/null 2>&1; then
    note "  → stopping agentzero.service"
    systemctl --user stop agentzero
fi

if systemctl --user is-enabled agentzero.service >/dev/null 2>&1; then
    note "  → disabling agentzero.service"
    systemctl --user disable agentzero
fi

UNIT_FILE="${HOME}/.config/systemd/user/agentzero.service"
if [[ -f "${UNIT_FILE}" ]]; then
    note "  → removing ${UNIT_FILE}"
    rm -f "${UNIT_FILE}"
fi

BIN_FILE="${HOME}/.local/bin/zerod"
if [[ -f "${BIN_FILE}" ]]; then
    note "  → removing ${BIN_FILE}"
    rm -f "${BIN_FILE}"
fi

DIST_DIR="${HOME}/.local/share/agentzero"
if [[ -d "${DIST_DIR}" ]]; then
    note "  → removing ${DIST_DIR}"
    rm -rf "${DIST_DIR}"
fi

systemctl --user daemon-reload

note ""
note "${GREEN}${BOLD}✓ AgentZero uninstalled.${RESET}"
note ""
note "  To also delete user data: rm -rf ~/Documents/zbot"
note "  To disable user-service linger: loginctl disable-linger ${USER}"
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/uninstall.sh`

- [ ] **Step 3: Verify shellcheck is clean**

Run: `shellcheck scripts/uninstall.sh`
Expected: no warnings.

- [ ] **Step 4: Run on a system with the daemon installed**

Pre-condition: AgentZero installed via `./scripts/install.sh` from Task 7.

Run: `./scripts/uninstall.sh`
Expected:
- Each removal step prints a `→ removing <path>` line.
- Final message confirms uninstall and lists optional follow-ups.
- `~/Documents/zbot/` is still present (verify with `ls ~/Documents/zbot/config/`).
- `systemctl --user status agentzero` returns "Unit agentzero.service could not be found."

- [ ] **Step 5: Run uninstall.sh again on a clean system**

With the daemon already removed, run: `./scripts/uninstall.sh`
Expected: no errors. The `is-active` / `is-enabled` checks return false; the file checks all return false; final summary still prints.

- [ ] **Step 6: Commit**

```bash
git add scripts/uninstall.sh
git commit -m "feat(packaging): add symmetric uninstall.sh that preserves user data"
```

---

### Task 9: Add README install instructions

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Inspect existing README structure**

Run: `grep -n "^## \|^### " README.md | head -20`

Expected: shows the existing section headings. The "Installation" / "Development" / "Production Build" / "First Run" / "LAN access" sections are typical landmarks. Identify the right insertion point — likely after "Production Build" and before "First Run", under a new "## Install on Raspberry Pi" heading.

- [ ] **Step 2: Add the install section to README.md**

Insert this block in `README.md` at the appropriate location (after the "Production Build" section, before "First Run"):

```markdown
## Install on Raspberry Pi (or any Linux box)

Run AgentZero as an auto-starting user-account daemon, no `sudo` required.

```bash
git clone <repo> agentzero
cd agentzero
./scripts/install.sh
```

The script:

1. Validates prerequisites (rustc, cargo, node, npm, gcc, systemd, disk space).
2. If anything is missing, prints the exact `apt` / `rustup` command for you to run, then exits.
3. Once everything's green, builds the daemon and UI, installs into `~/.local/bin` and `~/.local/share/agentzero/`, and enables the systemd `--user` service with linger so it survives SSH logout and reboots.

To upgrade after pulling new code:

```bash
git pull
./scripts/install.sh
```

The same script handles fresh installs and upgrades — your `~/Documents/zbot/` data directory is never touched.

Common operations:

- `make status` — service status
- `make logs` — tail the rolling log
- `make restart` — restart the daemon
- `make stop` / `make start` — stop or start
- `./scripts/uninstall.sh` — remove the daemon (preserves user data)
```

- [ ] **Step 3: Verify the README still renders cleanly**

Run: `head -200 README.md | tail -80`
Expected: the new section sits naturally between adjacent sections; markdown headers nest correctly.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs(readme): add Raspberry Pi install instructions"
```

---

### Task 10: Pre-merge cross-cut verification

**Files:**
- None modified — this is a verification pass.

- [ ] **Step 1: shellcheck pass on both scripts**

Run:
```bash
shellcheck scripts/install.sh scripts/uninstall.sh
echo "exit: $?"
```
Expected: no output, exit 0.

- [ ] **Step 2: Makefile dry-run for every target**

Run:
```bash
for t in build install uninstall start stop restart status logs clean help; do
    echo "=== $t ==="
    make -n "$t" 2>&1 | head -10
done
```
Expected: each target prints reasonable shell commands; no Make errors.

- [ ] **Step 3: Verify the systemd unit template still renders**

Run:
```bash
sed 's|@@BIN@@|/tmp/zerod|g; s|@@DIST@@|/tmp/dist|g' scripts/agentzero.service.in > /tmp/test.service
systemd-analyze verify --user /tmp/test.service
echo "exit: $?"
```
Expected: clean. (Acceptable warning: `Failed to load unit dependency network-online.target` if your test environment doesn't have it.)

- [ ] **Step 4: Confirm no Rust or TypeScript code changed**

Run: `git diff --name-only origin/develop...HEAD | grep -v -E '^(Makefile|scripts/|README\.md|memory-bank/)$'`
Expected: empty output. (Only paths matching Makefile / scripts/ / README.md / memory-bank/ should appear in the diff.)

- [ ] **Step 5: Confirm the Rust workspace still builds (sanity)**

Run: `cargo check --workspace`
Expected: clean. (No reason it shouldn't — we didn't touch Rust — but a sanity check doesn't hurt.)

- [ ] **Step 6: Verify file permissions**

Run:
```bash
ls -l scripts/install.sh scripts/uninstall.sh
```
Expected: both files have execute permission for the user (`-rwxr-xr-x` or similar).

If they don't:
```bash
chmod +x scripts/install.sh scripts/uninstall.sh
git add scripts/install.sh scripts/uninstall.sh
git commit -m "chore(packaging): mark install/uninstall scripts executable"
```

---

### Task 11: Manual smoke test on real hardware

**Files:**
- None modified — this is a manual integration test.

This task is a checklist, not code. Each item is verified by a human on real hardware (or a Linux VM acting as one). Mark each as ✓ or ✗ in the PR description before requesting review.

- [ ] **Fresh install on a clean Pi OS image:**
    - `./scripts/install.sh` completes without prompts.
    - `systemctl --user status agentzero` shows `active (running)`.
    - From a phone on the same Wi-Fi: `http://agentzero.local:18791` loads the dashboard.

- [ ] **Reboot test:**
    - `sudo reboot`.
    - Wait 60 seconds.
    - Without logging in / SSHing back: phone can still reach `http://agentzero.local:18791` (linger working).

- [ ] **Upgrade test:**
    - `git pull` (or simulate by changing a file and committing).
    - `./scripts/install.sh`.
    - Header reads "Upgrading…".
    - Daemon restarts; service still running after.
    - `~/Documents/zbot/` contents unchanged.

- [ ] **Missing-prereq path:**
    - `sudo apt remove -y npm` (temporarily uninstall npm).
    - Run `./scripts/install.sh`.
    - Output lists npm as missing and prints the exact apt command.
    - Script exits non-zero without partial install.
    - `sudo apt install -y npm` to restore.

- [ ] **Uninstall test:**
    - `./scripts/uninstall.sh`.
    - Unit file, binary, dist directory all gone.
    - `~/Documents/zbot/` still present.
    - `systemctl --user status agentzero` reports unit-not-found.

- [ ] **Stdout-suppressed verification:**
    - With daemon running: `journalctl --user -u agentzero` returns nothing.
    - `tail -F ~/Documents/zbot/logs/*.log` shows tracing output.

- [ ] **Crash-restart test:**
    - `pgrep -u $USER -x zerod` to find the PID.
    - `kill -SEGV <pid>`.
    - Within 5 seconds, `systemctl --user status agentzero` shows the service back to active.

- [ ] **Circuit-breaker test:**
    - Temporarily break a config (e.g., set `network.advanced.httpPort` to a privileged port like 80 in `~/Documents/zbot/config/settings.json`).
    - `systemctl --user restart agentzero`.
    - Daemon fails to bind. Within 60 seconds the service hits the 3-attempt limit and stops trying.
    - `systemctl --user status agentzero` shows `failed`.
    - Restore the config; `make restart` recovers.

---

### Task 12: Push and open PR

- [ ] **Step 1: Final workspace check**

Run:
```bash
cargo fmt --all --check
shellcheck scripts/install.sh scripts/uninstall.sh
make -n install
```

Expected: all green.

- [ ] **Step 2: Push the branch**

```bash
git push -u origin feature/rpi-packaging
```

- [ ] **Step 3: Open the PR**

```bash
gh pr create --base develop --head feature/rpi-packaging \
  --title "feat: Raspberry Pi packaging — Makefile + bootstrap install script" \
  --body "$(cat <<'EOF'
## Summary

Implements [`memory-bank/future-state/2026-05-02-rpi-packaging-design.md`](../blob/develop/memory-bank/future-state/2026-05-02-rpi-packaging-design.md).

A user with a fresh Raspberry Pi running Pi OS 64-bit can now:

\`\`\`bash
git clone <repo> agentzero
cd agentzero
./scripts/install.sh
\`\`\`

The script validates prerequisites, suggests fixes for any missing ones, builds the daemon and UI, installs as a \`systemd --user\` service with linger, and starts the service. Re-running after \`git pull\` performs an in-place upgrade.

## Files

- \`Makefile\` — \`build\`, \`install\`, \`uninstall\`, \`start\`/\`stop\`/\`restart\`/\`status\`/\`logs\`/\`clean\`.
- \`scripts/install.sh\` — prereq checks (rust, node, gcc, systemd, loginctl, disk) with apt/rustup suggestions; bootstraps via Make; enables linger.
- \`scripts/uninstall.sh\` — symmetric, preserves \`~/Documents/zbot/\`.
- \`scripts/agentzero.service.in\` — systemd unit template (\`@@BIN@@\` and \`@@DIST@@\` placeholders).
- \`README.md\` — install instructions section.

No Rust or TypeScript code changed.

## Quality gates

- \`shellcheck scripts/*.sh\` — clean
- \`make -n <target>\` — every target dry-runs cleanly
- \`systemd-analyze verify --user <rendered-unit>\` — clean
- \`cargo check --workspace\` — clean (sanity)

## Manual smoke test

See \`memory-bank/plans/2026-05-02-rpi-packaging-implementation.md\` Task 11. Eight scenarios:
- [ ] Fresh install on clean Pi OS
- [ ] Reboot test (linger)
- [ ] Upgrade test (\`git pull && ./scripts/install.sh\`)
- [ ] Missing-prereq path (apt remove npm; verify suggestion)
- [ ] Uninstall preserves user data
- [ ] Stdout suppression (journalctl empty; log file populated)
- [ ] Crash-restart (kill -SEGV)
- [ ] Circuit-breaker (3 crashes in 60s → systemd gives up)

## Out of scope

Cross-compile from a dev laptop, .deb packaging, macOS/Windows installers, watchdog with sd_notify, resource quotas, size-based log rotation. All documented in the spec under "Out of scope for v1".

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

I reviewed the plan against the spec:

**Spec coverage:** Every spec section maps to at least one task. Architecture (Tasks 5-9), install layout (Task 3), Makefile targets (Tasks 2-4), systemd unit (Task 1), install.sh prereq checks (Task 6), bootstrap orchestration (Task 7), uninstall.sh (Task 8), file structure (covered across tasks), testing strategy (Tasks 10-11), out of scope (called out in the PR body in Task 12).

**Placeholder scan:** No "TBD"/"TODO"/"add appropriate"/"similar to Task N" patterns. Every step that creates code shows the exact code. The only deferred items are the manual smoke test scenarios in Task 11, which are documented as a checklist requiring human verification on real hardware — that's not a placeholder, that's the only practical verification for system integration.

**Type / name consistency:** `BIN_DIR`, `DIST_DIR`, `UNIT_DIR`, `PREFIX` used consistently across the Makefile. `agentzero.service` filename used consistently. `@@BIN@@` / `@@DIST@@` placeholders match between the systemd template and the Makefile sed call. Function names in `install.sh` (`check_rust`, `check_node`, etc.) follow a single naming convention. No drift detected.

No fixes needed.
