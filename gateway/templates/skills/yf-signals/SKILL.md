---
name: yf-signals
description: Build technical indicator, trend, and momentum signals directly from yfinance price data. Use when an agent needs RSI, MACD, EMA, Bollinger, or ATR workflows, multi-timeframe confirmation, signal scoring, or chart-ready technical outputs.
---

# YF Signals

## Overview

Generate reproducible technical signals from direct yfinance OHLCV pulls. Keep calculations transparent so conclusions can be audited and communicated clearly.

## Workflow

1. Load cleaned OHLCV data from `yf-data` or pull directly with yfinance.
2. Compute core indicators: EMA(20/50/200), RSI(14), MACD(12,26,9), ATR(14), Bollinger(20,2).
3. Build a weighted signal score: trend + momentum + volatility + volume confirmation.
4. Run multi-timeframe checks (for example, 1d trend and 1h timing).
5. Emit machine-readable signals and analyst-friendly visual summaries.

## Signal Schema

Return one row per symbol/timeframe with:

- `trend_state`: `bullish | neutral | bearish`
- `momentum_state`: `rising | flat | falling`
- `volatility_state`: `expansion | contraction`
- `signal_score`: `-100..100`
- `risk_flags`: list of rule breaches such as overbought or low-volume breakout

## Collaboration Contract

- Coordination role: provide indicator parameters and validation expectations.
- Quant role: return signal tables, assumptions, and visualization artifacts.
- Research role: correlate large technical moves with external catalysts.
- Reporting role: summarize alignment/conflicts across indicators.

## Decision Rules

- Require at least two aligned categories before making a directional call.
- Downrank signals when volume confirmation fails.
- Mark `analysis incomplete` when bars are fewer than `longest_lookback + 20`.

## References

Read `references/indicator-recipes.md` for pandas formulas and fallback logic when `pandas_ta` is unavailable.
