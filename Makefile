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
