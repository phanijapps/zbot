#!/usr/bin/env bash
#
# Cut a CalVer zbot release.
#
# This script updates workspace and UI versions, prepends CHANGELOG.md,
# commits the release bump, creates an annotated tag, and pushes the branch
# plus tag so .github/workflows/release.yml can publish GitHub Release assets.

set -euo pipefail

VERSION=""
DRY_RUN=false

usage() {
    cat <<'USAGE'
Usage: scripts/release.sh [options]

Options:
  --version <tag>  Release tag to cut, e.g. v2026.6.1.
  --dry-run        Print the planned release actions without changing files.
  -h, --help       Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="${2:?--version requires a tag}"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

repo_root() {
    git rev-parse --show-toplevel
}

calver_today() {
    local year month day
    year="$(date +%Y)"
    month="$(date +%m)"
    day="$(date +%d)"
    printf 'v%s.%d.%d\n' "$year" "$((10#$month))" "$((10#$day))"
}

validate_version() {
    local version="$1"
    if [[ ! "$version" =~ ^v[0-9]{4}\.([1-9]|1[0-2])\.([1-9]|[12][0-9]|3[01])$ ]]; then
        echo "Invalid release version: $version" >&2
        echo "Expected CalVer tag format vYYYY.M.D with no zero padding, e.g. v2026.6.1" >&2
        exit 2
    fi
}

require_clean_worktree() {
    if [[ -n "$(git status --porcelain)" ]]; then
        echo "Working tree is not clean. Commit or stash changes before cutting a release." >&2
        git status --short >&2
        exit 1
    fi
}

require_tools() {
    local missing=()
    for tool in git npm python3; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            missing+=("$tool")
        fi
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
        echo "Missing required tools: ${missing[*]}" >&2
        exit 1
    fi
}

last_release_tag() {
    git describe --tags --abbrev=0 --match 'v[0-9]*.[0-9]*.[0-9]*' 2>/dev/null || true
}

release_log_range() {
    local last_tag="$1"
    if [[ -n "$last_tag" ]]; then
        printf '%s..HEAD\n' "$last_tag"
    else
        printf 'HEAD\n'
    fi
}

update_cargo_workspace_version() {
    local plain_version="$1"
    python3 - "$plain_version" <<'PY'
from pathlib import Path
import sys

version = sys.argv[1]
path = Path("Cargo.toml")
lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
in_workspace_package = False
updated = False

for index, line in enumerate(lines):
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        in_workspace_package = stripped == "[workspace.package]"
    elif in_workspace_package and stripped.startswith("version"):
        prefix = line[: len(line) - len(line.lstrip())]
        newline = "\n" if line.endswith("\n") else ""
        lines[index] = f'{prefix}version = "{version}"{newline}'
        updated = True
        break

if not updated:
    raise SystemExit("Could not find [workspace.package] version in Cargo.toml")

path.write_text("".join(lines), encoding="utf-8")
PY
}

update_ui_version() {
    local plain_version="$1"
    npm --prefix apps/ui version "$plain_version" --no-git-tag-version --allow-same-version >/dev/null
}

prepend_changelog() {
    local version="$1" last_tag="$2" range="$3" date_stamp tmp
    date_stamp="$(date +%Y-%m-%d)"
    tmp="$(mktemp)"

    {
        printf '# Changelog\n\n'
        printf '## %s - %s\n\n' "$version" "$date_stamp"
        if [[ -n "$last_tag" ]]; then
            printf 'Changes since %s:\n\n' "\`$last_tag\`"
        else
            printf 'Initial recorded release changes:\n\n'
        fi
        git log "$range" --oneline --no-merges | sed 's/^/- /'
        printf '\n'
        if [[ -f CHANGELOG.md ]]; then
            sed '1{/^# Changelog$/d;}' CHANGELOG.md
        fi
    } > "$tmp"

    mv "$tmp" CHANGELOG.md
}

main() {
    cd "$(repo_root)"
    require_tools

    if [[ -z "$VERSION" ]]; then
        VERSION="$(calver_today)"
    fi
    validate_version "$VERSION"

    local plain_version="${VERSION#v}"
    local last_tag range current_branch
    last_tag="$(last_release_tag)"
    range="$(release_log_range "$last_tag")"
    current_branch="$(git branch --show-current)"

    if git rev-parse -q --verify "refs/tags/${VERSION}" >/dev/null; then
        echo "Release tag already exists: ${VERSION}" >&2
        exit 1
    fi

    echo "Release:       ${VERSION}"
    echo "Branch:        ${current_branch}"
    echo "Previous tag:  ${last_tag:-<none>}"
    echo "Log range:     ${range}"

    if [[ "$DRY_RUN" == "true" ]]; then
        echo ""
        echo "Dry run only. Planned actions:"
        echo "  - update Cargo.toml workspace version to ${plain_version}"
        echo "  - update apps/ui/package.json and package-lock.json to ${plain_version}"
        echo "  - prepend CHANGELOG.md with git log ${range}"
        echo "  - commit: release: ${plain_version}"
        echo "  - tag: ${VERSION}"
        echo "  - push ${current_branch} and ${VERSION}"
        exit 0
    fi

    require_clean_worktree
    update_cargo_workspace_version "$plain_version"
    update_ui_version "$plain_version"
    prepend_changelog "$VERSION" "$last_tag" "$range"

    git add Cargo.toml apps/ui/package.json apps/ui/package-lock.json CHANGELOG.md
    git commit -m "release: ${plain_version}"
    git tag -a "$VERSION" -m "Release ${plain_version}"
    git push origin "$current_branch"
    git push origin "$VERSION"
}

main "$@"
