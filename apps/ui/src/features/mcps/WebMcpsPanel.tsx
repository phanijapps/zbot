// ============================================================================
// WEB MCPS PANEL
// MCP server management for web dashboard (uses transport layer)
// ============================================================================

import { useState, useEffect } from "react";
import { Server, Plus, Trash2, Pencil, Play, Terminal, Globe, Check, X, Loader2, Wrench, Eye, EyeOff, Key } from "lucide-react";
import { getTransport, type McpServerSummary, type McpServerConfig, type CreateMcpRequest, type McpTestResult } from "@/services/transport";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/shared/ui/dialog";

// ============================================================================
// Types
// ============================================================================

interface EnvVarEntry {
  id: string;
  key: string;
  value: string;
}

// Helper functions for env var conversion
function recordToEnvVars(record: Record<string, string> | undefined): EnvVarEntry[] {
  if (!record) return [];
  return Object.entries(record).map(([key, value]) => ({
    id: crypto.randomUUID(),
    key,
    value,
  }));
}

function envVarsToRecord(envVars: EnvVarEntry[]): Record<string, string> | undefined {
  const filtered = envVars.filter((e) => e.key.trim() !== "");
  if (filtered.length === 0) return undefined;
  return Object.fromEntries(filtered.map((e) => [e.key.trim(), e.value]));
}

// ============================================================================
// Component
// ============================================================================

export function WebMcpsPanel() {
  const [mcpServers, setMcpServers] = useState<McpServerSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"create" | "edit">("create");
  const [editingId, setEditingId] = useState<string | null>(null);

  // Selected MCP for detail view
  const [selectedMcp, setSelectedMcp] = useState<McpServerConfig | null>(null);
  const [selectedSummary, setSelectedSummary] = useState<McpServerSummary | null>(null);

  // Test state
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<McpTestResult | null>(null);

  // Form state
  const [formData, setFormData] = useState<Partial<CreateMcpRequest>>({
    type: "stdio",
    name: "",
    description: "",
    command: "",
    args: [],
    url: "",
    enabled: true,
  });
  const [argsInput, setArgsInput] = useState("");
  const [envVars, setEnvVars] = useState<EnvVarEntry[]>([]);
  const [showEnvValues, setShowEnvValues] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadMcps();
  }, []);

  const loadMcps = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const result = await transport.listMcps();
      if (result.success && result.data) {
        setMcpServers(result.data.servers);
      } else {
        setError(result.error || "Failed to load MCP servers");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  };

  const loadMcpDetail = async (id: string) => {
    try {
      const transport = await getTransport();
      const result = await transport.getMcp(id);
      console.log("[MCP] Loaded detail for", id, ":", result.data);
      if (result.success && result.data) {
        setSelectedMcp(result.data);
      }
    } catch (err) {
      console.error("Failed to load MCP detail:", err);
    }
  };

  const handleSelectMcp = async (mcp: McpServerSummary) => {
    setSelectedSummary(mcp);
    setTestResult(null);
    await loadMcpDetail(mcp.id);
  };

  const openCreateDialog = () => {
    setDialogMode("create");
    setEditingId(null);
    setFormData({
      type: "stdio",
      name: "",
      description: "",
      command: "",
      args: [],
      url: "",
      enabled: true,
    });
    setArgsInput("");
    setEnvVars([]);
    setShowEnvValues(new Set());
    setDialogOpen(true);
  };

  const openEditDialog = () => {
    if (!selectedMcp) return;

    console.log("[MCP Edit] selectedMcp:", selectedMcp);
    console.log("[MCP Edit] selectedMcp.env:", selectedMcp.env);

    setDialogMode("edit");
    setEditingId(selectedMcp.id || selectedSummary?.id || null);
    setFormData({
      type: selectedMcp.type,
      name: selectedMcp.name,
      description: selectedMcp.description,
      command: selectedMcp.command || "",
      url: selectedMcp.url || "",
      enabled: selectedMcp.enabled,
    });
    setArgsInput(selectedMcp.args?.join(", ") || "");
    const loadedEnvVars = recordToEnvVars(selectedMcp.env);
    console.log("[MCP Edit] loadedEnvVars:", loadedEnvVars);
    setEnvVars(loadedEnvVars);
    setShowEnvValues(new Set());
    setDialogOpen(true);
  };

  const handleSave = async () => {
    if (!formData.name || !formData.type) return;

    try {
      const transport = await getTransport();
      const args = argsInput.split(",").map((a) => a.trim()).filter(Boolean);

      const request: CreateMcpRequest = {
        type: formData.type as CreateMcpRequest["type"],
        name: formData.name,
        description: formData.description || "",
        enabled: formData.enabled ?? true,
      };

      // Preserve ID when editing
      if (dialogMode === "edit" && editingId) {
        request.id = editingId;
      }

      if (formData.type === "stdio") {
        request.command = formData.command || "";
        request.args = args;
        request.env = envVarsToRecord(envVars);
      } else {
        request.url = formData.url || "";
      }

      let result;
      if (dialogMode === "edit" && editingId) {
        result = await transport.updateMcp(editingId, request);
      } else {
        result = await transport.createMcp(request);
      }

      if (result.success) {
        await loadMcps();
        setDialogOpen(false);

        // Reload detail if we were editing the selected one
        if (dialogMode === "edit" && selectedSummary) {
          await loadMcpDetail(selectedSummary.id);
        }
      } else {
        setError(result.error || `Failed to ${dialogMode} MCP server`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this MCP server?")) return;

    try {
      const transport = await getTransport();
      const result = await transport.deleteMcp(id);
      if (result.success) {
        setSelectedMcp(null);
        setSelectedSummary(null);
        await loadMcps();
      } else {
        setError(result.error || "Failed to delete MCP server");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleTest = async () => {
    if (!selectedSummary) return;

    setIsTesting(true);
    setTestResult(null);

    try {
      const transport = await getTransport();
      const result = await transport.testMcp(selectedSummary.id);

      if (result.success && result.data) {
        setTestResult(result.data);
      } else {
        setTestResult({
          success: false,
          message: result.error || "Test failed",
        });
      }
    } catch (err) {
      setTestResult({
        success: false,
        message: err instanceof Error ? err.message : "Unknown error",
      });
    } finally {
      setIsTesting(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-[var(--background)]">
        <Loader2 className="w-8 h-8 text-[var(--primary)] animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex h-full bg-[var(--background)]">
      {/* MCP Servers List */}
      <div className="w-80 bg-[var(--card)] border-r border-[var(--border)] flex flex-col">
        <div className="p-4 border-b border-[var(--border)] flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Server className="w-5 h-5 text-[var(--primary)]" />
            <h1 className="text-lg font-semibold text-[var(--foreground)]">MCP Servers</h1>
          </div>
          <button
            onClick={openCreateDialog}
            className="inline-flex items-center gap-1.5 bg-[var(--primary)] hover:opacity-90 text-[var(--primary-foreground)] px-3 py-1.5 rounded-lg text-sm transition-all font-medium"
          >
            <Plus className="w-4 h-4" />
            New
          </button>
        </div>

        {error && (
          <div className="p-3 bg-red-50 border-b border-red-200 text-red-700 text-sm flex items-center justify-between">
            <span>{error}</span>
            <button onClick={() => setError(null)} className="text-red-500 hover:text-red-700">
              <X className="w-4 h-4" />
            </button>
          </div>
        )}

        <div className="flex-1 overflow-auto">
          {mcpServers.length === 0 ? (
            <div className="p-6 text-center">
              <div className="w-12 h-12 rounded-xl bg-cyan-100 flex items-center justify-center mx-auto mb-3">
                <Server className="w-6 h-6 text-cyan-600" />
              </div>
              <p className="text-[var(--foreground)] font-medium">No MCP servers configured</p>
              <p className="text-sm text-[var(--muted-foreground)] mt-1">Add an MCP server to extend agent capabilities</p>
            </div>
          ) : (
            mcpServers.map((mcp) => (
              <button
                key={mcp.id}
                onClick={() => handleSelectMcp(mcp)}
                className={`w-full text-left px-4 py-3 border-b border-[var(--border)] hover:bg-[var(--muted)] transition-colors ${
                  selectedSummary?.id === mcp.id
                    ? "bg-[var(--accent)] border-l-2 border-l-[var(--primary)]"
                    : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="font-medium text-[var(--foreground)]">{mcp.name}</div>
                  <div className="flex items-center gap-2">
                    <span className="inline-flex items-center gap-1 text-xs bg-[var(--muted)] text-[var(--foreground)] px-2 py-0.5 rounded-md font-medium">
                      {mcp.type === "stdio" ? <Terminal className="w-3 h-3" /> : <Globe className="w-3 h-3" />}
                      {mcp.type}
                    </span>
                    {mcp.enabled ? (
                      <span className="w-2 h-2 rounded-full bg-emerald-500" title="Enabled" />
                    ) : (
                      <span className="w-2 h-2 rounded-full bg-gray-400" title="Disabled" />
                    )}
                  </div>
                </div>
                <div className="text-sm text-[var(--muted-foreground)] truncate">{mcp.description}</div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* MCP Detail */}
      <div className="flex-1 overflow-auto">
        {selectedMcp && selectedSummary ? (
          <div className="p-8 max-w-lg">
            <div className="flex items-start justify-between mb-6">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-cyan-100 flex items-center justify-center">
                  <Server className="w-5 h-5 text-cyan-600" />
                </div>
                <div>
                  <h2 className="text-xl font-semibold text-[var(--foreground)]">{selectedMcp.name}</h2>
                  <p className="text-sm text-[var(--muted-foreground)]">{selectedSummary.id}</p>
                </div>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={handleTest}
                  disabled={isTesting}
                  className="inline-flex items-center gap-1.5 text-[var(--primary)] border border-[var(--primary)] hover:bg-[var(--accent)] px-3 py-1.5 rounded-lg text-sm font-medium transition-all disabled:opacity-50"
                >
                  {isTesting ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Play className="w-4 h-4" />
                  )}
                  Test
                </button>
                <button
                  onClick={openEditDialog}
                  className="inline-flex items-center gap-1.5 text-[var(--foreground)] border border-[var(--border)] hover:bg-[var(--muted)] px-3 py-1.5 rounded-lg text-sm font-medium transition-all"
                >
                  <Pencil className="w-4 h-4" />
                  Edit
                </button>
                <button
                  onClick={() => handleDelete(selectedSummary.id)}
                  className="text-[var(--muted-foreground)] hover:text-red-500 transition-colors p-2 hover:bg-red-50 rounded-lg"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>

            {testResult && (
              <div
                className={`mb-6 p-4 rounded-xl ${
                  testResult.success
                    ? "bg-emerald-50 border border-emerald-200"
                    : "bg-red-50 border border-red-200"
                }`}
              >
                <div className={`flex items-start gap-2 ${testResult.success ? "text-emerald-700" : "text-red-700"}`}>
                  {testResult.success ? (
                    <Check className="w-5 h-5 flex-shrink-0 mt-0.5" />
                  ) : (
                    <X className="w-5 h-5 flex-shrink-0 mt-0.5" />
                  )}
                  <div>
                    <p>{testResult.message}</p>
                    {testResult.tools && testResult.tools.length > 0 && (
                      <div className="mt-3">
                        <p className="text-sm font-medium flex items-center gap-1.5 mb-2">
                          <Wrench className="w-4 h-4" />
                          Available tools ({testResult.tools.length})
                        </p>
                        <div className="flex flex-wrap gap-1.5">
                          {testResult.tools.map((tool) => (
                            <span key={tool} className="text-xs bg-white/50 px-2 py-1 rounded-lg font-mono">
                              {tool}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}

            <div className="space-y-4">
              <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Type</label>
                <span className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-[var(--muted)] rounded-lg text-sm font-medium text-[var(--foreground)]">
                  {selectedMcp.type === "stdio" ? <Terminal className="w-3.5 h-3.5" /> : <Globe className="w-3.5 h-3.5" />}
                  {selectedMcp.type}
                </span>
              </div>

              <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Description</label>
                <p className="text-[var(--foreground)]">{selectedMcp.description || "No description"}</p>
              </div>

              {selectedMcp.type === "stdio" ? (
                <>
                  <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                    <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-1 flex items-center gap-1.5">
                      <Terminal className="w-3.5 h-3.5" />
                      Command
                    </label>
                    <code className="block bg-[var(--muted)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] font-mono">
                      {selectedMcp.command}
                    </code>
                  </div>
                  {selectedMcp.args && selectedMcp.args.length > 0 && (
                    <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                      <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-2">Arguments</label>
                      <div className="flex flex-wrap gap-2">
                        {selectedMcp.args.map((arg, i) => (
                          <span key={i} className="px-2.5 py-1 bg-[var(--muted)] rounded-lg text-sm font-mono text-[var(--foreground)]">
                            {arg}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                  {selectedMcp.env && Object.keys(selectedMcp.env).length > 0 && (
                    <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                      <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-2 flex items-center gap-1.5">
                        <Key className="w-3.5 h-3.5" />
                        Environment Variables
                      </label>
                      <div className="space-y-1.5">
                        {Object.entries(selectedMcp.env).map(([key, value]) => (
                          <div key={key} className="flex items-center gap-2 text-sm font-mono">
                            <span className="text-[var(--foreground)] bg-[var(--muted)] px-2 py-1 rounded">{key}</span>
                            <span className="text-[var(--muted-foreground)]">=</span>
                            <span className="text-[var(--muted-foreground)]">{"•".repeat(Math.min(value.length, 20))}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                  <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-1 flex items-center gap-1.5">
                    <Globe className="w-3.5 h-3.5" />
                    URL
                  </label>
                  <code className="block bg-[var(--muted)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] font-mono">
                    {selectedMcp.url}
                  </code>
                </div>
              )}

              <div className="bg-[var(--card)] rounded-xl border border-[var(--border)] p-4">
                <label className="block text-xs font-medium text-[var(--muted-foreground)] uppercase tracking-wider mb-1">Status</label>
                <span className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium ${
                  selectedMcp.enabled
                    ? "bg-emerald-100 text-emerald-700"
                    : "bg-[var(--muted)] text-[var(--muted-foreground)]"
                }`}>
                  <span className={`w-2 h-2 rounded-full ${selectedMcp.enabled ? "bg-emerald-500" : "bg-gray-400"}`} />
                  {selectedMcp.enabled ? "Enabled" : "Disabled"}
                </span>
              </div>

              <div className="bg-[var(--muted)] rounded-xl p-4 border border-dashed border-[var(--border)]">
                <h3 className="text-sm font-medium text-[var(--foreground)] mb-2">Usage</h3>
                <p className="text-sm text-[var(--muted-foreground)] mb-3">
                  To use this MCP server, add its ID to an agent's <code className="bg-white/50 px-1.5 py-0.5 rounded">mcps</code> field:
                </p>
                <code className="block bg-[var(--card)] rounded-lg px-3 py-2 text-sm text-[var(--foreground)] font-mono border border-[var(--border)]">
                  "mcps": ["{selectedSummary.id}"]
                </code>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-16 h-16 rounded-2xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-4">
                <Server className="w-8 h-8 text-[var(--muted-foreground)]" />
              </div>
              <p className="text-[var(--muted-foreground)] font-medium">Select an MCP server to view details</p>
              <p className="text-sm text-[var(--muted-foreground)] mt-1">
                MCP servers extend agent capabilities with external tools
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Create/Edit Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="bg-[var(--card)] border-[var(--border)]">
          <DialogHeader>
            <DialogTitle className="text-[var(--foreground)]">
              {dialogMode === "create" ? "Add MCP Server" : "Edit MCP Server"}
            </DialogTitle>
            <DialogDescription>
              {dialogMode === "create"
                ? "Configure a new MCP server to extend agent capabilities."
                : "Update the MCP server configuration."}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div>
              <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Type</label>
              <select
                value={formData.type}
                onChange={(e) => setFormData({ ...formData, type: e.target.value as CreateMcpRequest["type"] })}
                className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
              >
                <option value="stdio">Stdio (Local Process)</option>
                <option value="http">HTTP</option>
                <option value="sse">SSE (Server-Sent Events)</option>
                <option value="streamable-http">Streamable HTTP</option>
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Name</label>
              <input
                type="text"
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                placeholder="filesystem, weather, etc."
                className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Description</label>
              <input
                type="text"
                value={formData.description}
                onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                placeholder="What this server provides"
                className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
              />
            </div>

            {formData.type === "stdio" ? (
              <>
                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Command</label>
                  <input
                    type="text"
                    value={formData.command}
                    onChange={(e) => setFormData({ ...formData, command: e.target.value })}
                    placeholder="npx, node, python, etc."
                    className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">Arguments (comma-separated)</label>
                  <input
                    type="text"
                    value={argsInput}
                    onChange={(e) => setArgsInput(e.target.value)}
                    placeholder="-y, @anthropic/mcp-server-filesystem, /home/user"
                    className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
                  />
                </div>

                {/* Environment Variables */}
                <div>
                  <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5 flex items-center gap-1.5">
                    <Key className="w-3.5 h-3.5" />
                    Environment Variables
                  </label>
                  <div className="space-y-2">
                    {envVars.map((envVar) => (
                      <div key={envVar.id} className="flex gap-2">
                        <input
                          type="text"
                          value={envVar.key}
                          onChange={(e) => {
                            setEnvVars((prev) =>
                              prev.map((ev) =>
                                ev.id === envVar.id ? { ...ev, key: e.target.value } : ev
                              )
                            );
                          }}
                          placeholder="VARIABLE_NAME"
                          className="flex-1 bg-[var(--muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)] font-mono"
                        />
                        <div className="flex-1 relative">
                          <input
                            type={showEnvValues.has(envVar.id) ? "text" : "password"}
                            value={envVar.value}
                            onChange={(e) => {
                              setEnvVars((prev) =>
                                prev.map((ev) =>
                                  ev.id === envVar.id ? { ...ev, value: e.target.value } : ev
                                )
                              );
                            }}
                            placeholder="value"
                            className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-lg px-3 py-2 pr-9 text-sm focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)] font-mono"
                          />
                          <button
                            type="button"
                            onClick={() => {
                              setShowEnvValues((prev) => {
                                const next = new Set(prev);
                                if (next.has(envVar.id)) {
                                  next.delete(envVar.id);
                                } else {
                                  next.add(envVar.id);
                                }
                                return next;
                              });
                            }}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
                          >
                            {showEnvValues.has(envVar.id) ? (
                              <EyeOff className="w-4 h-4" />
                            ) : (
                              <Eye className="w-4 h-4" />
                            )}
                          </button>
                        </div>
                        <button
                          type="button"
                          onClick={() => {
                            setEnvVars((prev) => prev.filter((ev) => ev.id !== envVar.id));
                          }}
                          className="text-[var(--muted-foreground)] hover:text-red-500 p-2 hover:bg-red-50 rounded-lg transition-colors"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                    <button
                      type="button"
                      onClick={() => {
                        setEnvVars((prev) => [
                          ...prev,
                          { id: crypto.randomUUID(), key: "", value: "" },
                        ]);
                      }}
                      className="inline-flex items-center gap-1.5 text-sm text-[var(--primary)] hover:text-[var(--primary)]/80 font-medium"
                    >
                      <Plus className="w-4 h-4" />
                      Add Variable
                    </button>
                  </div>
                </div>
              </>
            ) : (
              <div>
                <label className="block text-sm font-medium text-[var(--foreground)] mb-1.5">URL</label>
                <input
                  type="text"
                  value={formData.url}
                  onChange={(e) => setFormData({ ...formData, url: e.target.value })}
                  placeholder="http://localhost:8080/mcp"
                  className="w-full bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-2.5 focus:outline-none focus:ring-2 focus:ring-[var(--primary)] text-[var(--foreground)]"
                />
              </div>
            )}

            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                id="enabled"
                checked={formData.enabled}
                onChange={(e) => setFormData({ ...formData, enabled: e.target.checked })}
                className="w-4 h-4 rounded border-[var(--border)] bg-[var(--muted)] text-[var(--primary)] focus:ring-[var(--primary)]"
              />
              <label htmlFor="enabled" className="text-sm text-[var(--foreground)]">
                Enabled
              </label>
            </div>
          </div>

          <DialogFooter>
            <button
              onClick={() => setDialogOpen(false)}
              className="px-4 py-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors font-medium"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={!formData.name || (formData.type === "stdio" ? !formData.command : !formData.url)}
              className="bg-[var(--primary)] hover:opacity-90 disabled:opacity-50 text-[var(--primary-foreground)] px-5 py-2 rounded-xl transition-all font-medium"
            >
              {dialogMode === "create" ? "Add Server" : "Save Changes"}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
