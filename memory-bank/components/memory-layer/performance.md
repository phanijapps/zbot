# Memory Layer — Performance Baseline

This page preserves the memory-v2 baseline numbers that used to live under
`docs/`. Treat them as a historical benchmark, not a substitute for rerunning
the current workload.

## Baseline Results

| Area | Result | Budget | Headroom |
|---|---:|---:|---:|
| Entity resolver p95 | 2.15 ms | 20 ms | 9.3x under budget |
| Reader p95 under ingest load | 140 us | 200 ms | 1400x under budget |
| Sleep-time cycle | Within hourly interval | 60 min | Passed |

## Notes

- Resolver performance covers the alias/embedding cascade used when storing
  extracted graph entities.
- Reader performance covers recall reads while ingestion is active.
- Sleep-time performance covers the background maintenance cycle that runs
  compaction, decay, pruning, synthesis, contradiction handling, and related
  workers.

For current measurements, rerun the relevant memory benchmarks against the
same schema and data volume you are validating.
