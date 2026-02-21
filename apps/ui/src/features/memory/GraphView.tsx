import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport } from "@/services/transport";
import type { GraphStatsResponse } from "@/services/transport/types";
import { Loader2, ZoomIn, ZoomOut, RotateCcw, Network } from "lucide-react";

interface GraphViewProps {
  agentId?: string;
}

// Color map for entity types
const ENTITY_COLORS: Record<string, string> = {
  person: "#3b82f6",
  organization: "#8b5cf6",
  location: "#10b981",
  concept: "#f59e0b",
  tool: "#ef4444",
  project: "#ec4899",
  default: "#6b7280",
};

// Simple node for force simulation
interface GraphNode {
  id: string;
  name: string;
  type: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  connections: number;
}

interface GraphEdge {
  source: string;
  target: string;
  type: string;
}

export function GraphView({ agentId }: GraphViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<GraphStatsResponse | null>(null);
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [edges, setEdges] = useState<GraphEdge[]>([]);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [hoveredNode, setHoveredNode] = useState<GraphNode | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const animationRef = useRef<number | undefined>(undefined);

  // Fetch graph data
  useEffect(() => {
    const fetchGraphData = async () => {
      setLoading(true);
      setError(null);
      try {
        const transport = await getTransport();

        // Use root agent if no agent selected
        const effectiveAgentId = agentId || "root";

        // Fetch stats and entities in parallel
        const [statsResult, entitiesResult, relationshipsResult] = await Promise.all([
          transport.getGraphStats(effectiveAgentId),
          transport.getGraphEntities(effectiveAgentId, { limit: 100 }),
          transport.getGraphRelationships(effectiveAgentId, { limit: 200 }),
        ]);

        if (statsResult.success && statsResult.data) {
          setStats(statsResult.data);
        }

        if (entitiesResult.success && entitiesResult.data && relationshipsResult.success && relationshipsResult.data) {
          const entities = entitiesResult.data.entities;
          const relationships = relationshipsResult.data.relationships;
          const entityMap = new Map(entities.map((e) => [e.id, e]));

          // Count connections for each entity
          const connectionCounts: Record<string, number> = {};
          for (const rel of relationships) {
            connectionCounts[rel.source_entity_id] = (connectionCounts[rel.source_entity_id] || 0) + 1;
            connectionCounts[rel.target_entity_id] = (connectionCounts[rel.target_entity_id] || 0) + 1;
          }

          // Create nodes with random positions
          const centerX = 400;
          const centerY = 300;
          const graphNodes: GraphNode[] = entities.map((e, i) => {
            const angle = (2 * Math.PI * i) / entities.length;
            const radius = 150 + Math.random() * 100;
            return {
              id: e.id,
              name: e.name,
              type: e.entity_type,
              x: centerX + radius * Math.cos(angle),
              y: centerY + radius * Math.sin(angle),
              vx: 0,
              vy: 0,
              connections: connectionCounts[e.id] || 0,
            };
          });

          // Create edges
          const graphEdges: GraphEdge[] = relationships
            .filter((rel) => entityMap.has(rel.source_entity_id) && entityMap.has(rel.target_entity_id))
            .map((rel) => ({
              source: rel.source_entity_id,
              target: rel.target_entity_id,
              type: rel.relationship_type,
            }));

          setNodes(graphNodes);
          setEdges(graphEdges);
        } else {
          setError("Failed to load graph data");
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load graph");
      } finally {
        setLoading(false);
      }
    };

    fetchGraphData();
  }, [agentId]);

  // Simple force simulation
  const simulateForces = useCallback(() => {
    if (nodes.length === 0) return;

    const newNodes = [...nodes];
    const centerX = 400;
    const centerY = 300;

    // Apply forces
    for (let i = 0; i < newNodes.length; i++) {
      const node = newNodes[i];

      // Center gravity
      node.vx += (centerX - node.x) * 0.001;
      node.vy += (centerY - node.y) * 0.001;

      // Repulsion between nodes
      for (let j = i + 1; j < newNodes.length; j++) {
        const other = newNodes[j];
        const dx = other.x - node.x;
        const dy = other.y - node.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = 500 / (dist * dist);
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        node.vx -= fx;
        node.vy -= fy;
        other.vx += fx;
        other.vy += fy;
      }
    }

    // Edge attraction
    for (const edge of edges) {
      const source = newNodes.find((n) => n.id === edge.source);
      const target = newNodes.find((n) => n.id === edge.target);
      if (source && target) {
        const dx = target.x - source.x;
        const dy = target.y - source.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = (dist - 100) * 0.01;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        source.vx += fx;
        source.vy += fy;
        target.vx -= fx;
        target.vy -= fy;
      }
    }

    // Apply velocity with damping
    for (const node of newNodes) {
      node.vx *= 0.9;
      node.vy *= 0.9;
      node.x += node.vx;
      node.y += node.vy;

      // Keep within bounds
      node.x = Math.max(50, Math.min(750, node.x));
      node.y = Math.max(50, Math.min(550, node.y));
    }

    setNodes(newNodes);
  }, [nodes, edges]);

  // Animation loop
  useEffect(() => {
    if (nodes.length === 0 || loading) return;

    let frameCount = 0;
    const animate = () => {
      if (frameCount < 300) { // Run for ~5 seconds at 60fps
        simulateForces();
        frameCount++;
      }
      animationRef.current = requestAnimationFrame(animate);
    };
    animationRef.current = requestAnimationFrame(animate);

    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [nodes.length, loading, simulateForces]);

  // Render canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;

    // Clear
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--card").trim() || "#1e1e2e";
    ctx.fillRect(0, 0, width, height);

    ctx.save();
    ctx.translate(pan.x, pan.y);
    ctx.scale(zoom, zoom);

    // Draw edges
    ctx.strokeStyle = "rgba(100, 100, 100, 0.3)";
    ctx.lineWidth = 1;
    for (const edge of edges) {
      const source = nodes.find((n) => n.id === edge.source);
      const target = nodes.find((n) => n.id === edge.target);
      if (source && target) {
        ctx.beginPath();
        ctx.moveTo(source.x, source.y);
        ctx.lineTo(target.x, target.y);
        ctx.stroke();

        // Draw edge label at midpoint
        const midX = (source.x + target.x) / 2;
        const midY = (source.y + target.y) / 2;
        ctx.fillStyle = "rgba(150, 150, 150, 0.6)";
        ctx.font = "9px sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(edge.type, midX, midY);
      }
    }

    // Draw nodes
    for (const node of nodes) {
      const isSelected = selectedNode?.id === node.id;
      const isHovered = hoveredNode?.id === node.id;
      const baseRadius = 8 + Math.min(node.connections * 2, 10);
      const radius = isHovered ? baseRadius + 3 : baseRadius;

      // Node circle
      const color = ENTITY_COLORS[node.type.toLowerCase()] || ENTITY_COLORS.default;
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      ctx.fillStyle = isSelected ? "#ffffff" : color;
      ctx.fill();
      ctx.strokeStyle = isSelected ? color : "rgba(255, 255, 255, 0.5)";
      ctx.lineWidth = isSelected ? 3 : 1;
      ctx.stroke();

      // Node label
      if (isHovered || isSelected || node.connections > 2) {
        ctx.fillStyle = "#ffffff";
        ctx.font = `${isHovered ? "bold " : ""}11px sans-serif`;
        ctx.textAlign = "center";
        ctx.fillText(node.name, node.x, node.y - radius - 5);
      }
    }

    ctx.restore();
  }, [nodes, edges, zoom, pan, selectedNode, hoveredNode]);

  // Handle mouse interactions
  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const x = (e.clientX - rect.left - pan.x) / zoom;
    const y = (e.clientY - rect.top - pan.y) / zoom;

    if (isDragging) {
      setPan({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      });
      return;
    }

    // Find hovered node
    const hovered = nodes.find((node) => {
      const dx = node.x - x;
      const dy = node.y - y;
      return Math.sqrt(dx * dx + dy * dy) < 15;
    });
    setHoveredNode(hovered || null);
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (hoveredNode) {
      setSelectedNode(hoveredNode);
    } else {
      setIsDragging(true);
      setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y });
    }
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setZoom((z) => Math.max(0.5, Math.min(3, z * delta)));
  };

  const resetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
    setSelectedNode(null);
  };

  if (loading) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: 400,
          color: "var(--muted-foreground)",
        }}
      >
        <Loader2 style={{ width: 24, height: 24, marginRight: "var(--spacing-2)", animation: "spin 1s linear infinite" }} />
        Loading knowledge graph...
      </div>
    );
  }

  if (error) {
    return (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: 400,
          color: "var(--destructive)",
        }}
      >
        <p>{error}</p>
      </div>
    );
  }

  if (nodes.length === 0) {
    return (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: 400,
          color: "var(--muted-foreground)",
        }}
      >
        <Network style={{ width: 48, height: 48, marginBottom: "var(--spacing-3)", opacity: 0.5 }} />
        <p style={{ fontSize: "var(--text-lg)", fontWeight: 500 }}>No knowledge graph data</p>
        <p style={{ fontSize: "var(--text-sm)", marginTop: "var(--spacing-1)" }}>
          Entities and relationships will appear here after conversations
        </p>
      </div>
    );
  }

  return (
    <div>
      {/* Stats bar */}
      {stats && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--spacing-3)",
            marginBottom: "var(--spacing-3)",
            padding: "var(--spacing-2) var(--spacing-3)",
            backgroundColor: "var(--muted)",
            borderRadius: "var(--radius-md)",
            fontSize: "var(--text-sm)",
            flexWrap: "wrap",
          }}
        >
          <span>
            <strong>{stats.entity_count}</strong> entities
          </span>
          <span>
            <strong>{stats.relationship_count}</strong> relationships
          </span>
          {Object.entries(stats.entity_types).slice(0, 5).map(([type, count]) => (
            <span
              key={type}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--spacing-1)",
              }}
            >
              <span
                style={{
                  width: 10,
                  height: 10,
                  borderRadius: "50%",
                  backgroundColor: ENTITY_COLORS[type.toLowerCase()] || ENTITY_COLORS.default,
                }}
              />
              {type}: {count}
            </span>
          ))}
        </div>
      )}

      {/* Canvas controls */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--spacing-2)",
          marginBottom: "var(--spacing-3)",
        }}
      >
        <button onClick={() => setZoom((z) => Math.min(3, z * 1.2))} className="btn btn--ghost btn--sm" title="Zoom in">
          <ZoomIn style={{ width: 16, height: 16 }} />
        </button>
        <button onClick={() => setZoom((z) => Math.max(0.5, z * 0.8))} className="btn btn--ghost btn--sm" title="Zoom out">
          <ZoomOut style={{ width: 16, height: 16 }} />
        </button>
        <button onClick={resetView} className="btn btn--ghost btn--sm" title="Reset view">
          <RotateCcw style={{ width: 16, height: 16 }} />
        </button>
        <span style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)" }}>
          Zoom: {(zoom * 100).toFixed(0)}%
        </span>
      </div>

      {/* Canvas */}
      <div
        style={{
          border: "1px solid var(--border)",
          borderRadius: "var(--radius-md)",
          overflow: "hidden",
        }}
      >
        <canvas
          ref={canvasRef}
          width={800}
          height={600}
          onMouseMove={handleMouseMove}
          onMouseDown={handleMouseDown}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          onWheel={handleWheel}
          style={{ cursor: isDragging ? "grabbing" : hoveredNode ? "pointer" : "grab", width: "100%", height: "auto" }}
        />
      </div>

      {/* Selected node details */}
      {selectedNode && (
        <div
          style={{
            marginTop: "var(--spacing-3)",
            padding: "var(--spacing-3)",
            backgroundColor: "var(--card)",
            border: "1px solid var(--border)",
            borderRadius: "var(--radius-md)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)", marginBottom: "var(--spacing-2)" }}>
            <span
              style={{
                width: 12,
                height: 12,
                borderRadius: "50%",
                backgroundColor: ENTITY_COLORS[selectedNode.type.toLowerCase()] || ENTITY_COLORS.default,
              }}
            />
            <span style={{ fontWeight: 500 }}>{selectedNode.name}</span>
            <span
              style={{
                fontSize: "var(--text-xs)",
                padding: "2px var(--spacing-2)",
                backgroundColor: "var(--muted)",
                borderRadius: "var(--radius-sm)",
                color: "var(--muted-foreground)",
              }}
            >
              {selectedNode.type}
            </span>
            <span style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)" }}>
              {selectedNode.connections} connections
            </span>
          </div>

          {/* Show connected entities */}
          <div style={{ fontSize: "var(--text-sm)" }}>
            <strong style={{ color: "var(--muted-foreground)" }}>Connected to:</strong>
            <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--spacing-1)", marginTop: "var(--spacing-1)" }}>
              {edges
                .filter((e) => e.source === selectedNode.id || e.target === selectedNode.id)
                .slice(0, 10)
                .map((edge, i) => {
                  const connectedId = edge.source === selectedNode.id ? edge.target : edge.source;
                  const connected = nodes.find((n) => n.id === connectedId);
                  return connected ? (
                    <span
                      key={i}
                      style={{
                        padding: "2px var(--spacing-2)",
                        backgroundColor: "var(--muted)",
                        borderRadius: "var(--radius-sm)",
                        fontSize: "var(--text-xs)",
                      }}
                    >
                      {edge.type} → {connected.name}
                    </span>
                  ) : null;
                })}
            </div>
          </div>
        </div>
      )}

      {/* Legend */}
      <div
        style={{
          marginTop: "var(--spacing-3)",
          display: "flex",
          flexWrap: "wrap",
          gap: "var(--spacing-3)",
          fontSize: "var(--text-xs)",
          color: "var(--muted-foreground)",
        }}
      >
        {Object.entries(ENTITY_COLORS)
          .filter(([key]) => key !== "default")
          .map(([type, color]) => (
            <span key={type} style={{ display: "flex", alignItems: "center", gap: "var(--spacing-1)" }}>
              <span style={{ width: 10, height: 10, borderRadius: "50%", backgroundColor: color }} />
              {type}
            </span>
          ))}
      </div>
    </div>
  );
}
