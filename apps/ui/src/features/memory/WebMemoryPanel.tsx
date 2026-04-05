import { useState, useEffect, useCallback } from "react";
import { getTransport } from "@/services/transport";
import type {
  MemoryFact,
  MemoryFilter,
  MemoryCategory,
  MemoryScope,
  AgentResponse,
} from "@/services/transport/types";
import { MemoryFactCard } from "./MemoryFactCard";
import { Loader2, Database, Plus, Shield, Lightbulb, User } from "lucide-react";

const CATEGORIES: MemoryCategory[] = [
  "preference",
  "decision",
  "pattern",
  "entity",
  "instruction",
  "correction",
];

const SCOPES: MemoryScope[] = ["agent", "shared", "ward"];

export function WebMemoryPanel() {
  const [agents, setAgents] = useState<AgentResponse[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState<string>("");
  const [facts, setFacts] = useState<MemoryFact[]>([]);
  const [filter, setFilter] = useState<MemoryFilter>({ limit: 50, offset: 0 });
  const [searchQuery, setSearchQuery] = useState("");
  const [stats, setStats] = useState<Record<string, number>>({});
  const [loading, setLoading] = useState(true);
  const [agentsLoading, setAgentsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [addType, setAddType] = useState<"correction" | "instruction" | "user">("correction");
  const [addContent, setAddContent] = useState("");
  const [addSaving, setAddSaving] = useState(false);

  // Load ALL facts with optional agent filter
  const fetchFacts = useCallback(async (filterParams: MemoryFilter) => {
    setLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const effectiveFilter: MemoryFilter = {
        ...filterParams,
        // Include agent_id in filter if selected
        agent_id: selectedAgentId || undefined,
      };
      const response = await transport.listAllMemory(effectiveFilter);
      if (response.success && response.data) {
        setFacts(response.data.facts);
      } else {
        setError(response.error || "Failed to load memories");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load memories");
    } finally {
      setLoading(false);
    }
  }, [selectedAgentId]);

  // Load stats from all memories
  const fetchStats = useCallback(async () => {
    try {
      const transport = await getTransport();
      // Fetch all memories for stats computation (no agent filter)
      const response = await transport.listAllMemory({ limit: 1000 });
      if (response.success && response.data) {
        const categoryStats: Record<string, number> = {};
        response.data.facts.forEach((f) => {
          categoryStats[f.category] = (categoryStats[f.category] || 0) + 1;
        });
        setStats(categoryStats);
      }
    } catch (err) {
      console.error("Failed to load stats:", err);
    }
  }, []);

  // Load agents on mount
  useEffect(() => {
    const loadAgents = async () => {
      setAgentsLoading(true);
      try {
        const transport = await getTransport();
        const response = await transport.listAgents();
        if (response.success && response.data) {
          setAgents(response.data);
          // Don't auto-select any agent - show all by default
        }
      } catch (err) {
        console.error("Failed to load agents:", err);
      } finally {
        setAgentsLoading(false);
      }
    };
    loadAgents();
  }, []);

  // Load facts on mount and when filter changes
  useEffect(() => {
    fetchFacts(filter);
    fetchStats();
  }, [filter, fetchFacts, fetchStats]);

  // Reload facts when agent selection changes
  useEffect(() => {
    fetchFacts(filter);
  }, [selectedAgentId, filter, fetchFacts]);

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      fetchFacts(filter);
      return;
    }
    // Search requires agent ID - use selected agent or first available
    const searchAgentId = selectedAgentId || (agents.length > 0 ? agents[0].id : "");
    if (!searchAgentId) {
      setError("Select an agent to search memories");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const response = await transport.searchMemory(searchAgentId, searchQuery, {
        category: filter.category,
        limit: filter.limit,
      });
      if (response.success && response.data) {
        setFacts(response.data.facts);
      } else {
        setError(response.error || "Search failed");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (fact: MemoryFact) => {
    try {
      const transport = await getTransport();
      const response = await transport.deleteMemory(fact.agent_id, fact.id);
      if (response.success) {
        setFacts((prev) => prev.filter((f) => f.id !== fact.id));
        fetchStats(); // Refresh stats
      } else {
        setError(response.error || "Failed to delete memory");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete memory");
    }
  };

  const handleCreate = async () => {
    if (!addContent.trim()) return;
    setAddSaving(true);
    try {
      const transport = await getTransport();
      const typeConfig = {
        correction: { confidence: 1.0, prefix: "policy" },
        instruction: { confidence: 0.9, prefix: "instruction" },
        user: { confidence: 0.95, prefix: "user.profile" },
      }[addType];
      const key = `${typeConfig.prefix}.${Date.now()}`;
      const result = await transport.createMemory("root", {
        category: addType,
        key,
        content: addContent.trim(),
        confidence: typeConfig.confidence,
        pinned: true,
      });
      if (result.success) {
        setAddContent("");
        setShowAddForm(false);
        fetchFacts(filter);
        fetchStats();
      } else {
        setError(result.error || "Failed to create");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create");
    } finally {
      setAddSaving(false);
    }
  };

  // Group facts by agent for display
  const agentMap = new Map(agents.map(a => [a.id, a.name || a.id]));

  return (
    <div className="page">
      <div className="page-container">
        {/* Header */}
        <div className="page-header">
          <div>
            <h1 className="page-title">Memory Explorer</h1>
            <p className="page-subtitle">View and manage agent memory facts</p>
          </div>
          <button className="btn btn--primary btn--sm" onClick={() => setShowAddForm(!showAddForm)}>
            <Plus size={14} /> Add
          </button>
        </div>

        {/* Add Form */}
        {showAddForm && (
          <div className="card card__padding--lg" style={{ marginBottom: "var(--spacing-4)" }}>
            <div className="flex gap-3" style={{ marginBottom: "var(--spacing-3)" }}>
              <button
                className={`btn btn--sm ${addType === "correction" ? "btn--primary" : "btn--outline"}`}
                onClick={() => setAddType("correction")}
              >
                <Shield size={14} /> Policy
              </button>
              <button
                className={`btn btn--sm ${addType === "instruction" ? "btn--primary" : "btn--outline"}`}
                onClick={() => setAddType("instruction")}
              >
                <Lightbulb size={14} /> Instruction
              </button>
              <button
                className={`btn btn--sm ${addType === "user" ? "btn--primary" : "btn--outline"}`}
                onClick={() => setAddType("user")}
              >
                <User size={14} /> About Me
              </button>
            </div>
            <textarea
              className="form-input"
              rows={3}
              placeholder={
                addType === "correction"
                  ? "Add a rule agents MUST follow (e.g., 'Always use research-agent for factual data')"
                  : addType === "instruction"
                  ? "Add a preference or guideline (e.g., 'Prefer interactive HTML outputs')"
                  : "Tell z-Bot about yourself (e.g., 'I have a 9th grade son with ADHD')"
              }
              value={addContent}
              onChange={(e) => setAddContent(e.target.value)}
            />
            <div className="flex gap-2" style={{ marginTop: "var(--spacing-2)" }}>
              <button className="btn btn--primary btn--sm" onClick={handleCreate} disabled={!addContent.trim() || addSaving}>
                {addSaving ? "Saving..." : "Save"}
              </button>
              <button className="btn btn--ghost btn--sm" onClick={() => { setShowAddForm(false); setAddContent(""); }}>
                Cancel
              </button>
              <span className="settings-hint" style={{ marginLeft: "auto" }}>
                {addType === "correction" ? "📛 Highest priority — always recalled first"
                  : addType === "instruction" ? "💡 Guides behavior — recalled as preferences"
                  : "👤 Personal context — agents personalize their work"}
              </span>
            </div>
          </div>
        )}

        {/* Agent selector and stats */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--spacing-4)",
            marginBottom: "var(--spacing-5)",
            padding: "var(--spacing-3) var(--spacing-4)",
            backgroundColor: "var(--card)",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
            <label style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
              Filter by Agent:
            </label>
            {agentsLoading ? (
              <Loader2 style={{ width: 16, height: 16, animation: "spin 1s linear infinite", color: "var(--muted-foreground)" }} />
            ) : (
              <select
                value={selectedAgentId}
                onChange={(e) => {
                  setSelectedAgentId(e.target.value);
                  setFilter({ limit: 50, offset: 0 });
                  setSearchQuery("");
                }}
                className="form-select"
                style={{ minWidth: 150 }}
              >
                <option value="">All Agents</option>
                {agents.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name || a.id}
                  </option>
                ))}
              </select>
            )}
          </div>

          {/* Stats bar */}
          <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)", flexWrap: "wrap" }}>
            {Object.entries(stats).map(([cat, count]) => (
              <div
                key={cat}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--spacing-1)",
                  padding: "2px var(--spacing-2)",
                  backgroundColor: "var(--muted)",
                  borderRadius: "var(--radius-sm)",
                  fontSize: "var(--text-xs)",
                }}
              >
                <span style={{ color: "var(--muted-foreground)" }}>{cat}:</span>
                <span style={{ fontWeight: 500 }}>{count}</span>
              </div>
            ))}
            {Object.keys(stats).length === 0 && !loading && (
              <span style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)" }}>
                No memories yet
              </span>
            )}
          </div>
        </div>

        {/* Filter + Search */}
        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "var(--spacing-3)", marginBottom: "var(--spacing-4)" }}>
          <select
            value={filter.category || ""}
            onChange={(e) =>
              setFilter((f) => ({
                ...f,
                category: (e.target.value as MemoryCategory) || undefined,
                offset: 0,
              }))
            }
            className="form-select"
          >
            <option value="">All Categories</option>
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>

          <select
            value={filter.scope || ""}
            onChange={(e) =>
              setFilter((f) => ({
                ...f,
                scope: (e.target.value as MemoryScope) || undefined,
                offset: 0,
              }))
            }
            className="form-select"
          >
            <option value="">All Scopes</option>
            {SCOPES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>

          <div style={{ flex: 1, display: "flex", gap: "var(--spacing-2)", minWidth: 200 }}>
            <input
              type="text"
              placeholder="Search memories (requires agent filter)..."
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
                  fetchFacts(filter);
                }}
                className="btn btn--ghost btn--sm"
              >
                Clear
              </button>
            )}
          </div>
        </div>

        {/* Error message */}
        {error && (
          <div
            style={{
              marginBottom: "var(--spacing-4)",
              padding: "var(--spacing-3)",
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
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--spacing-3)" }}>
          {loading ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "var(--spacing-12) 0",
                color: "var(--muted-foreground)",
              }}
            >
              <Loader2 style={{ width: 24, height: 24, marginRight: "var(--spacing-2)", animation: "spin 1s linear infinite" }} />
              Loading memories...
            </div>
          ) : facts.length === 0 ? (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                padding: "var(--spacing-12) 0",
                color: "var(--muted-foreground)",
              }}
            >
              <Database style={{ width: 48, height: 48, marginBottom: "var(--spacing-3)", opacity: 0.5 }} />
              <p style={{ fontSize: "var(--text-lg)", fontWeight: 500 }}>No memories found</p>
              {searchQuery && (
                <p style={{ fontSize: "var(--text-sm)", marginTop: "var(--spacing-1)" }}>
                  Try a different search term or clear filters
                </p>
              )}
            </div>
          ) : (
            <>
              <div style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)", marginBottom: "var(--spacing-2)" }}>
                {facts.length} {facts.length === 1 ? "memory" : "memories"}
                {selectedAgentId && ` from ${agentMap.get(selectedAgentId) || selectedAgentId}`}
                {searchQuery && ` matching "${searchQuery}"`}
                {!selectedAgentId && " across all agents"}
              </div>
              {facts.map((fact) => (
                <div
                  key={fact.id}
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "var(--spacing-1)",
                  }}
                >
                  {/* Show agent badge when viewing all agents */}
                  {!selectedAgentId && (
                    <div
                      style={{
                        fontSize: "var(--text-xs)",
                        color: "var(--muted-foreground)",
                        padding: "2px var(--spacing-2)",
                        backgroundColor: "var(--muted)",
                        borderRadius: "var(--radius-sm)",
                        width: "fit-content",
                      }}
                    >
                      {agentMap.get(fact.agent_id) || fact.agent_id}
                    </div>
                  )}
                  <MemoryFactCard
                    fact={fact}
                    onDelete={() => handleDelete(fact)}
                    expanded
                  />
                </div>
              ))}
            </>
          )}
        </div>

        {/* Pagination */}
        {!loading && facts.length > 0 && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "var(--spacing-4)",
              marginTop: "var(--spacing-6)",
              paddingTop: "var(--spacing-4)",
              borderTop: "1px solid var(--border)",
            }}
          >
            <button
              onClick={() =>
                setFilter((f) => ({
                  ...f,
                  offset: Math.max(0, (f.offset || 0) - 50),
                }))
              }
              disabled={(filter.offset || 0) === 0}
              className="btn btn--ghost btn--sm"
            >
              Previous
            </button>
            <span style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)" }}>
              {(filter.offset || 0) + 1} - {(filter.offset || 0) + facts.length}
            </span>
            <button
              onClick={() =>
                setFilter((f) => ({ ...f, offset: (f.offset || 0) + 50 }))
              }
              disabled={facts.length < 50}
              className="btn btn--ghost btn--sm"
            >
              Next
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
