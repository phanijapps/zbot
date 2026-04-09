---
name: yf-catalysts
description: Build event and catalyst intelligence directly from yfinance news, earnings calendars, analyst actions, and price reaction windows. Use when an agent needs to explain price moves, identify upcoming catalysts, or map sentiment shifts to market behavior.
---

# YF Catalysts

## Overview

Connect price behavior to concrete catalysts using yfinance-native event feeds plus reaction-window analysis.

## Workflow

1. Pull catalyst feeds:
   - `ticker.news`
   - `ticker.calendar`
   - `ticker.earnings_dates`
   - `ticker.recommendations` / `ticker.upgrades_downgrades` when available
2. Normalize timestamps to UTC and deduplicate events.
3. Build event windows around each catalyst (default `-5d` to `+5d`).
4. Measure reaction metrics from yfinance price data:
   - gap percentage
   - day-1 and day-5 forward return
   - realized volatility change
5. Score catalyst impact (`high`, `medium`, `low`) and confidence.

## Expected Deliverables

- Structured catalyst timeline.
- Event reaction statistics.
- Narrative summary with confidence and uncertainty notes.
- Optional event-window visualizations.

## Collaboration Contract

- Coordination role: request catalyst timeline and unresolved questions.
- Quant role: compute reaction statistics for each event.
- Research role: enrich with non-yfinance context for missing macro/policy detail.
- Reporting role: distinguish observed reaction from causal claims.

## Guardrails

- Avoid single-headline causality claims without corroboration.
- Keep event timestamps and timezone assumptions explicit.
- Flag stale or low-coverage feeds.

## References

Read `references/event-study-template.md` for reaction-window defaults and scoring rules.
