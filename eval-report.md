# Phase 8 Eval Report

Model: `anthropic/claude-sonnet-4.6`

Fixtures: 6

**6/6 passed.**

## 01_reuse_basic

- Description: Ward has dcf_valuation primitive; task asks for DCF on TSLA. Pass if LLM imports rather than re-implements.
- Latency: 31.4s
- Verdict: **PASS**

### Assertions

- ✓ Must import primitives from core
- ✓ Must call dcf_valuation
- ✓ Must NOT redefine dcf_valuation
- ✓ Must NOT redefine calc_wacc

### LLM response (first 500 chars of code body)

```
Looking at the ward snapshot, I can see `dcf_valuation`, `calc_wacc`, and `sensitivity_grid` are all listed primitives in `core/valuation.py`. I'll import and use all three — no re-implementation.

```python
# models/tsla-dcf.py
"""
TSLA DCF Valuation
------------------
Thin wrapper around core/valuation.py primitives.
Loads fundamentals from data/tsla-fundamentals.json,
derives WACC via CAPM, then runs DCF + sensitivity grid.
"""

import json
import pathlib
import sys

# -----------------------
…[truncated, 5869 chars omitted]
```

## 02_parameterize_multiples

- Description: Ward has get_multiples(ticker); task asks for GOOG multiples. Pass if LLM calls it with the ticker arg, doesn't write a new goog-*.py file.
- Latency: 26.2s
- Verdict: **PASS**

### Assertions

- ✓ Must import existing primitives from the relative_valuation module
- ✓ Must call get_multiples
- ✓ Must NOT redefine get_multiples
- ✓ Must NOT redefine calc_stats

### LLM response (first 500 chars of code body)

```
Looking at the ward snapshot, I can see `get_multiples`, `calc_stats`, and `calc_percentile` are all available primitives in `analysis/relative_valuation.py`. I'll import and use all three — no re-implementation needed.

```python
# analysis/goog-peer-comparison.py
"""
GOOG vs. Peer Group Valuation Comparison
Peers: META, MSFT, AMZN, AAPL, NVDA

Uses primitives from analysis/relative_valuation.py:
  - get_multiples()
  - calc_stats()
  - calc_percentile()
"""

from relative_valuation import get_
…[truncated, 5483 chars omitted]
```

## 03_justify_new_primitive

- Description: Ward has DCF/multiples primitives but no options math. Task asks for options Greeks. Pass if LLM adds a new primitive AND justifies the addition explicitly.
- Latency: 38.5s
- Verdict: **PASS**

### Assertions

- ✓ New primitive must live in core/options.py
- ✓ Must define a Greeks primitive
- ✓ Output must compute Greeks
- ✓ Must justify the new primitive explicitly

### LLM response (first 500 chars of code body)

```
## core/options.py

```python
"""
core/options.py
---------------
Reusable Black-Scholes option pricing primitives.

Provides `bs_greeks` for computing the five standard Greeks
(delta, gamma, theta, vega, rho) for European call and put options.

No existing primitive in core/valuation.py (dcf_valuation) or
analysis/relative_valuation.py (get_multiples) covers option pricing
or Greeks — those are equity-valuation tools, not derivatives math.
"""

import math
from typing import Literal


def _d1_d
…[truncated, 7743 chars omitted]
```

## 04_prior_handoff_reference

- Description: Prior state says research-agent produced data/goog-fundamentals.json. Task says 'compute DCF using that data.' Pass if LLM reads the existing file, doesn't re-fetch from yfinance.
- Latency: 17.4s
- Verdict: **PASS**

### Assertions

- ✓ Must read the existing fundamentals file
- ✓ Must open/load the JSON file
- ✓ Must NOT re-fetch from yfinance
- ✓ Must NOT use yfinance Ticker API
- ✓ Must call the primitive

### LLM response (first 500 chars of code body)

```
## models/goog-dcf.py

```python
"""
GOOG DCF Intrinsic Value Model
Loads fundamentals saved by exec-research-01 and calls the shared dcf_valuation primitive.
"""

import json
from pathlib import Path

from core.valuation import dcf_valuation

# ── 1. Load fundamentals saved by the research-agent ──────────────────────────
fundamentals_path = Path("data/goog-fundamentals.json")

with fundamentals_path.open() as f:
    fundamentals = json.load(f)

base_fcf    = fundamentals["free_cash_flow"]     
…[truncated, 3063 chars omitted]
```

## 05_empty_ward

- Description: Fresh ward with only AGENTS.md — no primitives, no handoffs. Task asks for DCF. Pass if LLM writes clean code targeting core/valuation.py (so the NEXT agent can reuse).
- Latency: 42.5s
- Verdict: **PASS**

### Assertions

- ✓ Primitive must land in core/valuation.py
- ✓ Must define dcf_valuation
- ✓ Entry script must import from core
- ✓ Must write the entry script

### LLM response (first 500 chars of code body)

```
## core/valuation.py

```python
"""
core/valuation.py
-----------------
Reusable DCF valuation primitive for the stock-analysis ward.

All monetary inputs should be in the same unit (e.g. billions USD).
"""

from __future__ import annotations

from typing import List


def dcf_valuation(
    base_fcf: float,
    growth_rates: List[float],
    wacc: float,
    terminal_growth: float,
    shares_out: float,
    debt: float,
    cash: float,
) -> dict:
    """
    Compute a DCF-based intrinsic valu
…[truncated, 8495 chars omitted]
```

## 06_ticker_variant_duplication

- Description: THE CORE PAIN POINT. Ward already has `models/aapl-dcf-model.py` as a per-ticker wrapper. Task says 'do the same for TSLA.' Pass if LLM refactors to parameterize (accept ticker as arg) instead of writing a parallel `tsla-dcf-model.py`.
- Latency: 18.6s
- Verdict: **PASS**

### Assertions

- ✓ Must NOT write a tsla-* duplicate
- ✓ Must reference or refactor the existing runner
- ✓ Must introduce a ticker/symbol parameter
- ✓ Parameterize the existing runner

### LLM response (first 500 chars of code body)

```
Looking at the snapshot, `models/aapl-dcf-model.py` is hardcoded for AAPL and the doctrine explicitly flags it as "should be parameterized when extended." The right move is to refactor it into a generic runner that accepts a ticker argument, then both AAPL and TSLA work through the same file.

I will:
1. Refactor `models/aapl-dcf-model.py` into a ticker-parametric runner (no new duplicate file).
2. Create `data/tsla.json` with TSLA inputs.
3. Keep `data/aapl.json` as-is (its schema is implied by
…[truncated, 3475 chars omitted]
```

