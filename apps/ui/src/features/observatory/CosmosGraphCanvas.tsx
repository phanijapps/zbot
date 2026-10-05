// ============================================================================
// COSMOS GRAPH CANVAS — GPU force graph (cosmos.gl), the measured renderer
// (benchmark.md: D3 fails the AC5 input budget at full scale; cosmos.gl
// passes). Same props boundary as the D3 canvas so the page swaps cleanly.
// Styling follows spec AC6: restrained token-based background, type legend,
// focused readable labels (hover/selection), selected-neighborhood emphasis,
// reduced-motion support, and an explicit GPU-unavailable fallback.
// ============================================================================

import { useEffect, useMemo, useRef, useState } from "react";
import { Crosshair } from "lucide-react";
import type { GraphEntity, GraphRelationship } from "@/services/transport/types";

interface CosmosGraphCanvasProps {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  selectedEntityId?: string;
  /** The selected entity's agent — required for qualified emphasis (AC2). */
  selectedEntityAgentId?: string;
  highlightTerm?: string;
  /** False while pages are still streaming; the final rebuild always runs. */
  settled: boolean;
  onEntitySelect: (entity: GraphEntity) => void;
}

/** Curated type palette anchored on the theme's brand + neutrals. */
const TYPE_PALETTE: Array<{ type: string; rgb: [number, number, number] }> = [
  { type: "concept", rgb: [167, 89, 53] },      // --primary (terracotta)
  { type: "tool", rgb: [86, 97, 113] },         // muted steel
  { type: "person", rgb: [63, 106, 138] },      // deep blue
  { type: "organization", rgb: [120, 88, 140] },// plum
  { type: "location", rgb: [78, 128, 98] },     // pine
  { type: "event", rgb: [176, 122, 44] },       // ochre
  { type: "resource", rgb: [140, 98, 74] },     // umber
];
const FALLBACK_RGB: [number, number, number] = [120, 120, 120];

function typeRgb(entityType: string): [number, number, number] {
  const lower = entityType.toLowerCase();
  return TYPE_PALETTE.find(entry => entry.type === lower)?.rgb ?? FALLBACK_RGB;
}

/** css var -> rgb for the restrained canvas background. */
function tokenRgb(name: string, fallback: [number, number, number]): [number, number, number] {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const match = value.match(/^#([0-9a-f]{6})$/i);
  if (!match) return fallback;
  const hex = match[1];
  return [parseInt(hex.slice(0, 2), 16), parseInt(hex.slice(2, 4), 16), parseInt(hex.slice(4, 6), 16)];
}

export function CosmosGraphCanvas({ entities, relationships, selectedEntityId, selectedEntityAgentId, highlightTerm, settled, onEntitySelect }: CosmosGraphCanvasProps) {
  const host = useRef<HTMLDivElement | null>(null);
  const graphRef = useRef<{ fitView(): void; destroy(): void; setPointColors(c: Float32Array): void; setPointSizes(s: Float32Array): void; setLinkColors(c: Float32Array): void; setLinkWidths(w: Float32Array): void; pause(): void; unpause(): void } | null>(null);
  const [gpuUnavailable, setGpuUnavailable] = useState(false);
  const [sceneVersion, setSceneVersion] = useState(0);
  const lastRebuildRef = useRef(0);
  const [hover, setHover] = useState<{ x: number; y: number; entity: GraphEntity } | null>(null);
  const entitiesRef = useRef(entities);
  entitiesRef.current = entities;
  const reducedMotion = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

  const indexById = useMemo(() => {
    const map = new Map<string, number>();
    entities.forEach((entity, index) => map.set(`${entity.agent_id}:${entity.id}`, index));
    return map;
  }, [entities]);

  /** Adjacency for selected-neighborhood emphasis (AC6). */
  const neighborsOf = useMemo(() => {
    const adjacency = new Map<number, Set<number>>();
    for (const relationship of relationships) {
      const source = indexById.get(`${relationship.agent_id}:${relationship.source_entity_id}`);
      const target = indexById.get(`${relationship.agent_id}:${relationship.target_entity_id}`);
      if (source === undefined || target === undefined) continue;
      adjacency.get(source)?.add(target) ?? adjacency.set(source, new Set([target]));
      adjacency.get(target)?.add(source) ?? adjacency.set(target, new Set([source]));
    }
    return adjacency;
  }, [relationships, indexById]);

  // Scene construction + styling updates whenever data or selection changes.
  // Destroy/recreate is the only verified-correct path for changing point
  // counts; while pages stream in, rebuild at most every 1.5s so the growth
  // stays visible without a full rebuild per page. The settled flip always
  // triggers the final rebuild.
  useEffect(() => {
    if (!settled && Date.now() - lastRebuildRef.current < 1500) return;
    lastRebuildRef.current = Date.now();
    let disposed = false;
    let graph: import("@cosmos.gl/graph").Graph | null = null;
    void (async () => {
      const element = host.current;
      if (!element) return;
      const { Graph } = await import("@cosmos.gl/graph");
      if (disposed) return;
      try {
        const background = tokenRgb("--background", [255, 255, 255]);
        graph = new Graph(element, {
          backgroundColor: [...background, 1],
          enableSimulation: !reducedMotion,
          renderHoveredPointRing: true,
          onPointClick: (index: number) => {
            const entity = entitiesRef.current[index];
            if (entity) onEntitySelect(entity);
          },
          onMouseMove: (index: number | undefined, _position: [number, number] | undefined, event: MouseEvent) => {
            const entity = index !== undefined ? entitiesRef.current[index] : undefined;
            setHover(entity ? { x: event.offsetX, y: event.offsetY, entity } : null);
          },
        });
        await graph.ready;
        if (disposed) { graph.destroy(); return; }
        graphRef.current = graph;
        setSceneVersion(value => value + 1); // styling pass runs once the scene exists

        const positions = new Float32Array(entities.length * 2);
        // Deterministic per-index placement: rebuilds keep every node where
        // it was (a re-seeded random would teleport the whole graph).
        const coord = (index: number, salt: number) => {
          const value = Math.sin(index * 12.9898 + salt * 78.233) * 43758.5453;
          return (value - Math.floor(value)) * 1000;
        };
        for (let index = 0; index < entities.length; index += 1) {
          positions[index * 2] = coord(index, 1);
          positions[index * 2 + 1] = coord(index, 2);
        }
        graph.setPointPositions(positions);

        const links = new Float32Array(relationships.length * 2);
        let linkCount = 0;
        for (const relationship of relationships) {
          const source = indexById.get(`${relationship.agent_id}:${relationship.source_entity_id}`);
          const target = indexById.get(`${relationship.agent_id}:${relationship.target_entity_id}`);
          if (source === undefined || target === undefined) continue; // counted upstream, never silently hidden from totals
          links[linkCount * 2] = source;
          links[linkCount * 2 + 1] = target;
          linkCount += 1;
        }
        graph.setLinks(links.subarray(0, linkCount * 2));
        graph.fitView();
        // v3's async engine does not start its render loop until render()
        // is called explicitly (without it: device ready, canvas never
        // resizes, nothing draws).
        graph.render();
      } catch {
        // ready-reject after a successful construction still owns WebGL
        // resources — destroy the instance instead of leaking the context.
        graph?.destroy();
        if (!disposed) setGpuUnavailable(true);
        graphRef.current = null;
      }
    })();
    return () => {
      disposed = true;
      graphRef.current?.destroy();
      graphRef.current = null;
    };
    // Rebuild the scene on data identity (not length): a same-length content
    // swap must re-index nodes, links and the click/hover mapping.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entities, relationships, settled]);

  // Styling pass: type colors, mention-driven sizes, selection emphasis.
  useEffect(() => {
    const graph = graphRef.current;
    if (!graph || gpuUnavailable) return;
    // Qualified match: agent-local ids collide across agents in all-agents
    // mode (AC2), so emphasis requires BOTH the id and the selecting agent.
    const selectedIndex =
      selectedEntityId && selectedEntityAgentId
        ? entities.findIndex(
            entity => entity.id === selectedEntityId && entity.agent_id === selectedEntityAgentId
          )
        : selectedEntityId
          ? entities.findIndex(entity => entity.id === selectedEntityId)
          : -1;
    const neighborhood = selectedIndex >= 0 ? neighborsOf.get(selectedIndex) : undefined;

    const pointColors = new Float32Array(entities.length * 4);
    const pointSizes = new Float32Array(entities.length);
    entities.forEach((entity, index) => {
      const [r, g, b] = typeRgb(entity.entity_type);
      const selected = index === selectedIndex;
      const inNeighborhood = neighborhood?.has(index) ?? false;
      const highlighted = !highlightTerm || entity.name.toLowerCase().includes(highlightTerm.toLowerCase());
      const emphasis = selected ? 1 : inNeighborhood ? 0.92 : neighborhood ? 0.35 : highlighted ? 0.8 : 0.3;
      pointColors.set([r, g, b, emphasis], index * 4);
      pointSizes[index] = (selected ? 9 : inNeighborhood ? 7 : 4 + Math.sqrt(Math.max(entity.mention_count, 1)));
    });
    graph.setPointColors(pointColors);
    graph.setPointSizes(pointSizes);

    const linkColors = new Float32Array(relationships.length * 4);
    const linkWidths = new Float32Array(relationships.length);
    relationships.forEach((relationship, index) => {
      const source = indexById.get(`${relationship.agent_id}:${relationship.source_entity_id}`);
      const target = indexById.get(`${relationship.agent_id}:${relationship.target_entity_id}`);
      const touchesSelected =
        selectedIndex >= 0 && (source === selectedIndex || target === selectedIndex);
      const [r, g, b] = touchesSelected ? typeRgb("concept") : [140, 140, 140];
      const alpha = touchesSelected ? 0.9 : neighborhood ? 0.12 : 0.4;
      linkColors.set([r, g, b, alpha], index * 4);
      linkWidths[index] = touchesSelected ? 2.2 : 0.7;
    });
    graph.setLinkColors(linkColors);
    graph.setLinkWidths(linkWidths);
  }, [entities, relationships, selectedEntityId, selectedEntityAgentId, highlightTerm, neighborsOf, indexById, gpuUnavailable, sceneVersion]);

  const legendEntries = useMemo(() => {
    const counts = new Map<string, number>();
    for (const entity of entities) {
      const key = entity.entity_type.toLowerCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 7);
  }, [entities]);

  if (gpuUnavailable) {
    return <div className="observatory__canvas observatory__canvas--fallback" role="status">
      <Crosshair size={20} aria-hidden="true" />
      <p>GPU rendering is unavailable in this browser, so the visual graph is disabled.</p>
      <p className="observatory__hint">Search and entity inspection still work and cover the whole scope — this fallback does not limit them.</p>
    </div>;
  }

  return <div className="observatory__canvas observatory__canvas--cosmos">
    <div ref={host} className="observatory__cosmos-host" aria-label="Knowledge graph canvas" role="img" />
    {hover && (
      <div className="observatory__hover-chip" style={{ left: hover.x + 12, top: hover.y - 8 }} aria-hidden="true">
        <span className="observatory__hover-type" data-type={hover.entity.entity_type.toLowerCase()}>{hover.entity.entity_type}</span>
        <span className="observatory__hover-name">{hover.entity.name}</span>
      </div>
    )}
    <div className="observatory__legend" aria-label="Entity types">
      {legendEntries.map(([type, count]) => (
        <span key={type} className="observatory__legend-entry">
          <span className="observatory__legend-dot" data-type={type} aria-hidden="true" />
          {type} <span className="observatory__legend-count">{count.toLocaleString()}</span>
        </span>
      ))}
    </div>
    <button type="button" className="btn btn--ghost btn--sm observatory__fit" onClick={() => graphRef.current?.fitView()}>
      Fit view
    </button>
  </div>;
}
