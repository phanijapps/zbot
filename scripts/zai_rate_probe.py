#!/usr/bin/env python3
"""z.ai coding endpoint rate-limit probe.

Fires programming-related chat-completion requests (SSE streaming, like
zbot) and reports where 429/500s start. Use to find a safe QPS and
validate backoff strategies.

Token is read from ZAI_API_KEY env var. Rotate after testing.

Modes:
  burst    N requests back-to-back (finds the ceiling)
  paced    N requests with fixed --delay between (verify safe rate)
  backoff  Retry on 429/500 with exponential backoff + jitter

Examples:
  ZAI_API_KEY=sk-... python scripts/zai_rate_probe.py --mode burst -n 10
  ZAI_API_KEY=sk-... python scripts/zai_rate_probe.py --mode paced -n 20 --delay 2
  ZAI_API_KEY=sk-... python scripts/zai_rate_probe.py --mode backoff -n 30
"""
from __future__ import annotations

import argparse
import json
import os
import random
import statistics
import sys
import time
from dataclasses import dataclass, field
from typing import Optional

import requests

ENDPOINT = "https://api.z.ai/api/coding/paas/v4/chat/completions"
MODEL = "glm-5-turbo"
MAX_TOKENS = 512
TEMPERATURE = 0.7

PROMPTS = [
    "Explain how Python's GIL affects CPU-bound threading.",
    "Write a Rust function that reverses a linked list in place.",
    "What is the time complexity of inserting into a Rust BTreeMap?",
    "Debug this: `for i in range(len(xs)): xs.append(i)` — what happens?",
    "Refactor this TypeScript switch into a dispatch map.",
    "Why does `Arc<Mutex<T>>` beat `RwLock<T>` for write-heavy workloads?",
    "Give me a 5-line SQL query to find duplicate rows by (email, created_at).",
    "Explain the difference between structural and nominal typing with examples.",
    "How does Go's context.Context propagate cancellation across goroutines?",
    "What's the safest way to handle file descriptors in async Rust?",
    "Write a Python decorator that memoizes with a TTL.",
    "Review this code for race conditions: a goroutine reads map[int]string while main writes to it.",
    "Explain CRDTs at a practical level — what problem do they solve?",
    "Compare tokio::select! vs futures::select! — when does each matter?",
    "Give me a minimal bash one-liner to find files modified in the last hour.",
]


@dataclass
class Result:
    idx: int
    status: int
    latency_ms: float
    first_token_ms: Optional[float]
    tokens_out: int
    error: Optional[str] = None


@dataclass
class Summary:
    results: list[Result] = field(default_factory=list)

    def record(self, r: Result) -> None:
        self.results.append(r)

    def report(self) -> None:
        n = len(self.results)
        if n == 0:
            print("no requests made")
            return
        ok = [r for r in self.results if r.status == 200]
        bad = [r for r in self.results if r.status != 200]
        first_fail = next(
            (r.idx for r in self.results if r.status != 200), None
        )
        lat = sorted(r.latency_ms for r in ok)
        ttft = sorted(r.first_token_ms for r in ok if r.first_token_ms is not None)

        def pct(xs: list[float], p: float) -> float:
            if not xs:
                return 0.0
            k = max(0, min(len(xs) - 1, int(round(p * (len(xs) - 1)))))
            return xs[k]

        print()
        print("=" * 60)
        print(f"total: {n}  ok: {len(ok)}  failed: {len(bad)}  "
              f"success_rate: {len(ok)/n:.1%}")
        if first_fail is not None:
            print(f"first failure at request #{first_fail}")
        if lat:
            print(
                f"latency (ms): p50={pct(lat, 0.5):.0f} "
                f"p95={pct(lat, 0.95):.0f} mean={statistics.mean(lat):.0f}"
            )
        if ttft:
            print(
                f"time-to-first-token (ms): p50={pct(ttft, 0.5):.0f} "
                f"p95={pct(ttft, 0.95):.0f}"
            )
        if bad:
            status_counts: dict[int, int] = {}
            for r in bad:
                status_counts[r.status] = status_counts.get(r.status, 0) + 1
            print(f"failure status codes: {status_counts}")
            # show first two error bodies for debugging
            for r in bad[:2]:
                if r.error:
                    snippet = r.error[:200].replace("\n", " ")
                    print(f"  #{r.idx} [{r.status}]: {snippet}")


def build_payload(prompt: str) -> dict:
    return {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": "You are a helpful coding assistant."},
            {"role": "user", "content": prompt},
        ],
        "stream": True,
        "temperature": TEMPERATURE,
        "max_tokens": MAX_TOKENS,
    }


def one_request(
    idx: int, token: str, prompt: str, connect_timeout: float = 15.0
) -> Result:
    """Fire one SSE request, return Result with timings and token count."""
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Accept": "text/event-stream",
    }
    t0 = time.monotonic()
    first_token_ms: Optional[float] = None
    tokens_out = 0
    try:
        with requests.post(
            ENDPOINT,
            headers=headers,
            json=build_payload(prompt),
            stream=True,
            timeout=(connect_timeout, 60.0),
        ) as resp:
            if resp.status_code != 200:
                body = resp.text[:500]
                return Result(
                    idx=idx,
                    status=resp.status_code,
                    latency_ms=(time.monotonic() - t0) * 1000,
                    first_token_ms=None,
                    tokens_out=0,
                    error=body,
                )
            for raw in resp.iter_lines(decode_unicode=True):
                if not raw or not raw.startswith("data:"):
                    continue
                data = raw[5:].strip()
                if data == "[DONE]":
                    break
                if first_token_ms is None:
                    first_token_ms = (time.monotonic() - t0) * 1000
                try:
                    j = json.loads(data)
                    choices = j.get("choices") or []
                    if choices:
                        delta = choices[0].get("delta") or {}
                        if delta.get("content") or delta.get("reasoning_content"):
                            tokens_out += 1
                except json.JSONDecodeError:
                    pass
            return Result(
                idx=idx,
                status=200,
                latency_ms=(time.monotonic() - t0) * 1000,
                first_token_ms=first_token_ms,
                tokens_out=tokens_out,
            )
    except requests.exceptions.RequestException as e:
        return Result(
            idx=idx,
            status=-1,
            latency_ms=(time.monotonic() - t0) * 1000,
            first_token_ms=None,
            tokens_out=0,
            error=str(e),
        )


def fmt_result(r: Result) -> str:
    tag = "OK " if r.status == 200 else "ERR"
    ttft = f"{r.first_token_ms:.0f}ms" if r.first_token_ms else "—"
    return (
        f"[{r.idx:>3}] {tag} status={r.status} "
        f"lat={r.latency_ms:>5.0f}ms ttft={ttft:>6} tok={r.tokens_out}"
    )


def mode_burst(token: str, n: int, summary: Summary) -> None:
    print(f"BURST mode: {n} requests back-to-back")
    for i in range(n):
        prompt = PROMPTS[i % len(PROMPTS)]
        r = one_request(i, token, prompt)
        summary.record(r)
        print(fmt_result(r), flush=True)


def mode_paced(token: str, n: int, delay: float, summary: Summary) -> None:
    print(f"PACED mode: {n} requests, {delay}s delay between")
    for i in range(n):
        prompt = PROMPTS[i % len(PROMPTS)]
        r = one_request(i, token, prompt)
        summary.record(r)
        print(fmt_result(r), flush=True)
        if i < n - 1:
            time.sleep(delay)


def mode_backoff(token: str, n: int, summary: Summary) -> None:
    print(f"BACKOFF mode: {n} requests, exponential backoff on 429/500")
    max_retries = 4
    base_delay = 1.0
    for i in range(n):
        prompt = PROMPTS[i % len(PROMPTS)]
        attempt = 0
        while True:
            r = one_request(i, token, prompt)
            retryable = r.status in (429, 500, 502, 503, 504)
            if not retryable or attempt >= max_retries:
                summary.record(r)
                tag = f" (after {attempt} retries)" if attempt else ""
                print(fmt_result(r) + tag, flush=True)
                break
            sleep_s = base_delay * (2**attempt) + random.uniform(0, 0.5)
            print(
                f"[{i:>3}] retry (status={r.status}) in {sleep_s:.1f}s",
                flush=True,
            )
            time.sleep(sleep_s)
            attempt += 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--mode", choices=["burst", "paced", "backoff"], required=True)
    ap.add_argument("-n", "--num", type=int, default=10, help="number of requests")
    ap.add_argument("--delay", type=float, default=1.0, help="paced mode delay (s)")
    args = ap.parse_args()

    token = os.environ.get("ZAI_API_KEY")
    if not token:
        print("ZAI_API_KEY env var required", file=sys.stderr)
        return 2

    summary = Summary()
    t_start = time.monotonic()
    try:
        if args.mode == "burst":
            mode_burst(token, args.num, summary)
        elif args.mode == "paced":
            mode_paced(token, args.num, args.delay, summary)
        elif args.mode == "backoff":
            mode_backoff(token, args.num, summary)
    except KeyboardInterrupt:
        print("\ninterrupted")
    elapsed = time.monotonic() - t_start
    print(f"\nwall time: {elapsed:.1f}s")
    summary.report()
    return 0


if __name__ == "__main__":
    sys.exit(main())
