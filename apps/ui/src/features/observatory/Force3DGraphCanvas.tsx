// ============================================================================
// FORCE 3D GRAPH CANVAS — three.js 3D force graph (owner choice 2026-10-05:
// "a rotatable galaxy of the knowledge graph"). Same props boundary as the
// other canvases. Nodes stream into one live graph (no rebuilds); the d3
// force layout runs in 3D; click selects with hover labels via the native
// node objects.
// ============================================================================

import { useEffect, useMemo, useRef, useState } from "react";
import { Crosshair, Maximize2 } from "lucide-react";
import ForceGraph3D from "3d-force-graph";
import type { GraphEntity, GraphRelationship } from "@/services/transport/types";

/** The instance type (the module's default IS the configured instance type
 * in its typings; use the exported interface loosely). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type ForceGraph3DInstance = any;

interface Force3DGraphCanvasProps {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  selectedEntityId?: string;
  selectedEntityAgentId?: string;
  highlightTerm?: string;
  settled: boolean;
  onEntitySelect: (entity: GraphEntity) => void;
}

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

/** Deterministic per-index 3D placement (sphere-ish shell) so streaming
 * pages never teleport existing nodes. */
const coord = (index: number, salt: number) => {
  const value = Math.sin(index * 12.9898 + salt * 78.233) * 43758.5453;
  return (value - Math.floor(value)) * 400 - 200;
};

export function Force3DGraphCanvas({ entities, relationships, selectedEntityId, selectedEntityAgentId, highlightTerm, settled, onEntitySelect }: Force3DGraphCanvasProps) {
  const host = useRef<HTMLDivElement | null>(null);
  const graphRef = useRef<ForceGraph3DInstance | null>(null);
  const nodeIndexRef = useRef(new Map<string, GraphEntity>());
  const [webglUnavailable, setWebglUnavailable] = useState(false);
  const settledRef = useRef(settled);
  settledRef.current = settled;

  // One instance for the component's lifetime.
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let graph: ForceGraph3DInstance | null = null;
    try {
      // Canonical usage: ForceGraph3D()(domElement) — the default export is
      // an instance factory; mounting happens on the second call.
      graph = (ForceGraph3D as unknown as () => (el: HTMLElement) => ForceGraph3DInstance)()(element)
        .backgroundColor("rgba(0,0,0,0)")
        .nodeLabel("name")
        .nodeVal("val")
        .nodeColor("color")
        .linkColor(() => "rgba(120,120,120,0.35)")
        .linkOpacity(0.35)
        .linkWidth(0.4)
        .onNodeClick((node: { id: string }) => {
          (window as unknown as Record<string, unknown>).__f3dClick = node.id;
          const entity = nodeIndexRef.current.get(node.id);
          if (entity) onEntitySelect(entity);
        });
      graphRef.current = graph;
      // Deterministic screen-space hit test: the library's raycast click can
      // miss under DPR transforms, so every canvas click resolves the nearest
      // projected node within a small radius (17k projections ≈ ms).
      element.addEventListener("click", (event) => {
        const g = graphRef.current;
        if (!g) return;
        const data = g.graphData();
        if (!data?.nodes?.length) return;
        const rect = element.getBoundingClientRect();
        const px = event.clientX - rect.left;
        const py = event.clientY - rect.top;
        const projected = g.graph2ScreenCoords;
        if (typeof projected !== "function") return;
        let best: { id: string; d: number } | null = null;
        for (const node of data.nodes) {
          const { x, y } = projected(node.x, node.y, node.z);
          const distance = Math.hypot(x - rect.left - px, y - rect.top - py);
          if (distance <= 10 && (!best || distance < best.d)) best = { id: node.id as string, d: distance };
        }
        if (best) {
          const entity = nodeIndexRef.current.get(best.id);
          if (entity) onEntitySelect(entity);
        }
      });
    } catch {
      setWebglUnavailable(true);
      return;
    }
    return () => {
      (graph as unknown as { _destructor?(): void })?._destructor?.();
      graphRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Streaming data: feed the graph new nodes/links, then re-heat lightly.
  useEffect(() => {
    const graph = graphRef.current;
    if (!graph || webglUnavailable) return;

    const existing = new Set<string>();
    const currentNodes = (graph.graphData()?.nodes ?? []) as Array<{ id: string }>;
    for (const node of currentNodes) existing.add(node.id);

    const byKey = nodeIndexRef.current;
    const nodes: Array<Record<string, unknown>> = currentNodes as Array<Record<string, unknown>>;
    for (let index = 0; index < entities.length; index += 1) {
      const entity = entities[index];
      const key = `${entity.agent_id}:${entity.id}`;
      byKey.set(key, entity);
      if (existing.has(key)) continue;
      nodes.push({
        id: key,
        name: entity.name,
        val: 0.5 + Math.sqrt(Math.max(entity.mention_count, 1)) * 0.3,
        color: typeColor(entity.entity_type),
        x: coord(index, 1),
        y: coord(index, 2),
        z: coord(index, 3),
      });
    }
    const nodeByKey = new Map(nodes.map((node) => [node.id as string, node]));
    const links = relationships
      .map((relationship) => {
        const source = `${relationship.agent_id}:${relationship.source_entity_id}`;
        const target = `${relationship.agent_id}:${relationship.target_entity_id}`;
        return { source, target };
      })
      .filter(({ source, target }) => nodeByKey.has(source) && nodeByKey.has(target));

    graph.graphData({ nodes: nodes as never, links: links as never });
    // Gentle re-heat while streaming; the settled flip does the final settle.
    graph.d3ReheatSimulation();
  }, [entities, relationships, webglUnavailable]);

  // Settle: zoom to fit the galaxy once the traversal completes.
  useEffect(() => {
    if (!settled) return;
    const graph = graphRef.current;
    if (!graph) return;
    const timer = setTimeout(() => {
      graph.zoomToFit(900, 60);
    }, 1200);
    return () => clearTimeout(timer);
  }, [settled]);

  // Selection emphasis: color the neighborhood, dim the rest.
  useEffect(() => {
    const graph = graphRef.current;
    if (!graph || webglUnavailable) return;
    const selectedKey = selectedEntityId && selectedEntityAgentId
      ? `${selectedEntityAgentId}:${selectedEntityId}`
      : null;
    const neighborhood = new Set<string>();
    if (selectedKey) {
      for (const relationship of relationships) {
        const s = `${relationship.agent_id}:${relationship.source_entity_id}`;
        const t = `${relationship.agent_id}:${relationship.target_entity_id}`;
        if (s === selectedKey) neighborhood.add(t);
        if (t === selectedKey) neighborhood.add(s);
      }
    }
    graph.nodeColor((node: { id: string }) => {
      const key = node.id as string;
      if (!selectedKey) {
        const entity = nodeIndexRef.current.get(key);
        if (highlightTerm && entity && !entity.name.toLowerCase().includes(highlightTerm.toLowerCase())) {
          return "rgba(140,140,140,0.25)";
        }
        return entity ? typeColor(entity.entity_type) : FALLBACK_COLOR;
      }
      if (key === selectedKey) return "#ff7d45";
      if (neighborhood.has(key)) return typeColor(nodeIndexRef.current.get(key)?.entity_type ?? "");
      return "rgba(140,140,140,0.15)";
    }).linkColor((link: { source: unknown; target: unknown }) => {
      const source = typeof link.source === "object" ? (link.source as { id: string }).id : link.source;
      const target = typeof link.target === "object" ? (link.target as { id: string }).id : link.target;
      if (!selectedKey) return "rgba(120,120,120,0.3)";
      return source === selectedKey || target === selectedKey ? "#a75935" : "rgba(120,120,120,0.06)";
    }).linkWidth((link: { source: unknown; target: unknown }) => {
      const source = typeof link.source === "object" ? (link.source as { id: string }).id : link.source;
      const target = typeof link.target === "object" ? (link.target as { id: string }).id : link.target;
      return selectedKey && (source === selectedKey || target === selectedKey) ? 1.5 : 0.4;
    }).nodeVal((node: { id: string }) => {
      const key = node.id as string;
      const entity = nodeIndexRef.current.get(key);
      const base = 0.5 + Math.sqrt(Math.max(entity?.mention_count ?? 1, 1)) * 0.3;
      return selectedKey && key === selectedKey ? base * 2.2 : base;
    });
  }, [selectedEntityId, selectedEntityAgentId, highlightTerm, relationships, webglUnavailable]);

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
    <button type="button" className="btn btn--ghost btn--sm observatory__fit"
      onClick={() => graphRef.current?.zoomToFit(700, 60)} title="Fit view">
      <Maximize2 size={14} aria-hidden="true" /> Fit
    </button>
  </div>;
}
