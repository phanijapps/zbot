---
name: yf-options
description: Analyze options chains directly from yfinance to measure implied volatility structure, open-interest positioning, and directional options sentiment. Use when an agent needs expiry-by-expiry options diagnostics, put-call ratios, max pain estimates, or volatility regime checks.
---

# YF Options

## Overview

Extract and analyze options microstructure with direct yfinance chain pulls. Focus on volatility term structure and positioning signals that can confirm or contradict spot-price technical setups.

## Workflow

1. Load expirations with `ticker.options`.
2. For each target expiry, fetch chains using `ticker.option_chain(expiry)`.
3. Clean calls/puts tables, enforce numeric types, and drop stale rows.
4. Compute:
   - put/call open-interest ratio
   - put/call volume ratio
   - max pain estimate
   - ATM IV by expiry and term-structure slope
   - skew proxy using moneyness buckets
5. Rank expiries by abnormal activity and produce a summary narrative.

## Expected Deliverables

- Raw chain snapshots for selected expiries.
- Derived metrics tables for sentiment and IV structure.
- Narrative summary with confidence tags and caveats.
- Optional visuals for term structure and strike positioning.

## Collaboration Contract

- Coordination role: specify symbol, expiry scope, and decision horizon.
- Quant role: return raw chains plus derived metrics and assumptions.
- Research role: relate unusual activity to events or macro triggers.
- Reporting role: separate hard evidence from interpretation.

## Guardrails

- Treat yfinance options as delayed snapshots, not tick-level truth.
- Require minimum OI/liquidity thresholds before inferring sentiment.
- Mark calculations as approximate when Greeks or IV fields are sparse.

## References

Read `references/options-metrics.md` for formulas and liquidity filters.
