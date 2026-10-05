// ============================================================================
// SIGMA GRAPH CANVAS — sigma.js + graphology renderer (owner choice
// 2026-10-05: label quality and stability over the cosmos nebula). Same props
// boundary as the previous canvases. Nodes stream into ONE live graph
// instance (no destroy/recreate — no flicker); ForceAtlas2 runs in a worker
// with Barnes-Hut for the 17k scale and stops when the traversal settles.
// ============================================================================

import { useEffect, useMemo, useRef, useState } from "react";
import { Crosshair, Maximize2 } from "lucide-react";
import type { GraphEntity, GraphRelationship } from "@/services/transport/types";
import Graph from "graphology";
import FA2Layout from "graphology-layout-forceatlas2/worker";
import Sigma from "sigma";

interface SigmaGraphCanvasProps {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  selectedEntityId?: string;
  selectedEntityAgentId?: string;
  highlightTerm?: string;
  /** False while pages are still streaming; FA2 stops (or pauses) on settle. */
  settled: boolean;
  onEntitySelect: (entity: GraphEntity) => void;
}

/** Type palette shared with the legend (terracotta-forward, theme-anchored). */
const TYPE_PALETTE: Record<string, string> = {
  concept: "#a75935",
  tool: "#566171",
  person: "#3f6a8a",
  organization: "#78588c",
  location: "#4e8062",
  event: "#b07a2c",
  resource: "#8c624a",
  file: "#6b705c",
  artifact: "#7d5ba6",
  ward: "#4a6fa5",
};
const FALLBACK_COLOR = "#8a8a8a";
const typeColor = (type: string) => TYPE_PALETTE[type?.toLowerCase()] ?? FALLBACK_COLOR;

/** Deterministic per-index placement so page flushes never teleport nodes. */
const coord = (index: number, salt: number) => {
  const value = Math.sin(index * 12.9898 + salt * 78.233) * 43758.5453;
  return (value - Math.floor(value)) * 1000;
};

export function SigmaGraphCanvas({ entities, relationships, selectedEntityId, selectedEntityAgentId, highlightTerm, settled, onEntitySelect }: SigmaGraphCanvasProps) {
  const host = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<Sigma | null>(null);
  const graphRef = useRef<Graph | null>(null);
  const layoutRef = useRef<FA2Layout | null>(null);
  const entitiesRef = useRef(entities);
  entitiesRef.current = entities;
  const [webglUnavailable, setWebglUnavailable] = useState(false);
  const reducedMotion = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

  const indexById = useMemo(() => {
    const map = new Map<string, number>();
    entities.forEach((entity, index) => map.set(`${entity.agent_id}:${entity.id}`, index));
    return map;
  }, [entities]);

  // One Sigma instance for the component's lifetime; nodes/edges STREAM in.
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const graph = new Graph({ multi: false, type: "directed" });
    let renderer: Sigma | null = null;
    try {
      renderer = new Sigma(graph, element, {
        minCameraRatio: 0.02,
        maxCameraRatio: 20,
        labelDensity: 0.4,
        labelGridCellSize: 90,
        labelRenderedSizeThreshold: 8,
        renderEdgeLabels: false,
        defaultEdgeColor: "rgba(120,120,120,0.35)",
        zIndex: true,
      });
      renderer.on("clickNode", ({ node }) => {
        const entity = entitiesRef.current[indexById.get(node) ?? -1];
        if (entity) onEntitySelect(entity);
      });
    } catch {
      setWebglUnavailable(true);
      return;
    }
    graphRef.current = graph;
    rendererRef.current = renderer;

    const layout = new FA2Layout(graph, {
      settings: {
        barnesHutOptimize: true,
        barnesHutTheta: 0.9,
        gravity: 0.4,
        scalingRatio: 12,
        slowDown: 4,
        adjustSizes: false,
        linLogMode: false,
        outboundAttractionDistribution: false,
      },
    });
    layoutRef.current = layout;
    if (!reducedMotion) layout.start();

    return () => {
      layout.kill();
      renderer?.kill();
      graphRef.current = null;
      rendererRef.current = null;
      layoutRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Streaming data: add only NEW nodes/edges to the live graph, then refresh.
  useEffect(() => {
    const graph = graphRef.current;
    const renderer = rendererRef.current;
    if (!graph || !renderer || webglUnavailable) return;

    for (let index = 0; index < entities.length; index += 1) {
      const entity = entities[index];
      const key = `${entity.agent_id}:${entity.id}`;
      if (!graph.hasNode(key)) {
        graph.addNode(key, {
          label: entity.name,
          size: 3 + Math.sqrt(Math.max(entity.mention_count, 1)),
          color: typeColor(entity.entity_type),
          x: coord(index, 1),
          y: coord(index, 2),
          entityType: entity.entity_type?.toLowerCase() ?? "other",
          highlighted: false,
        });
      }
    }
    for (const relationship of relationships) {
      const source = `${relationship.agent_id}:${relationship.source_entity_id}`;
      const target = `${relationship.agent_id}:${relationship.target_entity_id}`;
      if (graph.hasNode(source) && graph.hasNode(target) && !graph.hasEdge(source, target)) {
        graph.addEdge(source, target, { size: 0.4, color: "rgba(120,120,120,0.3)" });
      }
    }
    renderer.refresh();
  }, [entities, relationships, webglUnavailable]);

  // Settled: stop the layout and fit the view (one reveal, then calm).
  useEffect(() => {
    if (!settled) return;
    const stopTimer = setTimeout(() => {
      layoutRef.current?.stop();
      const camera = rendererRef.current?.getCamera();
      camera?.animate({ ratio: 0.15 }, { duration: reducedMotion ? 0 : 800 });
    }, reducedMotion ? 0 : 2500);
    return () => clearTimeout(stopTimer);
  }, [settled, reducedMotion]);

  // Selection + highlight emphasis via reducers (cheap, per-frame).
  useEffect(() => {
    const renderer = rendererRef.current;
    const graph = graphRef.current;
    if (!renderer || !graph || webglUnavailable) return;

    const selectedKey = selectedEntityId && selectedEntityAgentId
      ? `${selectedEntityAgentId}:${selectedEntityId}`
      : null;
    const neighborhood = new Set<string>();
    if (selectedKey && graph.hasNode(selectedKey)) {
      graph.forEachNeighbor(selectedKey, (neighbor) => neighborhood.add(neighbor));
    }

    renderer.setSetting("nodeReducer", (node, data) => {
      if (!selectedKey) {
        if (highlightTerm && !String(data.label ?? "").toLowerCase().includes(highlightTerm.toLowerCase())) {
          return { ...data, color: "rgba(140,140,140,0.25)", zIndex: 0 };
        }
        return data;
      }
      if (node === selectedKey) return { ...data, highlighted: true, zIndex: 2 };
      if (neighborhood.has(node)) return { ...data, zIndex: 1 };
      return { ...data, color: "rgba(140,140,140,0.18)", zIndex: 0 };
    });
    renderer.setSetting("edgeReducer", (edge, data) => {
      if (!selectedKey) return data;
      const [source] = graph.extremities(edge);
      const touches = source === selectedKey || graph.opposite(edge, source) === selectedKey;
      return touches
        ? { ...data, color: "#a75935", size: 1.6 }
        : { ...data, color: "rgba(140,140,140,0.08)", hidden: graph.size > 4000 ? true : false };
    });
    renderer.refresh();
    return () => {
      renderer.setSetting("nodeReducer", null);
      renderer.setSetting("edgeReducer", null);
    };
  }, [selectedEntityId, selectedEntityAgentId, highlightTerm, webglUnavailable]);

  const legendEntries = useMemo(() => {
    const counts = new Map<string, number>();
    for (const entity of entities) {
      const key = entity.entity_type.toLowerCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
  }, [entities]);

  if (webglUnavailable) {
    return <div className="observatory__canvas observatory__canvas--fallback" role="status">
      <Crosshair size={20} aria-hidden="true" />
      <p>GPU rendering is unavailable in this browser, so the visual graph is disabled.</p>
      <p className="observatory__hint">Search and entity inspection still work and cover the whole scope — this fallback does not limit them.</p>
    </div>;
  }

  return <div className="observatory__canvas observatory__canvas--cosmos">
    <div ref={host} className="observatory__cosmos-host" aria-label="Knowledge graph canvas" role="img" />
    <div className="observatory__legend" aria-label="Entity types">
      {legendEntries.map(([type, count]) => (
        <span key={type} className="observatory__legend-entry">
          <span className="observatory__legend-dot" data-type={type} aria-hidden="true" style={{ background: typeColor(type) }} />
          {type} <span className="observatory__legend-count">{count.toLocaleString()}</span>
        </span>
      ))}
    </div>
    <button
      type="button"
      className="btn btn--ghost btn--sm observatory__fit"
      onClick={() => rendererRef.current?.getCamera().animate({ ratio: 0.15 }, { duration: 400 })}
      title="Fit view"
    >
      <Maximize2 size={14} aria-hidden="true" /> Fit
    </button>
  </div>;
}
