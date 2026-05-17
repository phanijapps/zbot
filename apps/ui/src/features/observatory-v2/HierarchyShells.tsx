// ============================================================================
// HIERARCHY SHELLS — 3D layered visualization of the memory hierarchy
// ============================================================================
//
// Concentric spherical shells, one per hierarchy layer:
//   - L0 (outermost): base entities scattered across a large sphere as
//     small frosted dots. Count from `layer_counts[0]`.
//   - L1 (middle): aggregate entities as fewer, larger glowing nodes
//     positioned by member-weight on a smaller sphere.
//   - L2+ (inner): even fewer, smaller spheres. Currently we only have
//     L0 + L1 in production data — additional layers materialise here
//     automatically when they exist.
//
// Edges:
//   - Subtle radial threads from each L1 aggregate inward to the
//     centre, suggesting the hierarchical "lifting" relationship.
//   - Inter-cluster relations (count from `summary.inter_cluster_relations`)
//     drawn as soft arcs between random pairs of L1 nodes — we don't
//     yet have a precise edge list endpoint, so we synthesise visual
//     density proportional to the real count. (Phase 2 fetches the
//     real edge list.)
//
// Motion: slow ambient orbit, gentle camera drift. No jarring
// transitions. Apple-Vision palette — soft cream-white glow on near-
// black, depth fog, restrained particle count.
// ============================================================================

import { Canvas, useFrame } from "@react-three/fiber";
import { OrbitControls, AdaptiveDpr } from "@react-three/drei";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import type { AggregateSummary } from "../observatory/hierarchy/types";

interface HierarchyShellsProps {
  layerCounts: Array<[number, number]>;
  aggregates: AggregateSummary[];
  interClusterCount: number;
  enabled: boolean;
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
// lattice — gives a uniform-looking distribution without clumps.
function spherePoints(n: number, r: number, seed: number): THREE.Vector3[] {
  if (n <= 0) return [];
  const rand = seeded(seed);
  // Jitter the lattice so it doesn't look like a grid; preserves
  // uniformity but breaks regularity.
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

// Build a small radial-gradient texture so points render as soft
// disks (Apple-Vision feel) instead of square pixels. Created once
// per page lifetime via a module-level cache.
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

// L0 base-entity cloud — many small frosted dots on the outer shell.
function BaseShell({ count, radius }: { count: number; radius: number }) {
  // Cap rendered count so very large graphs don't tank the GPU. We're
  // showing a *visualisation*, not the full dataset — 800 points is
  // enough to read as "many" while staying buttery on integrated GPUs.
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

// L1 aggregate ring — larger glowing spheres sized by member_count.
function AggregateShell({
  aggregates,
  radius,
}: {
  aggregates: AggregateSummary[];
  radius: number;
}) {
  const ref = useRef<THREE.Group>(null);
  useFrame((_, delta) => {
    if (ref.current) ref.current.rotation.y -= delta * 0.025;
  });
  const positions = useMemo(
    () => spherePoints(aggregates.length, radius, 9173),
    [aggregates.length, radius],
  );
  // Size by member_count, normalised against the largest aggregate.
  const maxMembers = aggregates.reduce(
    (m, a) => Math.max(m, a.member_count),
    1,
  );
  return (
    <group ref={ref}>
      {aggregates.map((agg, i) => {
        const p = positions[i];
        if (!p) return null;
        const radiusPx = 0.08 + 0.18 * (agg.member_count / maxMembers);
        return (
          <group key={agg.id} position={[p.x, p.y, p.z]}>
            {/* Core */}
            <mesh>
              <sphereGeometry args={[radiusPx, 24, 24]} />
              <meshStandardMaterial
                color="#f8efdc"
                emissive="#f8efdc"
                emissiveIntensity={0.65}
                roughness={0.4}
              />
            </mesh>
            {/* Halo — additive sprite for the soft glow */}
            <mesh>
              <sphereGeometry args={[radiusPx * 1.8, 16, 16]} />
              <meshBasicMaterial
                color="#f8efdc"
                transparent
                opacity={0.05}
                blending={THREE.AdditiveBlending}
                depthWrite={false}
              />
            </mesh>
          </group>
        );
      })}
    </group>
  );
}

// Faint radial threads from each L1 aggregate to the centre — gives
// the "lifting toward an apex" sense without drawing every edge.
function RadialThreads({
  positions,
}: {
  positions: THREE.Vector3[];
}) {
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

// Soft arcs between random pairs of L1 nodes — a visual stand-in for
// the inter-cluster relation count until we have an edge-list endpoint.
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
    // Cap visible arc density — past ~12 the screen reads as tangled
    // rather than connected. Real edge density is still on the HUD.
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
      // Quadratic bezier with control point pushed outward from origin
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
        opacity={0.28}
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

// Ambient camera drift — adds subtle parallax so the scene feels alive
// even when the user is idle. Combined with OrbitControls, the user
// can still grab + rotate; drift resumes when they let go.
function CameraDrift() {
  const t = useRef(0);
  useFrame((state, delta) => {
    t.current += delta;
    const cam = state.camera;
    // Tiny lissajous so it doesn't feel mechanical
    cam.position.x += Math.sin(t.current * 0.21) * 0.0015;
    cam.position.y += Math.cos(t.current * 0.17) * 0.0011;
    cam.lookAt(0, 0, 0);
  });
  return null;
}

export function HierarchyShells({
  layerCounts,
  aggregates,
  interClusterCount,
  enabled,
}: HierarchyShellsProps) {
  // Pull the L0 count out of layer_counts. If layers aren't built yet
  // (or the feature is off), don't render any geometry — the empty
  // state owns the canvas instead.
  const l0 = layerCounts.find(([layer]) => layer === 0)?.[1] ?? 0;
  const L1_RADIUS = 1.4;

  const l1Positions = useMemo(
    () => spherePoints(aggregates.length, L1_RADIUS, 9173),
    [aggregates.length],
  );

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
        <>
          <BaseShell count={l0} radius={2.7} />
          <AggregateShell aggregates={aggregates} radius={L1_RADIUS} />
          <RadialThreads positions={l1Positions} />
          <InterClusterArcs
            positions={l1Positions}
            arcCount={interClusterCount}
          />
        </>
      )}

      {/* Centre apex — a small luminous core suggesting the LCA / root */}
      {enabled && aggregates.length > 0 && (
        <mesh>
          <sphereGeometry args={[0.12, 32, 32]} />
          <meshStandardMaterial
            color="#fffaef"
            emissive="#fffaef"
            emissiveIntensity={1.1}
          />
        </mesh>
      )}

      <CameraDrift />
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
