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

# shellcheck disable=SC2034  # color vars are used inside ok/fail/note/header below
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
