#!/usr/bin/env bash
#
# z-bot uninstaller. Symmetric with install.sh.
#
# Removes (current install + legacy `agentzero` artifacts):
#   - systemd unit (zbot.service or agentzero.service)
#   - binary at ~/.local/bin/zbotd (or zerod)
#   - UI dist at ~/.local/share/zbot/ (or agentzero/)
#
# Preserves:
#   - User data at ~/Documents/zbot/ (or ~/zbot/ on hosts without
#     ~/Documents/) — config, providers, agents, sessions.
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

# Match the daemon's runtime resolution.
if [[ -d "${HOME}/Documents" ]]; then
    VAULT_DIR="${HOME}/Documents/zbot"
else
    VAULT_DIR="${HOME}/zbot"
fi

header "Removing z-bot..."

# Stop + disable the current service AND any legacy unit. Both names
# get the same treatment so an old install left behind by a previous
# version still cleans up properly.
for svc in zbot agentzero; do
    if systemctl --user is-active "${svc}.service" >/dev/null 2>&1; then
        note "  → stopping ${svc}.service"
        systemctl --user stop "${svc}" || true
    fi
    if systemctl --user is-enabled "${svc}.service" >/dev/null 2>&1; then
        note "  → disabling ${svc}.service"
        systemctl --user disable "${svc}" || true
    fi
    UNIT_FILE="${HOME}/.config/systemd/user/${svc}.service"
    if [[ -f "${UNIT_FILE}" ]]; then
        note "  → removing ${UNIT_FILE}"
        rm -f "${UNIT_FILE}"
    fi
done

# Binary cleanup — current and legacy names.
for bin in zbotd zerod; do
    BIN_FILE="${HOME}/.local/bin/${bin}"
    if [[ -f "${BIN_FILE}" ]]; then
        note "  → removing ${BIN_FILE}"
        rm -f "${BIN_FILE}"
    fi
done

# UI dist cleanup — current and legacy directory names.
for d in zbot agentzero; do
    DIST_DIR="${HOME}/.local/share/${d}"
    if [[ -d "${DIST_DIR}" ]]; then
        note "  → removing ${DIST_DIR}"
        rm -rf "${DIST_DIR}"
    fi
done

systemctl --user daemon-reload

note ""
note "${GREEN}${BOLD}✓ z-bot uninstalled.${RESET}"
note ""
note "  To also delete user data: rm -rf ${VAULT_DIR}"
note "  To disable user-service linger: loginctl disable-linger ${USER}"
