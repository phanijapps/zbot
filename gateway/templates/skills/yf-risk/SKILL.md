---
name: yf-risk
description: Perform portfolio construction and risk diagnostics directly from yfinance multi-asset return series. Use when an agent needs correlation analysis, drawdown and VaR metrics, stress scenarios, or weight optimization for stock, ETF, and crypto portfolios.
---

# YF Risk

## Overview

Build portfolio analytics from direct yfinance return streams and produce risk-aware allocation outputs with transparent assumptions.

## Workflow

1. Pull aligned adjusted-close data for portfolio symbols with `yf.download`.
2. Convert to daily returns and handle missing dates with documented rules.
3. Compute baseline risk set:
   - annualized return and volatility
   - Sharpe ratio
   - max drawdown and drawdown duration
   - rolling correlation matrix
   - historical VaR/CVaR
4. Run scenario checks (market shock, volatility spike, concentration stress).
5. Optionally optimize weights (max Sharpe or min volatility) with explicit constraints.

## Expected Deliverables

- Aligned return matrix.
- Risk metrics table and assumptions.
- Stress-test summary.
- Optional visualizations (correlation, drawdown, frontier).

## Collaboration Contract

- Coordination role: provide objective, constraints, and rebalance cadence.
- Quant role: return reproducible metrics and scenario assumptions.
- Research role: identify macro risks that may break historical relationships.
- Reporting role: present baseline, stress case, and trade-offs clearly.

## Guardrails

- Do not optimize without explicit constraints.
- Separate in-sample analytics from forward assumptions.
- Report data start/end dates and missing-symbol handling.

## References

Read `references/risk-formulas.md` for metric formulas and optimization defaults.
