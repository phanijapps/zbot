import { useState, useEffect, useCallback } from "react";
import { getTransport } from "@/services/transport";
import type {
  MemoryFact,
  MemoryFilter,
  MemoryCategory,
} from "@/services/transport/types";
import { MemoryFactCard } from "./MemoryFactCard";

const CATEGORIES: MemoryCategory[] = [
  "preference",
  "decision",
  "pattern",
  "entity",
  "instruction",
  "correction",
];

interface MemoryTabProps {
  agentId: string;
}

export function MemoryTab({ agentId }: MemoryTabProps) {
  const [facts, setFacts] = useState<MemoryFact[]>([]);
  const [filter, setFilter] = useState<MemoryFilter>({});
  const [searchQuery, setSearchQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [total, setTotal] = useState(0);

  const loadFacts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const response = await transport.listMemory(agentId, filter);
      if (response.success && response.data) {
        setFacts(response.data.facts);
        setTotal(response.data.total);
      } else {
        setError(response.error || "Failed to load memories");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load memories");
    } finally {
      setLoading(false);
    }
  }, [agentId, filter]);

  useEffect(() => {
    loadFacts();
  }, [loadFacts]);

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      loadFacts();
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const response = await transport.searchMemory(agentId, searchQuery, filter);
      if (response.success && response.data) {
        setFacts(response.data.facts);
        setTotal(response.data.total);
      } else {
        setError(response.error || "Search failed");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (factId: string) => {
    try {
      const transport = await getTransport();
      const response = await transport.deleteMemory(agentId, factId);
      if (response.success) {
        setFacts((prev) => prev.filter((f) => f.id !== factId));
        setTotal((prev) => Math.max(0, prev - 1));
      } else {
        setError(response.error || "Failed to delete memory");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete memory");
    }
  };

  const toggleCategory = (category: MemoryCategory) => {
    setFilter((prev) => ({
      ...prev,
      category: prev.category === category ? undefined : category,
    }));
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Filter chips */}
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "var(--spacing-1)",
          padding: "var(--spacing-3)",
          borderBottom: "1px solid var(--border)",
        }}
      >
        {CATEGORIES.map((cat) => (
          <button
            key={cat}
            onClick={() => toggleCategory(cat)}
            style={{
              fontSize: "var(--text-xs)",
              padding: "4px var(--spacing-2)",
              borderRadius: "var(--radius-full)",
              border: "1px solid",
              transition: "all 0.15s ease",
              cursor: "pointer",
              backgroundColor: filter.category === cat ? "var(--primary-muted)" : "transparent",
              borderColor: filter.category === cat ? "var(--primary)" : "var(--border)",
              color: filter.category === cat ? "var(--primary)" : "var(--muted-foreground)",
            }}
          >
            {cat}
          </button>
        ))}
      </div>

      {/* Search bar */}
      <div
        style={{
          display: "flex",
          gap: "var(--spacing-2)",
          padding: "var(--spacing-3)",
          borderBottom: "1px solid var(--border)",
        }}
      >
        <input
          type="text"
          placeholder="Search memories..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
          className="form-input"
          style={{ flex: 1 }}
        />
        <button onClick={handleSearch} className="btn btn--secondary btn--sm">
          Search
        </button>
        {searchQuery && (
          <button
            onClick={() => {
              setSearchQuery("");
              loadFacts();
            }}
            className="btn btn--ghost btn--sm"
          >
            Clear
          </button>
        )}
      </div>

      {/* Error message */}
      {error && (
        <div
          style={{
            margin: "var(--spacing-3)",
            padding: "var(--spacing-2)",
            backgroundColor: "var(--destructive-muted)",
            border: "1px solid var(--destructive)",
            borderRadius: "var(--radius-md)",
            color: "var(--destructive)",
            fontSize: "var(--text-sm)",
          }}
        >
          {error}
        </div>
      )}

      {/* Facts list */}
      <div style={{ flex: 1, overflow: "auto", padding: "var(--spacing-3)" }}>
        {loading ? (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: 128,
              color: "var(--muted-foreground)",
            }}
          >
            Loading memories...
          </div>
        ) : facts.length === 0 ? (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              height: 128,
              color: "var(--muted-foreground)",
            }}
          >
            <p>No memories found</p>
            {searchQuery && (
              <p style={{ fontSize: "var(--text-sm)", marginTop: "var(--spacing-1)" }}>
                Try a different search term
              </p>
            )}
          </div>
        ) : (
          <>
            <div
              style={{
                fontSize: "var(--text-xs)",
                color: "var(--muted-foreground)",
                marginBottom: "var(--spacing-2)",
              }}
            >
              {total} {total === 1 ? "memory" : "memories"}
              {searchQuery && ` matching "${searchQuery}"`}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-2)" }}>
              {facts.map((fact) => (
                <MemoryFactCard
                  key={fact.id}
                  fact={fact}
                  onDelete={() => handleDelete(fact.id)}
                />
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
