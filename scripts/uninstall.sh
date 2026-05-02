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
