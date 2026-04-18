#!/usr/bin/env python3
"""Phase 8 eval harness — runs fixtures against Sonnet 4.6 via OpenRouter.

Loads every JSON fixture under ~/Documents/zbot/wards/_eval_fixtures/,
reconstructs the exact prompt the runtime sends to code-agent (reuse_check +
ward_snapshot + task), sends to Sonnet 4.6, runs the fixture's assertions
against the response, and emits eval-report.md.

The reuse_check block + ward_snapshot rendering logic mirrors the Rust
implementation in:
  gateway/gateway-execution/src/session_ctx/snapshot.rs::render_primitives
  gateway/gateway-execution/src/delegation/spawn.rs::reuse_check_block

If those Rust sources change, update this harness.
"""
import json
import os
import sys
import time
import urllib.request
from pathlib import Path

FIXTURES_DIR = Path.home() / "Documents" / "zbot" / "wards" / "_eval_fixtures"
PROVIDERS_PATH = Path.home() / "Documents" / "zbot" / "config" / "providers.json"
REPORT_PATH = Path(__file__).parent.parent / "eval-report.md"

# Overridable via env: EVAL_PROVIDER + EVAL_MODEL.
PROVIDER_NAME = os.environ.get("EVAL_PROVIDER", "OpenRouter")
MODEL = os.environ.get("EVAL_MODEL", "anthropic/claude-sonnet-4.6")

REUSE_CHECK_BLOCK = (
    "<reuse_check>\n"
    "Before writing any code, inspect the Primitives section in <ward_snapshot> below.\n"
    "If a listed primitive matches your need, IMPORT IT — do not re-implement.\n"
    "\u2713 CORRECT: `from core.valuation import dcf_valuation` then call it with new args.\n"
    "\u2713 CORRECT: Extend an existing primitive to accept a new argument (parameterize, don't duplicate).\n"
    "\u2717 WRONG: Writing `goog-dcf-model.py` when `core/valuation.py::dcf_valuation(...)` is listed.\n"
    "\u2717 WRONG: Re-implementing `calc_wacc`, `get_multiples`, or any function already listed.\n"
    "If you add genuinely new primitives (none of the listed ones fit), say so explicitly in your respond() message.\n"
    "</reuse_check>"
)


def render_primitives(primitives: list[dict]) -> str:
    """Mirror of Rust's render_primitives — group by file, list by symbol."""
    by_file: dict[str, list[dict]] = {}
    for p in primitives:
        body = p["key"].removeprefix("primitive.")
        last_dot = body.rfind(".")
        file = body[:last_dot] if last_dot > 0 else body
        by_file.setdefault(file, []).append(p)
    out = []
    for file in sorted(by_file):
        out.append(f"### {file}")
        for p in by_file[file]:
            line = f"- `{p['signature']}`"
            if p.get("summary"):
                line += f" — {p['summary']}"
            out.append(line)
    return "\n".join(out) + "\n"


def render_handoffs(handoffs: list[dict]) -> str:
    if not handoffs:
        return ""
    lines = []
    for h in handoffs:
        lines.append(f"- **[{h['exec_id']}, {h['agent_id']}]** — {h['summary']}")
    return "\n".join(lines) + "\n"


def build_snapshot(fixture: dict) -> str:
    ward_id = fixture.get("ward_id", "stock-analysis")
    parts = [f'<ward_snapshot ward="{ward_id}">']
    if fixture.get("agents_md", "").strip():
        parts.append("\n## Doctrine (AGENTS.md)\n\n" + fixture["agents_md"].rstrip() + "\n")
    if fixture.get("primitives"):
        parts.append("\n## Primitives (import these — don't duplicate)\n\n"
                     + render_primitives(fixture["primitives"]))
    if fixture.get("prior_handoffs"):
        parts.append("\n## Prior steps this session\n\n"
                     + render_handoffs(fixture["prior_handoffs"]))
    parts.append("</ward_snapshot>")
    return "\n".join(parts)


def build_prompt(fixture: dict) -> str:
    snapshot = build_snapshot(fixture)
    return f"{REUSE_CHECK_BLOCK}\n\n{snapshot}\n\n{fixture['task']}"


# ------------------------------------------------------------------
# OpenRouter call
# ------------------------------------------------------------------

def load_provider() -> dict:
    providers = json.load(PROVIDERS_PATH.open())
    for p in providers:
        if p["name"] == PROVIDER_NAME:
            return p
    raise RuntimeError(
        f"Provider '{PROVIDER_NAME}' not found in providers.json "
        f"(set EVAL_PROVIDER env var to one of: {[x['name'] for x in providers]})"
    )


def call_llm(prompt: str, provider: dict) -> dict:
    """Call the configured model via the provider's OpenAI-compatible endpoint."""
    base_url = provider["baseUrl"].rstrip("/")
    url = f"{base_url}/chat/completions"
    body = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": "You are code-agent, a coding specialist. Read the <ward_snapshot> and <reuse_check> blocks carefully before writing code. Output the complete Python files requested, using standard markdown code fences. When you would normally call respond() at the end of your work, instead write a final plaintext section titled '## respond_message' describing what you did and why."},
            {"role": "user", "content": prompt},
        ],
        "temperature": 0.1,
        "max_tokens": 4000,
    }).encode()

    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Authorization": f"Bearer {provider['apiKey']}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=180) as resp:
        payload = json.load(resp)
    return payload


def extract_content(payload: dict) -> str:
    try:
        return payload["choices"][0]["message"]["content"]
    except (KeyError, IndexError) as e:
        raise RuntimeError(f"Unexpected response shape: {e}\n{json.dumps(payload)[:500]}")


# ------------------------------------------------------------------
# Assertions
# ------------------------------------------------------------------

def split_code_and_response(content: str) -> tuple[str, str]:
    """Split the LLM output into (code_body, respond_message).
    The system prompt asks for a '## respond_message' section at the end."""
    marker = "## respond_message"
    i = content.find(marker)
    if i == -1:
        return content, ""
    return content[:i], content[i + len(marker):]


def run_assertion(code: str, respond_msg: str, rule: dict) -> tuple[bool, str]:
    t = rule["type"]
    reason = rule.get("reason", "(no reason given)")
    if t == "must_contain":
        ok = rule["value"] in code
    elif t == "must_contain_any":
        ok = any(v in code for v in rule["values"])
    elif t == "must_not_contain":
        ok = rule["value"] not in code
    elif t == "response_must_contain_any":
        hay = (respond_msg or code).lower()
        ok = any(v.lower() in hay for v in rule["values"])
    else:
        return False, f"unknown assertion type '{t}'"
    return ok, reason


def evaluate(fixture: dict, content: str) -> dict:
    code, respond_msg = split_code_and_response(content)
    results = []
    for rule in fixture["assertions"]:
        ok, reason = run_assertion(code, respond_msg, rule)
        results.append({"pass": ok, "reason": reason, "rule": rule})
    return {
        "pass": all(r["pass"] for r in results),
        "results": results,
        "code_chars": len(code),
        "response_chars": len(respond_msg),
    }


# ------------------------------------------------------------------
# Main
# ------------------------------------------------------------------

def truncate(s: str, n: int = 500) -> str:
    return s if len(s) <= n else s[:n] + f"\n…[truncated, {len(s) - n} chars omitted]"


def main():
    if not FIXTURES_DIR.is_dir():
        sys.exit(f"Fixtures directory missing: {FIXTURES_DIR}")

    provider = load_provider()
    fixtures = sorted(FIXTURES_DIR.glob("*.json"))
    print(f"Loaded {len(fixtures)} fixtures; calling {PROVIDER_NAME}/{MODEL} for each.\n")

    report: list[str] = []
    report.append(f"# Phase 8 Eval Report\n\n")
    report.append(f"Provider: `{PROVIDER_NAME}`  Model: `{MODEL}`\n\n")
    report.append(f"Fixtures: {len(fixtures)}\n\n")

    passed = 0
    for fp in fixtures:
        fixture = json.load(fp.open())
        name = fixture["name"]
        print(f"━━━ {name} ━━━")
        prompt = build_prompt(fixture)
        t0 = time.time()
        try:
            payload = call_llm(prompt, provider)
            content = extract_content(payload)
        except Exception as e:
            print(f"  ERROR: {e}\n")
            report.append(f"## {name}: **ERROR**\n\n{e}\n\n")
            continue
        elapsed = time.time() - t0
        verdict = evaluate(fixture, content)
        if verdict["pass"]:
            passed += 1
            print(f"  PASS ({elapsed:.1f}s)")
        else:
            print(f"  FAIL ({elapsed:.1f}s)")
            for r in verdict["results"]:
                if not r["pass"]:
                    print(f"    · {r['reason']}")

        report.append(f"## {name}\n\n")
        report.append(f"- Description: {fixture['description']}\n")
        report.append(f"- Latency: {elapsed:.1f}s\n")
        report.append(f"- Verdict: {'**PASS**' if verdict['pass'] else '**FAIL**'}\n\n")
        report.append("### Assertions\n\n")
        for r in verdict["results"]:
            mark = "\u2713" if r["pass"] else "\u2717"
            report.append(f"- {mark} {r['reason']}\n")
        report.append("\n### LLM response (first 500 chars of code body)\n\n")
        report.append("```\n" + truncate(content) + "\n```\n\n")

    summary = f"**{passed}/{len(fixtures)} passed.**"
    report.insert(3, f"{summary}\n\n")
    REPORT_PATH.write_text("".join(report))
    print(f"\n{summary}")
    print(f"Full report: {REPORT_PATH}")


if __name__ == "__main__":
    main()
