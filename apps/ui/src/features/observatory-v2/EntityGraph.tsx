// ============================================================================
// ENTITY GRAPH — real entity-level connectivity layer (Phase 4)
// ============================================================================
//
// Renders the actual `kg_entities` + `kg_relationships` (the same data
// the legacy /observatory shows via D3-force) as a 3D shell sitting
// OUTSIDE the L1 aggregate ring. Each entity is a small typed sphere
// at a deterministic Fibonacci position; relationships connect them
// with thin cream lines.
//
// Why this layer exists: Phase 1-3 captured the macro hierarchy
// beautifully but lacked the actual entity names + relationships the
// agent's memory operates on. This brings it back without abandoning
// the layered shell aesthetic.
//
// Performance: all edges go into a single BufferGeometry so even 500
// relationships render in one draw call. Hover events fire per-entity
// (raycasted by r3f).
// ============================================================================

import { useFrame } from "@react-three/fiber";
import { Billboard, Html } from "@react-three/drei";
import { useMemo, useRef, useState } from "react";
import * as THREE from "three";
import type { GraphEntity, GraphRelationship } from "@/services/transport/types";

// Entity-type → cream-tinted color palette. Saturation pulled down
// from the legacy palette so the layer reads as a unified cream
// gradient rather than a rainbow — keeps the Apple-Vision aesthetic.
export const ENTITY_TYPE_COLORS_3D: Record<string, string> = {
  person: "#a4b6ff",
  concept: "#f5d28d",
  agent: "#8ee5b8",
  tool: "#8ee5b8",
  project: "#ff9d80",
  strategy: "#c9a8ff",
  unknown: "#dccba5",
};

function colorForType(t: string): string {
  return ENTITY_TYPE_COLORS_3D[t.toLowerCase()] ?? ENTITY_TYPE_COLORS_3D.unknown;
}

interface EntityGraphProps {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  radius: number;
  onEntityClick?: (entity: GraphEntity) => void;
}

// Deterministic Fibonacci lattice, grouped by entity_type so same-type
// entities cluster on the sphere (visual neighborhoods). Returns
// positions in the same order as the input array.
function positionsForEntities(
  entities: GraphEntity[],
  radius: number,
): THREE.Vector3[] {
  const n = entities.length;
  if (n === 0) return [];

  // Sort by entity_type so same-type ids are adjacent → adjacent
  // positions in the lattice → visual clustering. Preserve original
  // order via a permutation map.
  const indexed = entities.map((e, i) => ({ i, type: e.entity_type }));
  indexed.sort((a, b) => a.type.localeCompare(b.type));

  const offset = 2 / n;
  const increment = Math.PI * (3 - Math.sqrt(5));
  const out: THREE.Vector3[] = new Array(n);
  for (let k = 0; k < n; k++) {
    const y = k * offset - 1 + offset / 2;
    const rxz = Math.sqrt(1 - y * y);
    const phi = k * increment;
    const x = Math.cos(phi) * rxz;
    const z = Math.sin(phi) * rxz;
    out[indexed[k].i] = new THREE.Vector3(x * radius, y * radius, z * radius);
  }
  return out;
}

// Single dot/sphere with hover state. Kept lightweight — no per-frame
// material lerp since we have potentially 200 of these on screen.
interface EntityDotProps {
  entity: GraphEntity;
  position: THREE.Vector3;
  color: string;
  size: number;
  onHover: (id: string | null) => void;
  onClick?: (entity: GraphEntity) => void;
  hovered: boolean;
}

function EntityDot({
  entity,
  position,
  color,
  size,
  onHover,
  onClick,
  hovered,
}: EntityDotProps) {
  return (
    <group position={[position.x, position.y, position.z]}>
      <mesh
        onPointerOver={(e) => {
          e.stopPropagation();
          onHover(entity.id);
          document.body.style.cursor = "pointer";
        }}
        onPointerOut={(e) => {
          e.stopPropagation();
          onHover(null);
          document.body.style.cursor = "default";
        }}
        onClick={(e) => {
          e.stopPropagation();
          onClick?.(entity);
        }}
      >
        <sphereGeometry args={[size, 12, 12]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={hovered ? 1.2 : 0.55}
          roughness={0.45}
        />
      </mesh>
      {hovered && (
        <Billboard position={[0, size * 3, 0]}>
          <Html
            center
            distanceFactor={6}
            style={{ pointerEvents: "none" }}
          >
            <div className="obs2__entity-label">
              <span className="obs2__entity-label-name">{entity.name}</span>
              <span className="obs2__entity-label-meta">
                {entity.entity_type} · {entity.mention_count} mentions
              </span>
            </div>
          </Html>
        </Billboard>
      )}
    </group>
  );
}

// All relationships in a single line-segments mesh. Updates only when
// the underlying data changes — not per-frame.
function RelationshipEdges({
  relationships,
  positions,
  idToIndex,
}: {
  relationships: GraphRelationship[];
  positions: THREE.Vector3[];
  idToIndex: Map<string, number>;
}) {
  const geom = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const arr: number[] = [];
    for (const r of relationships) {
      const a = idToIndex.get(r.source_entity_id);
      const b = idToIndex.get(r.target_entity_id);
      if (a == null || b == null) continue;
      const pa = positions[a];
      const pb = positions[b];
      if (!pa || !pb) continue;
      arr.push(pa.x, pa.y, pa.z, pb.x, pb.y, pb.z);
    }
    if (arr.length > 0) {
      g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(arr), 3));
    }
    return g;
  }, [relationships, positions, idToIndex]);

  return (
    <lineSegments geometry={geom}>
      <lineBasicMaterial
        color="#f5ecd9"
        transparent
        opacity={0.18}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </lineSegments>
  );
}

export function EntityGraph({
  entities,
  relationships,
  radius,
  onEntityClick,
}: EntityGraphProps) {
  const ref = useRef<THREE.Group>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  // Slow counter-rotation against the L1 aggregate shell so the layers
  // visually separate (the L1 shell spins one way, entities the other).
  useFrame((_, delta) => {
    if (ref.current) ref.current.rotation.y += delta * 0.008;
  });

  const positions = useMemo(
    () => positionsForEntities(entities, radius),
    [entities, radius],
  );
  const idToIndex = useMemo(() => {
    const m = new Map<string, number>();
    entities.forEach((e, i) => m.set(e.id, i));
    return m;
  }, [entities]);

  // Mention-count-driven size: more mentions = bigger dot. Clamped so
  // outliers don't dominate.
  const maxMentions = entities.reduce(
    (m, e) => Math.max(m, e.mention_count),
    1,
  );

  return (
    <group ref={ref}>
      <RelationshipEdges
        relationships={relationships}
        positions={positions}
        idToIndex={idToIndex}
      />
      {entities.map((e, i) => {
        const p = positions[i];
        if (!p) return null;
        const norm = Math.min(1, e.mention_count / maxMentions);
        const size = 0.025 + norm * 0.05;
        return (
          <EntityDot
            key={e.id}
            entity={e}
            position={p}
            color={colorForType(e.entity_type)}
            size={size}
            onHover={setHoveredId}
            onClick={onEntityClick}
            hovered={hoveredId === e.id}
          />
        );
      })}
    </group>
  );
}
