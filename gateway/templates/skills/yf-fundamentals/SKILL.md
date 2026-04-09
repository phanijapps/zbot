---
name: yf-fundamentals
description: Build company and asset fundamental dossiers directly from yfinance objects such as info, financial statements, earnings, and analyst metadata. Use when an agent needs valuation, growth, profitability, leverage, or earnings-quality analysis for equities.
---

# YF Fundamentals

## Overview

Produce a structured, evidence-based fundamental view using direct `yfinance.Ticker` fields and statement tables, and document exactly which fields support each conclusion.

## Workflow

1. Pull base objects:
   - `ticker.info` / `ticker.fast_info`
   - `ticker.financials`, `ticker.quarterly_financials`
   - `ticker.balance_sheet`, `ticker.cashflow`
   - `ticker.earnings_dates`, `ticker.calendar`
2. Standardize metric names and units.
3. Compute derived metrics: revenue growth, margin trends, FCF trend, debt-to-equity, ROE proxy.
4. Compare valuation context: forward PE, trailing PE, price-to-book, EV/EBITDA if available.
5. Emit a bullish/base/bear stance with explicit missing-data caveats.

## Mandatory Output Blocks

- Business snapshot
- Growth quality
- Profitability quality
- Balance-sheet risk
- Valuation context
- Upcoming catalyst calendar
- Data confidence score (`0..1`)

## Collaboration Contract

- Coordination role: request metric table and missing-field audit.
- Quant role: return structured metrics and narrative summary.
- Research role: check whether external events explain abnormal metric changes.
- Reporting role: separate high-confidence conclusions from partial-data estimates.

## Guardrails

- Never treat missing fields as zero.
- Quote report date/period for every statement metric.
- Separate reported fields from computed custom ratios.

## References

Read `references/fundamental-field-map.md` for yfinance field mappings and fallback computations.
