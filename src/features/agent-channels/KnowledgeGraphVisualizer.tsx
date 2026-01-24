// ============================================================================
// KNOWLEDGE GRAPH VISUALIZER
// Interactive graph visualization for knowledge graph data
// ============================================================================

import { useState, useEffect, useCallback, useMemo } from "react";
import { X, Search, Filter, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import ReactFlow, {
  Node,
  Edge,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  BackgroundVariant,
  MarkerType,
} from "reactflow";
import dagre from "dagre";
import "reactflow/dist/style.css";

// Conditional logging
const isDev = import.meta.env.DEV;
const debugLog = (...args: unknown[]) => {
  if (isDev) {
    console.log("[KnowledgeGraphVisualizer]", ...args);
  }
};

// ============================================================================
// TYPES
// ============================================================================

interface KnowledgeGraphData {
  entities: Entity[];
  relationships: Relationship[];
}

interface Entity {
  id: string;
  agent_id: string;
  entity_type: string;
  name: string;
  properties: Record<string, unknown>;
  first_seen_at: string;
  last_seen_at: string;
  mention_count: number;
}

interface Relationship {
  id: string;
  agent_id: string;
  source_entity_id: string;
  target_entity_id: string;
  relationship_type: string;
  properties: Record<string, unknown>;
  first_seen_at: string;
  last_seen_at: string;
  mention_count: number;
}

interface KnowledgeGraphVisualizerProps {
  agentId: string;
  agentName: string;
  onClose: () => void;
}

// ============================================================================
// CUSTOM NODE COMPONENT (TEMP: Commented out to test with default nodes)
// ============================================================================

/*
interface EntityNodeData {
  label: string;
  entityType: string;
  mentionCount: number;
  properties: Record<string, unknown>;
}

const EntityNode = ({ data }: { data: EntityNodeData }) => {
  const getEntityColor = (entityType: string) => {
    const colors: Record<string, string> = {
      person: "bg-violet-500 border-violet-600",
      organization: "bg-blue-500 border-blue-600",
      location: "bg-green-500 border-green-600",
      concept: "bg-yellow-500 border-yellow-600",
      tool: "bg-orange-500 border-orange-600",
      project: "bg-pink-500 border-pink-600",
    };
    return colors[entityType.toLowerCase()] || "bg-gray-500 border-gray-600";
  };

  const colorClass = getEntityColor(data.entityType);

  return (
    <div className={`px-3 py-2 rounded-lg border-2 shadow-lg ${colorClass} text-white min-w-[120px]`}>
      <Handle type="target" position={Position.Left} />
      <div className="text-xs uppercase opacity-70 mb-1">{data.entityType}</div>
      <div className="font-semibold text-sm">{data.label}</div>
      {data.mentionCount > 1 && (
        <div className="text-xs opacity-70 mt-1">{data.mentionCount} mentions</div>
      )}
      <Handle type="source" position={Position.Right} />
    </div>
  );
};
*/

// ============================================================================
// LAYOUT FUNCTION
// ============================================================================

/**
 * Layout nodes and edges using dagre (directed graph layout algorithm)
 */
function layoutNodesAndEdges(nodes: Node[], edges: Edge[]): { nodes: Node[]; edges: Edge[] } {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: "LR", nodesep: 100, ranksep: 150 });

  // Add nodes to dagre graph
  nodes.forEach((node) => {
    dagreGraph.setNode(node.id, { width: 150, height: 80 });
  });

  // Add edges to dagre graph
  edges.forEach((edge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  // Calculate layout
  dagre.layout(dagreGraph);

  // Apply calculated positions to nodes
  const layoutedNodes = nodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      position: {
        x: nodeWithPosition.x - 75, // Center the node (width/2)
        y: nodeWithPosition.y - 40, // Center the node (height/2)
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

// ============================================================================
// MAIN COMPONENT
// ============================================================================

export function KnowledgeGraphVisualizer({
  agentId,
  agentName,
  onClose,
}: KnowledgeGraphVisualizerProps) {
  const [graphData, setGraphData] = useState<KnowledgeGraphData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedEntityType, setSelectedEntityType] = useState<string | null>(null);

  // React Flow state
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);

  // Load graph data
  const loadGraphData = useCallback(async () => {
    debugLog("Loading knowledge graph for agent:", agentId);
    setIsLoading(true);
    setError(null);

    try {
      const data = await invoke<KnowledgeGraphData>("get_knowledge_graph", {
        agentId,
      });
      setGraphData(data);
      debugLog("Loaded graph data:", data.entities.length, "entities,", data.relationships.length, "relationships");
    } catch (e) {
      console.error("Failed to load knowledge graph:", e);
      setError(e as string);
    } finally {
      setIsLoading(false);
    }
  }, [agentId]);

  useEffect(() => {
    loadGraphData();
  }, [loadGraphData]);

  // Convert graph data to React Flow format
  useEffect(() => {
    if (!graphData) return;

    // Filter entities by search query and entity type
    const filteredEntities = graphData.entities.filter((entity) => {
      const matchesSearch =
        !searchQuery ||
        entity.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        entity.entity_type.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesType =
        !selectedEntityType || entity.entity_type === selectedEntityType;

      return matchesSearch && matchesType;
    });

    // Get the IDs of filtered entities
    const filteredEntityIds = new Set(filteredEntities.map((e) => e.id));

    // Filter relationships to only include filtered entities
    const filteredRelationships = graphData.relationships.filter((rel) =>
      filteredEntityIds.has(rel.source_entity_id) && filteredEntityIds.has(rel.target_entity_id)
    );

    debugLog("Filtered relationships:", filteredRelationships.length, "out of", graphData.relationships.length);
    if (filteredRelationships.length > 0) {
      debugLog("Sample relationship:", filteredRelationships[0]);
    }

    // Create nodes
    const newNodes: Node[] = filteredEntities.map((entity) => ({
      id: entity.id,
      // TEMP: Use default node type to test edges
      // type: "entity",
      position: { x: 0, y: 0 }, // Will be laid out automatically
      data: {
        label: `${entity.name} (${entity.entity_type})`,
      },
      style: {
        background: getEntityColorHex(entity.entity_type),
        color: 'white',
        border: '2px solid white',
        borderRadius: '8px',
        padding: '10px',
        width: '150px',
      },
    }));

    function getEntityColorHex(entityType: string): string {
      const colors: Record<string, string> = {
        person: "#8b5cf6",
        organization: "#3b82f6",
        location: "#22c55e",
        concept: "#eab308",
        tool: "#f97316",
        project: "#ec4899",
      };
      return colors[entityType.toLowerCase()] || "#6b7280";
    }

    // Create edges
    const newEdges: Edge[] = filteredRelationships.map((rel) => {
      debugLog("Creating edge:", rel.id, "from", rel.source_entity_id, "to", rel.target_entity_id, "type:", rel.relationship_type);
      return {
      id: rel.id,
      source: rel.source_entity_id,
      target: rel.target_entity_id,
      label: rel.relationship_type,
      animated: rel.mention_count > 1,
      markerEnd: {
        type: MarkerType.ArrowClosed,
        color: rel.mention_count > 1 ? "#a78bfa" : "#6b7280",
      },
      style: { stroke: rel.mention_count > 1 ? "#a78bfa" : "#6b7280", strokeWidth: rel.mention_count > 1 ? 2 : 1 },
      labelStyle: { fill: "#9ca3af", fontSize: 10 },
      labelShowBg: true,
      labelBgStyle: { fill: "#1f2937", fillOpacity: 0.8 },
    };
    });

    debugLog("Total edges created:", newEdges.length);

    // Apply dagre layout to position nodes
    const { nodes: layoutedNodes, edges: layoutedEdges } = layoutNodesAndEdges(newNodes, newEdges);

    setNodes(layoutedNodes);
    setEdges(layoutedEdges);
  }, [graphData, searchQuery, selectedEntityType, setNodes, setEdges]);

  // Get unique entity types for filter dropdown
  const entityTypes = useMemo(() => {
    if (!graphData) return [];
    const types = new Set(graphData.entities.map((e) => e.entity_type));
    return Array.from(types).sort();
  }, [graphData]);

  return (
    <div className="fixed inset-0 bg-[#0a0a0a] z-50 flex flex-col">
      {/* Header */}
      <div className="shrink-0 border-b border-white/10 bg-[#0f0f0f] px-4 py-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
              <h1 className="text-lg font-semibold text-white">Knowledge Graph</h1>
              <span className="text-sm text-gray-400">{agentName}</span>
            </div>
            {graphData && (
              <div className="text-sm text-gray-400">
                {graphData.entities.length} entities, {graphData.relationships.length} relationships
              </div>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={loadGraphData}
              className="p-2 text-gray-300 hover:text-white transition-colors rounded hover:bg-white/5"
              aria-label="Refresh"
              title="Refresh"
            >
              <RefreshCw className="size-5" />
            </button>
            <button
              onClick={onClose}
              className="p-2 text-gray-300 hover:text-white transition-colors rounded hover:bg-white/5"
              aria-label="Close"
            >
              <X className="size-5" />
            </button>
          </div>
        </div>

        {/* Search and Filter Bar */}
        <div className="flex items-center gap-3 mt-3">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-gray-400" />
            <input
              type="text"
              placeholder="Search entities or relationships..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-10 pr-4 py-2 bg-white/5 border border-white/10 rounded-lg text-white text-sm placeholder:text-gray-500 focus:outline-none focus:border-violet-500"
            />
          </div>
          <div className="flex items-center gap-2 min-w-[150px]">
            <Filter className="size-4 text-gray-400" />
            <Select value={selectedEntityType || "all"} onValueChange={(value) => setSelectedEntityType(value === "all" ? null : value)}>
              <SelectTrigger className="w-full">
                <SelectValue placeholder="All Types" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Types</SelectItem>
                {entityTypes.map((type: string) => (
                  <SelectItem key={type} value={type}>
                    {type}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </div>

      {/* Graph Area */}
      <div className="flex-1 relative">
        {isLoading ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <RefreshCw className="size-8 text-violet-400 animate-spin mx-auto mb-3" />
              <p className="text-gray-400">Loading knowledge graph...</p>
            </div>
          </div>
        ) : error ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <p className="text-red-400 mb-2">Failed to load knowledge graph</p>
              <p className="text-gray-400 text-sm">{error}</p>
            </div>
          </div>
        ) : nodes.length === 0 ? (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center">
              <p className="text-gray-400 mb-2">No knowledge graph data</p>
              <p className="text-gray-500 text-sm">
                Start a conversation with {agentName} to build the knowledge graph
              </p>
            </div>
          </div>
        ) : (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            // TEMP: Using default nodes to test edges
            // nodeTypes={nodeTypes}
            fitView
            className="bg-[#0a0a0a]"
          >
            <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="#ffffff20" />
            <Controls />
            <MiniMap
              nodeColor={() => {
                // TEMP: Simplified for default nodes
                return "#8b5cf6";
              }}
              className="!bg-[#1a1a1a] !border border-white/10"
            />
          </ReactFlow>
        )}
      </div>
    </div>
  );
}
