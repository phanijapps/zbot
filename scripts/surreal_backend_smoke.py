#!/usr/bin/env python3
"""SurrealDB vs SQLite backend smoke harness.

Flips the `persistence.knowledge_backend` setting between runs and exercises
the same HTTP-API surface against both backends. Compares status codes and
response shapes (not content — content starts empty on a fresh Surreal launch).

Usage:
    python scripts/surreal_backend_smoke.py --vault /path/to/vault \\
        [--base-url http://localhost:18791] [--agent-id root]

Workflow:
    1. Sets backend=sqlite, prompts for daemon restart, probes endpoints.
    2. Sets backend=surreal, prompts for daemon restart, probes endpoints.
    3. Compares status + JSON keys side-by-side; fails on status mismatch.

The daemon must be started manually with `--features surreal-backend` for the
SurrealDB run. The harness does NOT spawn the daemon — keeping the daemon
lifecycle out of the script avoids subprocess weirdness with RocksDB locks.
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import requests


ENDPOINTS_TO_PROBE = [
    "/api/graph/test-agent/stats",
    "/api/graph/test-agent/entities?limit=10",
    "/api/graph/test-agent/relationships?limit=10",
    "/api/embeddings/health",
    "/api/memory/stats",
]


def write_settings(vault: Path, backend: str) -> None:
    settings_path = vault / "settings.json"
    settings: dict[str, Any] = {}
    if settings_path.exists():
        settings = json.loads(settings_path.read_text())
    settings.setdefault("persistence", {})
    settings["persistence"]["knowledge_backend"] = backend
    if backend == "surreal":
        settings["persistence"]["surreal"] = {
            "url": "rocksdb://$VAULT/data/knowledge.surreal",
            "namespace": "memory_kg",
            "database": "main",
            "credentials": None,
        }
    settings_path.write_text(json.dumps(settings, indent=2))


def probe(base_url: str) -> dict[str, dict[str, Any]]:
    results: dict[str, dict[str, Any]] = {}
    for ep in ENDPOINTS_TO_PROBE:
        try:
            r = requests.get(f"{base_url}{ep}", timeout=5)
            shape = None
            if r.ok:
                try:
                    body = r.json()
                    shape = sorted(body.keys()) if isinstance(body, dict) else "list"
                except ValueError:
                    shape = "non-json"
            results[ep] = {"status": r.status_code, "shape": shape}
        except requests.RequestException as e:
            results[ep] = {"error": str(e)}
    return results


def parity_report(sqlite: dict, surreal: dict) -> int:
    failed = 0
    print("--- Parity report ---")
    for ep in ENDPOINTS_TO_PROBE:
        s = sqlite.get(ep, {}).get("status")
        u = surreal.get(ep, {}).get("status")
        s_shape = sqlite.get(ep, {}).get("shape")
        u_shape = surreal.get(ep, {}).get("shape")
        if s != u:
            print(f"FAIL {ep}: status {s} (sqlite) vs {u} (surreal)")
            failed += 1
            continue
        if s_shape != u_shape:
            print(f"WARN {ep}: shape mismatch — sqlite={s_shape} surreal={u_shape}")
            continue
        print(f"OK   {ep}: status {s}, shape {s_shape}")
    return failed


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--vault", required=True, type=Path, help="Vault root directory")
    p.add_argument("--base-url", default="http://localhost:18791")
    args = p.parse_args()

    if not args.vault.exists():
        print(f"vault not found: {args.vault}", file=sys.stderr)
        return 2

    print("=== SQLite probe ===")
    write_settings(args.vault, "sqlite")
    print(f"Settings written to {args.vault}/settings.json")
    print("Restart the daemon now: cargo run -p daemon")
    input("Press Enter when daemon is healthy...")
    sqlite = probe(args.base_url)
    print(json.dumps(sqlite, indent=2))

    print("\n=== SurrealDB probe ===")
    write_settings(args.vault, "surreal")
    print(f"Settings written to {args.vault}/settings.json")
    print("Restart the daemon now: cargo run -p daemon --features surreal-backend")
    input("Press Enter when daemon is healthy...")
    surreal = probe(args.base_url)
    print(json.dumps(surreal, indent=2))

    print()
    failed = parity_report(sqlite, surreal)
    if failed:
        print(f"\n{failed} endpoint(s) diverged on status code")
        return 1
    print("\nPASS: status codes match across all probed endpoints")
    return 0


if __name__ == "__main__":
    sys.exit(main())
