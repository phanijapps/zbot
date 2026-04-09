# YFinance Recipes

## Symbol Formats

- US stocks/ETFs: `AAPL`, `SPY`
- Indices: `^GSPC`, `^IXIC`
- Forex: `EURUSD=X`
- Crypto: `BTC-USD`
- Futures: `GC=F`, `CL=F`

## Download Modes

- Batch mode: `yf.download([...], group_by="ticker", threads=True)`
- Single symbol fallback: `yf.Ticker(symbol).history(...)`

## Interval Constraints

- `1m` data is limited to recent windows.
- Intraday intervals have shorter lookback than daily/weekly/monthly intervals.

## Fallback Logic

1. Try `yf.download`.
2. If empty/error, call `Ticker.history`.
3. If still empty, attempt a shorter period and record the failure reason.

## Data Hygiene

- Convert index to UTC.
- Lowercase columns and standardize `adj_close`.
- Prefer both a columnar format and a portable format when persistence is needed.
