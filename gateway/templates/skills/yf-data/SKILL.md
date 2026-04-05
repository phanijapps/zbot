---
name: yf-data
description: Collect, normalize, and validate market data directly from yfinance for stocks, ETFs, indices, forex, crypto, and futures. Use when an agent needs reliable OHLCV downloads, multi-ticker batching, timezone-safe alignment, corporate action handling, or clean datasets for downstream analysis.
---

# YF Data

## Overview

Use direct `yfinance` calls as the data foundation. Build reusable, validated datasets for downstream technical, fundamental, options, and risk workflows.

## Workflow

1. Validate symbols and market type.
2. Pull data with `yf.download` first; fall back to `Ticker.history` when needed.
3. Normalize columns to `open/high/low/close/adj_close/volume`.
4. Align timezone to UTC and sort index ascending.
5. Detect and log gaps, duplicates, and stale end timestamps.
6. Persist outputs in runtime-defined formats and locations.

## Minimal Pattern

```python
import yfinance as yf
import pandas as pd


def fetch_ohlcv(symbols, start=None, end=None, period="1y", interval="1d"):
    raw = yf.download(
        tickers=symbols,
        start=start,
        end=end,
        period=None if start or end else period,
        interval=interval,
        auto_adjust=False,
        actions=True,
        progress=False,
        group_by="ticker",
        threads=True,
    )
    if isinstance(symbols, str):
        raw = raw.rename(columns=str.lower)
        raw.columns = [c.replace("adj close", "adj_close") for c in raw.columns]
        raw = raw.sort_index()
        raw.index = pd.to_datetime(raw.index, utc=True)
        return {symbols: raw}

    out = {}
    for symbol in symbols:
        df = raw[symbol].copy()
        df.columns = [c.lower().replace("adj close", "adj_close") for c in df.columns]
        df = df.sort_index()
        df.index = pd.to_datetime(df.index, utc=True)
        out[symbol] = df
    return out
```

## Collaboration Contract

- Coordination role: define symbols, timeframe, interval, and required fields.
- Quant role: return normalized data plus a data-quality summary.
- Research role: add exchange-calendar context when gaps or anomalies appear.
- Reporting role: include data-quality caveats before higher-level conclusions.

## Quality Gates

- Reject empty frames.
- Reject frames with unsorted or duplicate timestamps.
- Flag if the latest candle is older than expected for the interval.
- Record split/dividend rows when `actions=True`.

## References

Read `references/yfinance-recipes.md` for ticker-format rules and fallback logic by asset class.
