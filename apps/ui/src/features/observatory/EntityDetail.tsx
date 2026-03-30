// ============================================================================
// ENTITY DETAIL — Sidebar panel for selected graph entity
// ============================================================================

import { X, Loader2 } from "lucide-react";
import type { GraphEntity } from "@/services/transport/types";
import { useEntityConnections } from "./graph-hooks";

// ============================================================================
// Types
// ============================================================================

interface EntityDetailProps {
  entity: GraphEntity | null;
  onClose: () => void;
}

// ============================================================================
// Helpers
// ============================================================================

function formatDate(iso: string | undefined | null): string {
  if (!iso) return "--";
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso;
  }
}

// ============================================================================
// Component
// ============================================================================

export function EntityDetail({ entity, onClose }: EntityDetailProps) {
  const { data: connections, loading } = useEntityConnections(
    entity?.agent_id ?? "",
    entity?.id ?? ""
  );

  if (!entity) return null;

  const props = entity.properties as Record<string, unknown> | undefined;
  const firstSeen = entity.first_seen_at;
  const lastSeen = entity.last_seen_at;

  return (
    <div className="observatory__sidebar">
      {/* Header */}
      <div className="observatory__sidebar-header">
        <div>
          <div className="observatory__sidebar-title">{entity.name}</div>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)", marginTop: "var(--spacing-1)" }}>
            <span className="badge">{entity.entity_type}</span>
            <span style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)" }}>
              {entity.mention_count} mention{entity.mention_count !== 1 ? "s" : ""}
            </span>
          </div>
        </div>
        <button className="slideover__close" onClick={onClose}>
          <X style={{ width: 14, height: 14 }} />
        </button>
      </div>

      {/* Connections */}
      <div className="observatory__sidebar-section">
        <div className="observatory__sidebar-label">Connections</div>
        {loading ? (
          <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: "var(--text-sm)" }}>
            <Loader2 style={{ width: 14, height: 14, animation: "spin 1s linear infinite" }} />
            Loading...
          </div>
        ) : connections && connections.neighbors.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)" }}>
            {connections.neighbors.map((n, i) => (
              <div key={i} className="observatory__connection">
                <span className="observatory__connection-type">
                  {n.relationship.relationship_type}
                </span>
                <span className="observatory__connection-arrow">
                  {n.direction === "outgoing" ? "\u2192" : "\u2190"}
                </span>
                <span>{n.entity.name}</span>
                <span className="badge" style={{ marginLeft: "auto" }}>
                  {n.entity.entity_type}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p style={{ fontSize: "var(--text-sm)", color: "var(--dim-foreground)" }}>
            No connections found.
          </p>
        )}
      </div>

      {/* Timeline */}
      <div className="observatory__sidebar-section">
        <div className="observatory__sidebar-label">Timeline</div>
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)", fontSize: "var(--text-sm)" }}>
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: "var(--muted-foreground)" }}>First seen</span>
            <span style={{ color: "var(--foreground)" }}>{formatDate(firstSeen)}</span>
          </div>
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: "var(--muted-foreground)" }}>Last seen</span>
            <span style={{ color: "var(--foreground)" }}>{formatDate(lastSeen)}</span>
          </div>
        </div>
      </div>

      {/* Properties */}
      {props && Object.keys(props).length > 0 && (
        <div className="observatory__sidebar-section">
          <div className="observatory__sidebar-label">Properties</div>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-1)", fontSize: "var(--text-xs)" }}>
            {Object.entries(props)
              .filter(([k]) => k !== "first_seen_at" && k !== "last_seen_at")
              .slice(0, 10)
              .map(([key, value]) => (
                <div key={key} style={{ display: "flex", justifyContent: "space-between", gap: "var(--spacing-2)" }}>
                  <span style={{ color: "var(--dim-foreground)", fontWeight: 500 }}>{key}</span>
                  <span style={{ color: "var(--muted-foreground)", textAlign: "right", wordBreak: "break-all" }}>
                    {String(value)}
                  </span>
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  );
}
