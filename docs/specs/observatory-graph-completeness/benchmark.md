# Graph renderer benchmark (spec AC5 evidence)

Scene: 17,000 entities / 5,000 edges (seeded synthetic; generated content only).
Host: Lab; CPU: Intel(R) Xeon(R) Gold 6140 CPU @ 2.30GHz; Browser: desktop Chromium (Playwright); 2026-10-05T00:30:57.271Z.

Budgets (AC5): first usable view ≤ 5000 ms; input→visible p95 ≤ 150 ms across 30 pan/zoom interactions after warmup; peak JS heap ≤ 256 MiB; no monotonic growth over ten mount/unmount cycles.

| Renderer | First usable view (ms) | Input p95 (ms) | Peak heap (MiB) | Mount cycles before→after (MiB) |
| --- | --- | --- | --- | --- |
| d3-svg (shipped) | 1092 | 1687 | 125 | 125 → 125 |
| cosmos.gl 3.4.2 | 103 | 17 | 125 | 125 → 125 |


## Verdict

- d3-svg (shipped) **fails AC5** on this scene: input→visible p95 1687 ms vs the 150 ms budget (11× over), driven by the 17k-node SVG simulation. First usable view, heap, and lifecycle budgets pass.
- cosmos.gl 3.4.2 **passes every AC5 budget** on the identical scene: 103 ms first view, 17 ms p95, 125 MiB peak, no mount-cycle growth.

Per the spec ("pin a renderer only if the current implementation fails the criterion") and the owner's standing outcome (the full force graph must be explorable), the measured decision is to **adopt cosmos.gl as the production renderer** behind a narrow boundary that preserves selection/detail/labels/zoom/fit, with the GPU-unavailable fallback (AC7). The owner ratified adoption on 2026-10-05 ("stunning visualization"): @cosmos.gl/graph is now a production dependency behind CosmosGraphCanvas (same props boundary as the D3 canvas), with the GPU-unavailable fallback (AC7). Raw re-run measurements append to benchmark-runs.md; this file keeps the verdict.
