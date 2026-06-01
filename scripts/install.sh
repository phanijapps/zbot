#!/usr/bin/env bash
#
# Public zbot installer entrypoint.
#
# This wrapper intentionally installs prebuilt GitHub Release artifacts. For a
# source build from a checkout, use ./scripts/install-from-source.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${SCRIPT_DIR}/install-release.sh" "$@"
