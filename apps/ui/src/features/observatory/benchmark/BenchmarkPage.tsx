// ============================================================================
// GRAPH RENDERER BENCHMARK — spec observatory-graph-completeness AC5.
// Dev-only route (/observatory/benchmark): measures the D3 baseline against
// the production cosmos.gl renderer on an identical seeded synthetic scene
// (17,000 entities / 5,000 edges; generated content only, never user data).
// The route itself never ships in production builds; raw run measurements
// append to benchmark-runs.md via the Playwright harness.
// ============================================================================

import { useEffect, useRef, useState } from "react";
import { GraphCanvas } from "../GraphCanvas";
import type { GraphEntity, GraphRelationship } from "@/services/transport/types";

const ENTITIES = 17_000;
const EDGES = 5_000;

/** Deterministic PRNG (mulberry32) so every run measures the same scene. */
function mulberry32(seed: number) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Scene {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  positions: Float32Array;
  links: Float32Array;
}

function buildScene(): Scene {
  const random = mulberry32(42);
  const entities: GraphEntity[] = [];
  for (let index = 0; index < ENTITIES; index += 1) {
    entities.push({
      id: `bench-${index}`,
      agent_id: "benchmark",
      name: `Node ${index}`,
      entity_type: index % 3 === 0 ? "concept" : "tool",
      properties: {},
      mention_count: 1 + Math.floor(random() * 10),
      first_seen_at: "2026-10-04T00:00:00Z",
      last_seen_at: "2026-10-04T00:00:00Z",
    });
  }
  const relationships: GraphRelationship[] = [];
  const links = new Float32Array(EDGES * 2);
  for (let index = 0; index < EDGES; index += 1) {
    const source = Math.floor(random() * ENTITIES);
    const target = Math.floor(random() * ENTITIES);
    links[index * 2] = source;
    links[index * 2 + 1] = target;
    relationships.push({
      id: `rel-${index}`,
      agent_id: "benchmark",
      source_entity_id: `bench-${source}`,
      target_entity_id: `bench-${target}`,
      relationship_type: "related_to",
      mention_count: 1,
    });
  }
  const positions = new Float32Array(ENTITIES * 2);
  for (let index = 0; index < ENTITIES * 2; index += 1) positions[index] = random() * 1000;
  return { entities, relationships, positions, links };
}

interface RendererResult {
  renderer: string;
  firstUsableViewMs: number;
  inputFeedbackP95Ms: number;
  peakHeapMiB: number;
  mountCycles: { beforeMiB: number; afterMiB: number };
  error?: string;
}

const now = () => performance.now();
const heapMiB = () =>
  (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory
    ? ((performance as Performance & { memory?: { usedJSHeapSize: number } }).memory!.usedJSHeapSize / 1024 / 1024)
    : 0;

/** Measure event→next-frame latency across 30 synthetic pan/zoom inputs. */
async function measureInputFeedback(target: HTMLElement, interactions = 30): Promise<number> {
  const latencies: number[] = [];
  for (let index = 0; index < interactions; index += 1) {
    const start = now();
    target.dispatchEvent(
      new WheelEvent("wheel", { deltaY: index % 2 ? 40 : -40, clientX: 400, clientY: 300, bubbles: true, cancelable: true })
    );
    await new Promise<number>(resolve => requestAnimationFrame(() => resolve(now() - start)));
    latencies.push(now() - start);
  }
  latencies.sort((a, b) => a - b);
  return latencies[Math.floor(latencies.length * 0.95)];
}

export function BenchmarkPage() {
  const [results, setResults] = useState<RendererResult[]>([]);
  const [phase, setPhase] = useState("idle");
  const cosmosHost = useRef<HTMLDivElement | null>(null);
  const d3CanvasHost = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const scene = buildScene();
      const collected: RendererResult[] = [];

      // ---- D3 baseline (the shipped GraphCanvas component) ----
      try {
        setPhase("d3");
        const d3Start = now();
        const root = d3CanvasHost.current!;
        const { createRoot } = await import("react-dom/client");
        const reactRoot = createRoot(root);
        await new Promise<void>(resolve => {
          reactRoot.render(
            <GraphCanvas
              entities={scene.entities}
              relationships={scene.relationships}
              onEntitySelect={() => {}}
            />
          );
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
        });
        const firstUsableViewMs = now() - d3Start;
        await new Promise(resolve => setTimeout(resolve, 2500)); // let the simulation settle
        const svg = root.querySelector("svg") as HTMLElement | null;
        const inputFeedbackP95Ms = svg ? await measureInputFeedback(svg) : -1;
        let peak = heapMiB();
        const peakSampler = setInterval(() => { peak = Math.max(peak, heapMiB()); }, 200);
        const before = heapMiB();
        reactRoot.unmount();
        await new Promise(resolve => setTimeout(resolve, 300));
        for (let cycle = 0; cycle < 9; cycle += 1) {
          const cycleRoot = createRoot(root);
          await new Promise<void>(resolve => {
            cycleRoot.render(
              <GraphCanvas
                entities={scene.entities}
                relationships={scene.relationships}
                onEntitySelect={() => {}}
              />
            );
            requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
          });
          cycleRoot.unmount();
          await new Promise(resolve => setTimeout(resolve, 100));
        }
        const after = heapMiB();
        clearInterval(peakSampler);
        collected.push({
          renderer: "d3-svg (shipped)",
          firstUsableViewMs: Math.round(firstUsableViewMs),
          inputFeedbackP95Ms: Math.round(inputFeedbackP95Ms),
          peakHeapMiB: Math.round(peak),
          mountCycles: { beforeMiB: Math.round(before), afterMiB: Math.round(after) },
        });
      } catch (err) {
        collected.push({ renderer: "d3-svg (shipped)", firstUsableViewMs: -1, inputFeedbackP95Ms: -1, peakHeapMiB: -1, mountCycles: { beforeMiB: -1, afterMiB: -1 }, error: String(err) });
      }

      // ---- cosmos.gl (GPU reference, devDependency) ----
      try {
        setPhase("cosmos");
        const cosmosStart = now();
        const { Graph } = await import("@cosmos.gl/graph");
        const graph = new Graph(cosmosHost.current!);
        await graph.ready;
        graph.setPointPositions(scene.positions);
        graph.setLinks(scene.links);
        graph.fitView();
        await new Promise<void>(resolve => {
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
        });
        const firstUsableViewMs = now() - cosmosStart;
        await new Promise(resolve => setTimeout(resolve, 2500));
        const canvas = cosmosHost.current!.querySelector("canvas") as HTMLElement | null;
        const inputFeedbackP95Ms = canvas ? await measureInputFeedback(canvas) : -1;
        let peak = heapMiB();
        const peakSampler = setInterval(() => { peak = Math.max(peak, heapMiB()); }, 200);
        const before = heapMiB();
        graph.destroy();
        const after = heapMiB();
        clearInterval(peakSampler);
        collected.push({
          renderer: "cosmos.gl 3.4.2",
          firstUsableViewMs: Math.round(firstUsableViewMs),
          inputFeedbackP95Ms: Math.round(inputFeedbackP95Ms),
          peakHeapMiB: Math.round(peak),
          mountCycles: { beforeMiB: Math.round(before), afterMiB: Math.round(after) },
        });
      } catch (err) {
        collected.push({ renderer: "cosmos.gl 3.4.2", firstUsableViewMs: -1, inputFeedbackP95Ms: -1, peakHeapMiB: -1, mountCycles: { beforeMiB: -1, afterMiB: -1 }, error: String(err) });
      }

      if (!cancelled) {
        setResults(collected);
        setPhase("done");
      }
    };
    void run();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (phase !== "done" || results.length === 0) return;
    const pre = document.getElementById("benchmark-results");
    if (pre) pre.textContent = JSON.stringify({ scene: { entities: ENTITIES, edges: EDGES }, results }, null, 2);
  }, [phase, results]);

  return (
    <div className="observatory-benchmark" aria-label="Graph renderer benchmark">
      <h1>Graph renderer benchmark — {ENTITIES.toLocaleString()} entities / {EDGES.toLocaleString()} edges</h1>
      <p>Seeded synthetic scene; generated content only. Phase: {phase}.</p>
      <div ref={d3CanvasHost} style={{ width: 800, height: 500, display: phase === "d3" ? "block" : "none" }} />
      <div ref={cosmosHost} style={{ width: 800, height: 500, display: phase === "cosmos" ? "block" : "none" }} />
      <pre id="benchmark-results" aria-live="polite">{results.length ? JSON.stringify({ scene: { entities: ENTITIES, edges: EDGES }, results }, null, 2) : ""}</pre>
    </div>
  );
}
