#!/usr/bin/env python3
"""Planner-agent eval harness.

Reads the planner's live config (~/Documents/zbot/agents/planner-agent/config.yaml),
resolves the provider from providers.json, loads the planner's AGENTS.md as the
system prompt, then runs every fixture in ./planner_fixtures/ against the
configured model.

Each fixture declares a user-facing task (the planner's "user" message) and a
list of assertions against the response. The planner is instructed (via a
SYSTEM_SUFFIX) to emit files as fenced blocks tagged with `### FILE: <path>`
instead of making tool calls, so the harness can parse its output with no
runtime machinery.

Emits eval-report-planner.md with pass/fail details per fixture.

Override via env:
  EVAL_PROVIDER  — overrides the provider id from planner config
  EVAL_MODEL     — overrides the model from planner config
"""
import json
import os
import re
import sys
import time
import urllib.request
from pathlib import Path

HOME = Path.home()
PLANNER_CONFIG = HOME / "Documents/zbot/agents/planner-agent/config.yaml"
PLANNER_PROMPT = HOME / "Documents/zbot/agents/planner-agent/AGENTS.md"
PROVIDERS = HOME / "Documents/zbot/config/providers.json"

FIXTURES_DIR = Path(__file__).parent / "planner_fixtures"
REPORT_PATH = Path(__file__).parent.parent / "eval-report-planner.md"

OVERRIDE_PROVIDER = os.environ.get("EVAL_PROVIDER")
OVERRIDE_MODEL = os.environ.get("EVAL_MODEL")


# ------------------------------------------------------------------
# Config loading (no yaml lib needed — planner config is flat key: value)
# ------------------------------------------------------------------

def load_planner_config() -> dict:
    cfg: dict = {}
    for line in PLANNER_CONFIG.read_text().splitlines():
        s = line.split("#", 1)[0].rstrip()
        if ":" in s and not s.startswith(" "):
            k, v = s.split(":", 1)
            cfg[k.strip()] = v.strip().strip("'\"")
    return {
        "provider_id": OVERRIDE_PROVIDER or cfg.get("providerId", ""),
        "model": OVERRIDE_MODEL or cfg.get("model", ""),
        "temperature": float(cfg.get("temperature", "0.5") or "0.5"),
        "maxTokens": int(cfg.get("maxTokens", "8000") or "8000"),
    }


def load_provider(provider_ref: str) -> dict:
    providers = json.loads(PROVIDERS.read_text())
    for p in providers:
        if p.get("id") == provider_ref or p.get("name") == provider_ref:
            return p
    raise RuntimeError(
        f"provider {provider_ref!r} not found. Available: "
        f"{[(p.get('id'), p.get('name')) for p in providers]}"
    )


# ------------------------------------------------------------------
# Call the planner model (OpenAI-compatible chat/completions)
# ------------------------------------------------------------------

SYSTEM_SUFFIX = """

---

## EVAL MODE INSTRUCTIONS (applied to this session only)

Do NOT call `write_file`, `edit_file`, or any tool. For each file you would
normally write via `write_file`, emit a fenced markdown block in your response
with this exact shape:

### FILE: <path>
```markdown
<full file content>
```

After emitting all files, write a final line beginning `### RESPONSE:`
followed by the one-line confirmation you would normally return. The harness
parses your output as text; tool calls will be ignored.
"""


def call_planner(
    system_prompt: str,
    user_task: str,
    provider: dict,
    model: str,
    temperature: float,
    max_tokens: int,
) -> str:
    url = f"{provider['baseUrl'].rstrip('/')}/chat/completions"
    body = json.dumps(
        {
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt + SYSTEM_SUFFIX},
                {"role": "user", "content": user_task},
            ],
            "temperature": temperature,
            "max_tokens": min(max_tokens, 8192),
        }
    ).encode()
    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Authorization": f"Bearer {provider.get('apiKey', '')}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=600) as resp:
        payload = json.load(resp)
    return payload["choices"][0]["message"]["content"]


# ------------------------------------------------------------------
# Parse the planner's text output into {path: content} + final response
# ------------------------------------------------------------------

FILE_HEADER_RE = re.compile(r"^###\s+FILE:\s+(\S+)\s*$", re.MULTILINE)


def parse_response(content: str):
    """Extract files from the planner's text output.

    The planner is instructed to emit each file as ``### FILE: <path>`` followed
    by a fenced code block. In practice the fences are sometimes dropped, so we
    fall back to "content until the next ### FILE:, ### RESPONSE:, or EOF",
    then strip a leading/trailing fence line if present.
    """
    headers = list(FILE_HEADER_RE.finditer(content))
    files = {}
    for idx, m in enumerate(headers):
        path = m.group(1).strip()
        start = m.end()
        # Next section boundary
        if idx + 1 < len(headers):
            end = headers[idx + 1].start()
        else:
            rsp = content.find("\n### RESPONSE:", start)
            end = rsp if rsp != -1 else len(content)
        body = content[start:end].strip("\n")
        # Strip leading fence (``` or ```markdown etc.) and trailing fence
        lines = body.split("\n")
        if lines and re.match(r"^```\w*\s*$", lines[0]):
            lines = lines[1:]
        if lines and lines[-1].rstrip() == "```":
            lines = lines[:-1]
        files[path] = "\n".join(lines).rstrip()

    final = None
    for line in content.splitlines():
        if line.startswith("### RESPONSE:"):
            final = line.replace("### RESPONSE:", "").strip()
            break
    return files, final


# ------------------------------------------------------------------
# Assertion helpers
# ------------------------------------------------------------------

def find_plan(files: dict) -> str | None:
    for path, body in files.items():
        if path.endswith("/plan.md") or path == "plan.md":
            return body
    return None


def find_step(files: dict, n: int) -> str | None:
    pat = re.compile(rf"steps/step{n}\.md$")
    for path, body in files.items():
        if pat.search(path):
            return body
    return None


def scope(files: dict, scope_name: str) -> str:
    if scope_name == "all":
        return "\n".join(files.values())
    if scope_name == "plan":
        return find_plan(files) or ""
    m = re.match(r"step(\d+)", scope_name)
    if m:
        return find_step(files, int(m.group(1))) or ""
    return ""


def a_classification_equals(files, value):
    plan = find_plan(files)
    if plan is None:
        return False, f"plan.md not found (files: {list(files.keys())})"
    m = re.search(r"^\*\*Classification:\*\*\s*([a-z_]+)", plan, re.MULTILINE)
    if not m:
        return False, "Classification field missing from plan.md"
    actual = m.group(1)
    return actual == value, f"actual={actual!r} expected={value!r}"


def a_step_count_equals(files, value):
    step_files = [p for p in files if re.search(r"steps/step\d+\.md$", p)]
    return len(step_files) == int(value), f"step files={len(step_files)} expected={value}"


def a_step_count_at_most(files, value):
    step_files = [p for p in files if re.search(r"steps/step\d+\.md$", p)]
    return len(step_files) <= int(value), f"step files={len(step_files)} expected ≤ {value}"


def a_step_has_skill(files, step, value):
    sp = find_step(files, step)
    if sp is None:
        return False, f"step{step}.md not found"
    m = re.search(r"^\*\*Skills:\*\*\s*(.+)$", sp, re.MULTILINE)
    if not m:
        return False, f"Skills field missing in step{step}"
    skills = m.group(1).strip()
    return value in skills, f"step{step} skills={skills!r} expected contains {value!r}"


def a_step_has_agent_in(files, step, values):
    sp = find_step(files, step)
    if sp is None:
        return False, f"step{step}.md not found"
    m = re.search(r"^\*\*Agent:\*\*\s*(\S+)", sp, re.MULTILINE)
    if not m:
        return False, f"Agent field missing in step{step}"
    agent = m.group(1).strip()
    return agent in values, f"step{step} agent={agent!r} expected in {values}"


def a_pattern_absent(files, value, in_scope="all", case_sensitive=True):
    text = scope(files, in_scope)
    needle = value if case_sensitive else value.lower()
    haystack = text if case_sensitive else text.lower()
    if needle in haystack:
        # Identify which file
        for path, body in files.items():
            hb = body if case_sensitive else body.lower()
            if needle in hb:
                return False, f"found in {path}"
        return False, f"found in scope={in_scope}"
    return True, f"absent from scope={in_scope}"


def a_pattern_present(files, value, in_scope="all", case_sensitive=True):
    text = scope(files, in_scope)
    needle = value if case_sensitive else value.lower()
    haystack = text if case_sensitive else text.lower()
    return (needle in haystack), f"{'present' if needle in haystack else 'missing'} in scope={in_scope}"


def a_no_step_assigns(files, agent, when_skills_include):
    for path, body in files.items():
        if not re.search(r"steps/step\d+\.md$", path):
            continue
        am = re.search(r"^\*\*Agent:\*\*\s*(\S+)", body, re.MULTILINE)
        sm = re.search(r"^\*\*Skills:\*\*\s*(.+)$", body, re.MULTILINE)
        if not am or not sm:
            continue
        step_agent = am.group(1).strip()
        step_skills = sm.group(1).strip()
        if step_agent == agent:
            for forbid in when_skills_include:
                if forbid in step_skills:
                    return False, f"{path}: assigns {agent} + {forbid} (forbidden)"
    return True, "no forbidden agent-skill combinations"


def a_reuse_audit_filled(files, step):
    sp = find_step(files, step)
    if sp is None:
        return False, f"step{step}.md not found"
    lf_m = re.search(r"looking_for:\s*(.+?)$", sp, re.MULTILINE)
    if not lf_m:
        return False, "reuse_audit.looking_for missing"
    lf = lf_m.group(1).strip()
    if lf in ("[]", "[ ]") or lf.lower().startswith("na"):
        return False, f"reuse_audit.looking_for is empty ({lf!r})"
    return True, f"reuse_audit.looking_for={lf!r}"


def run_assertion(files, rule):
    t = rule["type"]
    try:
        if t == "plan_classification_equals":
            return a_classification_equals(files, rule["value"])
        if t == "plan_step_count_equals":
            return a_step_count_equals(files, rule["value"])
        if t == "plan_step_count_at_most":
            return a_step_count_at_most(files, rule["value"])
        if t == "step_has_skill":
            return a_step_has_skill(files, rule["step"], rule["value"])
        if t == "step_has_agent_in":
            return a_step_has_agent_in(files, rule["step"], rule["values"])
        if t == "pattern_absent":
            return a_pattern_absent(files, rule["value"], rule.get("in", "all"), rule.get("case_sensitive", True))
        if t == "pattern_present":
            return a_pattern_present(files, rule["value"], rule.get("in", "all"), rule.get("case_sensitive", True))
        if t == "no_step_assigns":
            return a_no_step_assigns(files, rule["agent"], rule["when_skills_include"])
        if t == "reuse_audit_filled":
            return a_reuse_audit_filled(files, rule["step"])
        return False, f"unknown assertion type {t!r}"
    except Exception as e:
        return False, f"assertion error: {e}"


# ------------------------------------------------------------------
# Main
# ------------------------------------------------------------------

def truncate(s: str, n: int = 2500) -> str:
    return s if len(s) <= n else s[:n] + f"\n…[truncated {len(s)-n} chars]"


def main():
    if not FIXTURES_DIR.is_dir():
        sys.exit(f"Fixtures dir missing: {FIXTURES_DIR}")

    cfg = load_planner_config()
    if not cfg["provider_id"] or not cfg["model"]:
        sys.exit(f"Invalid planner config: {cfg}")
    provider = load_provider(cfg["provider_id"])
    system_prompt = PLANNER_PROMPT.read_text()

    fixtures = sorted(FIXTURES_DIR.glob("*.json"))
    print(
        f"Provider: {provider.get('id')} ({provider.get('name')})  "
        f"Model: {cfg['model']}  Temp: {cfg['temperature']}"
    )
    print(f"Planner prompt: {PLANNER_PROMPT} ({len(system_prompt.splitlines())} lines)")
    print(f"Fixtures: {len(fixtures)}\n")

    report = []
    report.append("# Planner Eval Report\n\n")
    report.append(
        f"Provider: `{provider.get('id')} ({provider.get('name')})`  "
        f"Model: `{cfg['model']}`  Temp: {cfg['temperature']}\n\n"
    )
    report.append(f"Planner prompt: `{PLANNER_PROMPT}` ({len(system_prompt.splitlines())} lines)\n\n")
    report.append(f"Fixtures: {len(fixtures)}\n\n")

    passed = 0
    for fp in fixtures:
        fixture = json.loads(fp.read_text())
        name = fixture["name"]
        print(f"━━━ {name} ━━━")
        t0 = time.time()
        try:
            response = call_planner(
                system_prompt,
                fixture["task"],
                provider,
                cfg["model"],
                cfg["temperature"],
                cfg["maxTokens"],
            )
        except Exception as e:
            print(f"  ERROR: {e}")
            report.append(f"## {name}: **ERROR**\n\n{e}\n\n")
            continue
        elapsed = time.time() - t0
        files, final = parse_response(response)

        results = []
        for rule in fixture.get("assertions", []):
            ok, detail = run_assertion(files, rule)
            results.append({"pass": ok, "rule": rule, "detail": detail})

        verdict = all(r["pass"] for r in results) and len(results) > 0
        if verdict:
            passed += 1
            print(f"  PASS ({elapsed:.1f}s)  files={list(files.keys())}")
        else:
            print(f"  FAIL ({elapsed:.1f}s)  files={list(files.keys())}")
            for r in results:
                if not r["pass"]:
                    print(f"    ✗ {r['rule']['type']} — {r['detail']}")

        report.append(f"## {name}\n\n")
        report.append(f"- Description: {fixture.get('description', '')}\n")
        report.append(f"- Latency: {elapsed:.1f}s\n")
        report.append(f"- Files emitted: {list(files.keys())}\n")
        report.append(f"- Final response: `{final}`\n")
        report.append(f"- Verdict: {'**PASS**' if verdict else '**FAIL**'}\n\n")
        report.append("### Assertions\n\n")
        for r in results:
            mark = "✓" if r["pass"] else "✗"
            report.append(f"- {mark} `{r['rule']['type']}` — {r['detail']}\n")
        report.append("\n### Raw response (first 2.5KB)\n\n```\n")
        report.append(truncate(response))
        report.append("\n```\n\n")

    summary = f"**{passed}/{len(fixtures)} passed**\n\n"
    report.insert(4, summary)
    REPORT_PATH.write_text("".join(report))
    print(f"\n{summary.strip()}")
    print(f"Full report: {REPORT_PATH}")


if __name__ == "__main__":
    main()
