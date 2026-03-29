// ============================================================================
// OBSERVATORY PAGE — Knowledge graph visualization
// ============================================================================

import { useState, useEffect } from "react";
import { Loader2, Search, Network } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { AgentResponse, GraphEntity } from "@/services/transport/types";
import { useGraphData } from "./graph-hooks";
import { GraphCanvas } from "./GraphCanvas";
import { EntityDetail } from "./EntityDetail";
import { LearningHealthBar } from "./LearningHealthBar";

// ============================================================================
// Component
// ============================================================================

export function ObservatoryPage() {
  const [agents, setAgents] = useState<AgentResponse[]>([]);
  const [agentFilter, setAgentFilter] = useState<string | undefined>(undefined);
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedEntity, setSelectedEntity] = useState<GraphEntity | null>(null);

  // Load agents for filter pills
  useEffect(() => {
    const load = async () => {
      try {
        const transport = await getTransport();
        const res = await transport.listAgents();
        if (res.success && res.data) {
          setAgents(res.data);
        }
      } catch {
        // swallow — agent list is optional
      }
    };
    load();
  }, []);

  // Graph data from hook
  const { entities, relationships, loading, error, refetch } = useGraphData(agentFilter);

  const handleEntitySelect = (entity: GraphEntity) => {
    setSelectedEntity(entity);
  };

  const handleCloseDetail = () => {
    setSelectedEntity(null);
  };

  return (
    <div className="observatory">
      {/* Toolbar */}
      <div className="observatory__toolbar">
        <div className="observatory__toolbar-left">
          {/* All agents pill */}
          <button
            className={`filter-chip ${!agentFilter ? "filter-chip--active" : ""}`}
            onClick={() => {
              setAgentFilter(undefined);
              setSelectedEntity(null);
            }}
          >
            All
          </button>
          {/* Per-agent pills */}
          {agents.map((a) => (
            <button
              key={a.id}
              className={`filter-chip ${agentFilter === a.id ? "filter-chip--active" : ""}`}
              onClick={() => {
                setAgentFilter(a.id);
                setSelectedEntity(null);
              }}
            >
              {a.name || a.id}
            </button>
          ))}
        </div>

        <div className="observatory__toolbar-right">
          {/* Search */}
          <div className="action-bar__search">
            <Search
              className="action-bar__search-icon"
              style={{ width: 14, height: 14 }}
            />
            <input
              type="text"
              className="action-bar__search-input"
              placeholder="Highlight entities..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>
          <button
            className="btn btn--ghost btn--sm"
            onClick={refetch}
            title="Refresh"
          >
            Refresh
          </button>
        </div>
      </div>

      {/* Main area */}
      <div className="observatory__main">
        {loading ? (
          <div
            className="observatory__canvas"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <div style={{ textAlign: "center", color: "var(--muted-foreground)" }}>
              <Loader2
                style={{
                  width: 24,
                  height: 24,
                  animation: "spin 1s linear infinite",
                  marginBottom: "var(--spacing-2)",
                }}
              />
              <p style={{ fontSize: "var(--text-sm)" }}>Loading knowledge graph...</p>
            </div>
          </div>
        ) : error ? (
          <div
            className="observatory__canvas"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <div style={{ textAlign: "center" }}>
              <p style={{ color: "var(--destructive)", fontSize: "var(--text-sm)" }}>{error}</p>
              <button className="btn btn--ghost btn--sm" onClick={refetch} style={{ marginTop: "var(--spacing-2)" }}>
                Retry
              </button>
            </div>
          </div>
        ) : entities.length === 0 ? (
          <div
            className="observatory__canvas"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <div style={{ textAlign: "center", color: "var(--muted-foreground)" }}>
              <Network style={{ width: 48, height: 48, opacity: 0.5, marginBottom: "var(--spacing-3)" }} />
              <p style={{ fontSize: "var(--text-lg)", fontWeight: 500 }}>No knowledge graph data</p>
              <p style={{ fontSize: "var(--text-sm)", marginTop: "var(--spacing-1)" }}>
                Entities and relationships appear here after conversations are distilled.
              </p>
            </div>
          </div>
        ) : (
          <GraphCanvas
            entities={entities}
            relationships={relationships}
            selectedEntityId={selectedEntity?.id}
            highlightTerm={searchTerm}
            onEntitySelect={handleEntitySelect}
          />
        )}

        {/* Entity detail sidebar */}
        {selectedEntity && (
          <EntityDetail entity={selectedEntity} onClose={handleCloseDetail} />
        )}
      </div>

      {/* Health bar */}
      <LearningHealthBar />
    </div>
  );
}
