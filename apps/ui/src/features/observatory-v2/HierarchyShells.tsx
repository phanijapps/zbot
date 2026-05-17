// ============================================================================
// HIERARCHY SHELLS — 3D layered visualization of the memory hierarchy
// ============================================================================
//
// Concentric spherical shells, one per hierarchy layer:
//   - L0 (outermost): base entities scattered across a large sphere as
//     small frosted dots. Count from `layer_counts[0]`.
//   - L1 (middle): aggregate entities as fewer, larger glowing nodes
//     positioned by member-weight on a smaller sphere.
//   - L2+ (inner): even fewer, smaller spheres.
//
// Phase 2 additions (this file):
//   - Ambient pulse loop: every ~3-6s a random aggregate emits an
//     expanding pulse ring + brief brightness boost. The graph
//     "breathes" so the page never feels static.
//   - Hover interactivity: pointer over an aggregate brightens it +
//     draws faint tracer lines to nearby aggregates ("connections").
//   - Click → onAggregateClick callback. Parent uses this to focus
//     the camera and open a detail slideover.
//   - Apex flash: the central core pulses in sync with ambient events.
//
// Phase 3 (NOT in this PR) will add precise live RAG overlay driven
// by backend RecallTrace events.
// ============================================================================

import { Canvas, useFrame, type ThreeEvent } from "@react-three/fiber";
import { OrbitControls, AdaptiveDpr } from "@react-three/drei";
import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import type { AggregateSummary } from "../observatory/hierarchy/types";

interface HierarchyShellsProps {
  layerCounts: Array<[number, number]>;
  aggregates: AggregateSummary[];
  interClusterCount: number;
  enabled: boolean;
  /** Called when the user clicks an aggregate sphere. */
  onAggregateClick?: (agg: AggregateSummary) => void;
}

// Deterministic pseudo-random scatter so the visualisation looks the
// same across refreshes for a given input. Mulberry32 PRNG seeded by
// the input index.
function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Spread N points on a sphere of radius `r` using the Fibonacci
// lattice — uniform-looking distribution without clumps.
function spherePoints(n: number, r: number, seed: number): THREE.Vector3[] {
  if (n <= 0) return [];
  const rand = seeded(seed);
  const jitter = rand() * 0.4;
  const offset = 2 / n;
  const increment = Math.PI * (3 - Math.sqrt(5));
  const out: THREE.Vector3[] = [];
  for (let i = 0; i < n; i++) {
    const y = i * offset - 1 + offset / 2;
    const rxz = Math.sqrt(1 - y * y);
    const phi = (i + jitter) * increment;
    const x = Math.cos(phi) * rxz;
    const z = Math.sin(phi) * rxz;
    out.push(new THREE.Vector3(x * r, y * r, z * r));
  }
  return out;
}

// ============================================================================
// Soft-disk dot texture for L0 points (no square pixels)
// ============================================================================

let dotTextureCache: THREE.Texture | null = null;
function getDotTexture(): THREE.Texture {
  if (dotTextureCache) return dotTextureCache;
  const size = 64;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d")!;
  const grad = ctx.createRadialGradient(
    size / 2,
    size / 2,
    0,
    size / 2,
    size / 2,
    size / 2,
  );
  grad.addColorStop(0, "rgba(245, 236, 217, 1)");
  grad.addColorStop(0.45, "rgba(245, 236, 217, 0.6)");
  grad.addColorStop(1, "rgba(245, 236, 217, 0)");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.needsUpdate = true;
  dotTextureCache = tex;
  return tex;
}

// ============================================================================
// L0 base-entity cloud
// ============================================================================

function BaseShell({ count, radius }: { count: number; radius: number }) {
  const renderCount = Math.min(count, 800);
  const positions = useMemo(
    () => spherePoints(renderCount, radius, 1701),
    [renderCount, radius],
  );
  const ref = useRef<THREE.Points>(null);
  useFrame((_, delta) => {
    if (ref.current) ref.current.rotation.y += delta * 0.012;
  });

  const geom = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const arr = new Float32Array(positions.length * 3);
    positions.forEach((p, i) => {
      arr[i * 3 + 0] = p.x;
      arr[i * 3 + 1] = p.y;
      arr[i * 3 + 2] = p.z;
    });
    g.setAttribute("position", new THREE.BufferAttribute(arr, 3));
    return g;
  }, [positions]);

  const map = useMemo(() => getDotTexture(), []);

  return (
    <points ref={ref} geometry={geom}>
      <pointsMaterial
        color="#f5ecd9"
        size={0.085}
        sizeAttenuation
        transparent
        opacity={0.7}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
        map={map}
        alphaTest={0.02}
      />
    </points>
  );
}

// ============================================================================
// L1 aggregate node — single sphere with hover + pulse + click state
// ============================================================================

interface AggregateNodeProps {
  aggregate: AggregateSummary;
  position: THREE.Vector3;
  radius: number;
  /** True when the ambient pulse is currently lit on this node. */
  pulseT: number; // 0..1, where 0=quiet, 1=peak brightness
  onPointerOver?: () => void;
  onPointerOut?: () => void;
  onClick?: () => void;
}

function AggregateNode({
  aggregate: _aggregate,
  position,
  radius,
  pulseT,
  onPointerOver,
  onPointerOut,
  onClick,
}: AggregateNodeProps) {
  const [hovered, setHovered] = useState(false);
  const matRef = useRef<THREE.MeshStandardMaterial>(null);
  const haloRef = useRef<THREE.MeshBasicMaterial>(null);

  // Soft easing on hover boost + pulse so transitions don't snap.
  useFrame((_, delta) => {
    if (!matRef.current || !haloRef.current) return;
    const baseEmissive = 0.65;
    const hoverBoost = hovered ? 0.5 : 0;
    const pulseBoost = pulseT * 0.6;
    const target = baseEmissive + hoverBoost + pulseBoost;
    matRef.current.emissiveIntensity = THREE.MathUtils.lerp(
      matRef.current.emissiveIntensity,
      target,
      Math.min(1, delta * 6),
    );
    const haloTarget = 0.05 + (hovered ? 0.10 : 0) + pulseT * 0.18;
    haloRef.current.opacity = THREE.MathUtils.lerp(
      haloRef.current.opacity,
      haloTarget,
      Math.min(1, delta * 5),
    );
  });

  const handleOver = (e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation();
    setHovered(true);
    onPointerOver?.();
    document.body.style.cursor = "pointer";
  };
  const handleOut = (e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation();
    setHovered(false);
    onPointerOut?.();
    document.body.style.cursor = "default";
  };
  const handleClick = (e: ThreeEvent<MouseEvent>) => {
    e.stopPropagation();
    onClick?.();
  };

  return (
    <group position={[position.x, position.y, position.z]}>
      <mesh
        onPointerOver={handleOver}
        onPointerOut={handleOut}
        onClick={handleClick}
      >
        <sphereGeometry args={[radius, 24, 24]} />
        <meshStandardMaterial
          ref={matRef}
          color="#f8efdc"
          emissive="#f8efdc"
          emissiveIntensity={0.65}
          roughness={0.4}
        />
      </mesh>
      <mesh>
        <sphereGeometry args={[radius * 1.85, 16, 16]} />
        <meshBasicMaterial
          ref={haloRef}
          color="#f8efdc"
          transparent
          opacity={0.05}
          blending={THREE.AdditiveBlending}
          depthWrite={false}
        />
      </mesh>
    </group>
  );
}

// ============================================================================
// Expanding pulse ring — emits from a position, grows, fades, dies
// ============================================================================

interface PulseRingProps {
  position: THREE.Vector3;
  /** Seconds since the pulse was triggered. */
  age: number;
  /** Lifetime in seconds. */
  life: number;
  baseRadius: number;
}

function PulseRing({ position, age, life, baseRadius }: PulseRingProps) {
  const ref = useRef<THREE.Mesh>(null);
  const t = Math.min(age / life, 1);
  const scale = baseRadius * (1 + t * 4);
  const opacity = (1 - t) * 0.4;

  useFrame(() => {
    if (ref.current) {
      ref.current.scale.setScalar(scale);
      const mat = ref.current.material as THREE.MeshBasicMaterial;
      mat.opacity = opacity;
    }
  });

  // Orient the ring so its flat face points toward the camera-ish (we
  // use the position vector from origin as a stand-in for the normal —
  // looks correct enough at any orbit angle).
  const quaternion = useMemo(() => {
    const normal = position.clone().normalize();
    const q = new THREE.Quaternion();
    q.setFromUnitVectors(new THREE.Vector3(0, 0, 1), normal);
    return q;
  }, [position]);

  return (
    <mesh
      ref={ref}
      position={[position.x, position.y, position.z]}
      quaternion={quaternion}
    >
      <ringGeometry args={[0.95, 1, 48]} />
      <meshBasicMaterial
        color="#f8efdc"
        transparent
        opacity={opacity}
        side={THREE.DoubleSide}
        blending={THREE.AdditiveBlending}
        depthWrite={false}
      />
    </mesh>
  );
}

// ============================================================================
// Tracer arc — pre-computed bezier, drawn over time (progressive reveal)
// ============================================================================

interface TracerArcProps {
  from: THREE.Vector3;
  to: THREE.Vector3;
  /** 0..1 — how far along the path we've drawn. */
  progress: number;
  opacity: number;
}

function TracerArc({ from, to, progress, opacity }: TracerArcProps) {
  // Quadratic bezier through a control point pushed outward from origin.
  const path = useMemo(() => {
    const mid = from.clone().add(to).multiplyScalar(0.5);
    mid.normalize().multiplyScalar(Math.max(from.length(), to.length()) * 1.35);
    const segments = 32;
    const pts: THREE.Vector3[] = [];
    for (let i = 0; i <= segments; i++) {
      const t = i / segments;
      const oneMinus = 1 - t;
      pts.push(
        new THREE.Vector3(
          oneMinus * oneMinus * from.x + 2 * oneMinus * t * mid.x + t * t * to.x,
          oneMinus * oneMinus * from.y + 2 * oneMinus * t * mid.y + t * t * to.y,
          oneMinus * oneMinus * from.z + 2 * oneMinus * t * mid.z + t * t * to.z,
        ),
      );
    }
    return pts;
  }, [from, to]);

  // Slice the path according to progress so it draws in over time.
  const visiblePts = useMemo(() => {
    if (progress >= 1) return path;
    const count = Math.max(2, Math.floor(path.length * progress));
    return path.slice(0, count);
  }, [path, progress]);

  const geom = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const arr = new Float32Array(visiblePts.length * 3);
    visiblePts.forEach((p, i) => {
      arr[i * 3 + 0] = p.x;
      arr[i * 3 + 1] = p.y;
      arr[i * 3 + 2] = p.z;
    });
    g.setAttribute("position", new THREE.BufferAttribute(arr, 3));
    return g;
  }, [visiblePts]);

  if (visiblePts.length < 2) return null;
  return (
    <line>
      <primitive object={geom} attach="geometry" />
      <lineBasicMaterial
        color="#f8efdc"
        transparent
        opacity={opacity}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </line>
  );
}

// ============================================================================
// RadialThreads — faint static threads from L1 nodes to centre
// ============================================================================

function RadialThreads({ positions }: { positions: THREE.Vector3[] }) {
  const geom = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const arr = new Float32Array(positions.length * 6);
    positions.forEach((p, i) => {
      arr[i * 6 + 0] = 0;
      arr[i * 6 + 1] = 0;
      arr[i * 6 + 2] = 0;
      arr[i * 6 + 3] = p.x;
      arr[i * 6 + 4] = p.y;
      arr[i * 6 + 5] = p.z;
    });
    g.setAttribute("position", new THREE.BufferAttribute(arr, 3));
    return g;
  }, [positions]);
  return (
    <lineSegments geometry={geom}>
      <lineBasicMaterial
        color="#a89d83"
        transparent
        opacity={0.18}
        depthWrite={false}
      />
    </lineSegments>
  );
}

// ============================================================================
// InterClusterArcs — static visual stand-in for the inter-cluster count
// ============================================================================

function InterClusterArcs({
  positions,
  arcCount,
}: {
  positions: THREE.Vector3[];
  arcCount: number;
}) {
  const arcs = useMemo(() => {
    if (positions.length < 2 || arcCount <= 0) return [];
    const rand = seeded(4242);
    const segmentsPerArc = 14;
    const out: number[] = [];
    const max = Math.min(arcCount, 12);
    const usedPairs = new Set<string>();
    let attempts = 0;
    let drawn = 0;
    while (drawn < max && attempts < max * 6) {
      attempts += 1;
      const i = Math.floor(rand() * positions.length);
      let j = Math.floor(rand() * positions.length);
      if (j === i) j = (j + 1) % positions.length;
      const key = i < j ? `${i}-${j}` : `${j}-${i}`;
      if (usedPairs.has(key)) continue;
      usedPairs.add(key);
      const a = positions[i];
      const b = positions[j];
      const mid = a.clone().add(b).multiplyScalar(0.5);
      mid.normalize().multiplyScalar(a.length() * 1.35);
      for (let s = 0; s < segmentsPerArc; s++) {
        const t1 = s / segmentsPerArc;
        const t2 = (s + 1) / segmentsPerArc;
        const p1 = quadBezier(a, mid, b, t1);
        const p2 = quadBezier(a, mid, b, t2);
        out.push(p1.x, p1.y, p1.z, p2.x, p2.y, p2.z);
      }
      drawn += 1;
    }
    return out;
  }, [positions, arcCount]);

  const geom = useMemo(() => {
    const g = new THREE.BufferGeometry();
    if (arcs.length > 0) {
      g.setAttribute(
        "position",
        new THREE.BufferAttribute(new Float32Array(arcs), 3),
      );
    }
    return g;
  }, [arcs]);

  if (arcs.length === 0) return null;
  return (
    <lineSegments geometry={geom}>
      <lineBasicMaterial
        color="#f5ecd9"
        transparent
        opacity={0.22}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
      />
    </lineSegments>
  );
}

function quadBezier(
  a: THREE.Vector3,
  c: THREE.Vector3,
  b: THREE.Vector3,
  t: number,
): THREE.Vector3 {
  const oneMinus = 1 - t;
  return new THREE.Vector3(
    oneMinus * oneMinus * a.x + 2 * oneMinus * t * c.x + t * t * b.x,
    oneMinus * oneMinus * a.y + 2 * oneMinus * t * c.y + t * t * b.y,
    oneMinus * oneMinus * a.z + 2 * oneMinus * t * c.z + t * t * b.z,
  );
}

// ============================================================================
// Apex — luminous central core that pulses with ambient activity
// ============================================================================

function Apex({ pulseT }: { pulseT: number }) {
  const matRef = useRef<THREE.MeshStandardMaterial>(null);
  useFrame((_, delta) => {
    if (!matRef.current) return;
    const target = 1.1 + pulseT * 1.4;
    matRef.current.emissiveIntensity = THREE.MathUtils.lerp(
      matRef.current.emissiveIntensity,
      target,
      Math.min(1, delta * 4),
    );
  });
  return (
    <mesh>
      <sphereGeometry args={[0.12, 32, 32]} />
      <meshStandardMaterial
        ref={matRef}
        color="#fffaef"
        emissive="#fffaef"
        emissiveIntensity={1.1}
      />
    </mesh>
  );
}

// ============================================================================
// CameraDrift — subtle ambient parallax
// ============================================================================

function CameraDrift() {
  const t = useRef(0);
  useFrame((state, delta) => {
    t.current += delta;
    const cam = state.camera;
    cam.position.x += Math.sin(t.current * 0.21) * 0.0015;
    cam.position.y += Math.cos(t.current * 0.17) * 0.0011;
    cam.lookAt(0, 0, 0);
  });
  return null;
}

// ============================================================================
// SceneContent — owns the live state (pulses, tracers, hover)
// ============================================================================

interface SceneContentProps {
  l0Count: number;
  aggregates: AggregateSummary[];
  interClusterCount: number;
  onAggregateClick?: (agg: AggregateSummary) => void;
}

interface ActivePulse {
  /** Index into the aggregates array. */
  aggIndex: number;
  /** Wallclock ms when the pulse was emitted. */
  startedAt: number;
}

const PULSE_LIFE_S = 1.6;

function SceneContent({
  l0Count,
  aggregates,
  interClusterCount,
  onAggregateClick,
}: SceneContentProps) {
  const L1_RADIUS = 1.4;
  const positions = useMemo(
    () => spherePoints(aggregates.length, L1_RADIUS, 9173),
    [aggregates.length],
  );
  const maxMembers = aggregates.reduce(
    (m, a) => Math.max(m, a.member_count),
    1,
  );
  const radii = useMemo(
    () => aggregates.map((a) => 0.08 + 0.18 * (a.member_count / maxMembers)),
    [aggregates, maxMembers],
  );

  // ----- Ambient pulse loop ------------------------------------------------
  const [pulses, setPulses] = useState<ActivePulse[]>([]);
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  // Spawn a pulse on a random aggregate every 3-6s. Also expire old
  // pulses so the array doesn't grow unbounded.
  useEffect(() => {
    if (aggregates.length === 0) return;
    let cancelled = false;
    function schedule() {
      const ms = 3000 + Math.random() * 3000;
      setTimeout(() => {
        if (cancelled) return;
        setPulses((prev) => {
          const cutoff = performance.now() - PULSE_LIFE_S * 1000;
          const filtered = prev.filter((p) => p.startedAt >= cutoff);
          return [
            ...filtered,
            {
              aggIndex: Math.floor(Math.random() * aggregates.length),
              startedAt: performance.now(),
            },
          ];
        });
        schedule();
      }, ms);
    }
    schedule();
    return () => {
      cancelled = true;
    };
  }, [aggregates.length]);

  // Tracer arcs from hovered aggregate to its 3 nearest neighbours.
  const hoverNeighbours = useMemo(() => {
    if (hoveredIdx == null || aggregates.length < 2) return [];
    const a = positions[hoveredIdx];
    if (!a) return [];
    const dists = positions
      .map((p, i) => ({ i, d: i === hoveredIdx ? Infinity : p.distanceTo(a) }))
      .sort((x, y) => x.d - y.d)
      .slice(0, 3);
    return dists.map((d) => d.i);
  }, [hoveredIdx, positions, aggregates.length]);

  // ----- pulseT per aggregate -----
  // Compute on every frame in the parent's useFrame would be nicest but
  // we'd need a ref into each child; instead we run a 60fps update via
  // RAF and recompute the array. R3F batches re-renders, so this is fine.
  const [tick, setTick] = useState(0);
  useFrame(() => setTick((t) => (t + 1) % 1000000));

  const pulseTByAgg = useMemo(() => {
    const arr = new Array(aggregates.length).fill(0);
    const now = performance.now();
    for (const p of pulses) {
      const age = (now - p.startedAt) / 1000;
      if (age < 0 || age > PULSE_LIFE_S) continue;
      // Triangle envelope: ramp up first 20%, ramp down rest
      const t = age / PULSE_LIFE_S;
      const env = t < 0.2 ? t / 0.2 : 1 - (t - 0.2) / 0.8;
      arr[p.aggIndex] = Math.max(arr[p.aggIndex], env);
    }
    return arr;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pulses, tick, aggregates.length]);

  // Apex pulses = mean of all current aggregate pulses
  const apexPulse =
    pulseTByAgg.reduce((s, v) => s + v, 0) / Math.max(1, pulseTByAgg.length);

  return (
    <>
      <BaseShell count={l0Count} radius={2.7} />
      <RadialThreads positions={positions} />
      <InterClusterArcs positions={positions} arcCount={interClusterCount} />
      {aggregates.map((agg, i) => {
        const p = positions[i];
        if (!p) return null;
        return (
          <AggregateNode
            key={agg.id}
            aggregate={agg}
            position={p}
            radius={radii[i]}
            pulseT={pulseTByAgg[i] ?? 0}
            onPointerOver={() => setHoveredIdx(i)}
            onPointerOut={() => setHoveredIdx((h) => (h === i ? null : h))}
            onClick={() => onAggregateClick?.(agg)}
          />
        );
      })}
      {/* Pulse rings */}
      {pulses.map((p) => {
        const pos = positions[p.aggIndex];
        if (!pos) return null;
        const age = (performance.now() - p.startedAt) / 1000;
        if (age > PULSE_LIFE_S) return null;
        return (
          <PulseRing
            key={`${p.aggIndex}-${p.startedAt}`}
            position={pos}
            age={age}
            life={PULSE_LIFE_S}
            baseRadius={radii[p.aggIndex] ?? 0.1}
          />
        );
      })}
      {/* Hover tracer arcs */}
      {hoveredIdx !== null &&
        hoverNeighbours.map((j) => {
          const a = positions[hoveredIdx];
          const b = positions[j];
          if (!a || !b) return null;
          // Hover tracers draw in quickly. Use a constant progress so
          // the arc fully renders on hover (we're not animating draw-in
          // for hovers — only for ambient pulses, which would be too
          // much motion on top of the auto-rotate).
          const ageMs = tick % 1000; // dummy use of `tick` to keep this
          // ↑ keeps the lint-as-used path; the value isn't read
          void ageMs;
          return (
            <TracerArc
              key={`hover-${hoveredIdx}-${j}`}
              from={a}
              to={b}
              progress={1}
              opacity={0.45}
            />
          );
        })}
      {aggregates.length > 0 && <Apex pulseT={apexPulse} />}
      <CameraDrift />
    </>
  );
}

// ============================================================================
// HierarchyShells — Canvas root
// ============================================================================

export function HierarchyShells({
  layerCounts,
  aggregates,
  interClusterCount,
  enabled,
  onAggregateClick,
}: HierarchyShellsProps) {
  const l0 = layerCounts.find(([layer]) => layer === 0)?.[1] ?? 0;

  return (
    <Canvas
      camera={{ position: [0, 0, 5.4], fov: 50 }}
      dpr={[1, 2]}
      gl={{ antialias: true, alpha: true }}
      style={{ background: "transparent" }}
    >
      <AdaptiveDpr pixelated />
      <fog attach="fog" args={["#0a0e15", 4, 11]} />
      <ambientLight intensity={0.35} />
      <pointLight position={[3, 3, 3]} intensity={0.45} color="#f5ecd9" />
      <pointLight position={[-3, -2, -1]} intensity={0.25} color="#dbd3bf" />

      {enabled && (
        <SceneContent
          l0Count={l0}
          aggregates={aggregates}
          interClusterCount={interClusterCount}
          onAggregateClick={onAggregateClick}
        />
      )}

      <OrbitControls
        enablePan={false}
        enableZoom
        minDistance={3.5}
        maxDistance={9}
        autoRotate
        autoRotateSpeed={0.35}
      />
    </Canvas>
  );
}
