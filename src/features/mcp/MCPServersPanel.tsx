// ============================================================================
// MCP SERVERS FEATURE
// Model Context Protocol server management
// ============================================================================

import { useState, useEffect } from "react";
import { Server, Plus, Trash2, Check, Loader2, RefreshCw, Play, Edit } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/shared/ui/button";
import { Badge } from "@/shared/ui/badge";
import { AddMCPServerDialog } from "./AddMCPServerDialog";
import * as mcpService from "@/services/mcp";
import type { MCPServer } from "./types";
import { useVaults } from "@/features/vaults/useVaults";

export function MCPServersPanel() {
  const { currentVault } = useVaults();
  const [servers, setServers] = useState<MCPServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [editingServer, setEditingServer] = useState<MCPServer | null>(null);
  const [testingServerId, setTestingServerId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  // Load servers on mount and when vault changes
  useEffect(() => {
    loadServers();
  }, [currentVault?.id]); // Reload when vault changes

  const loadServers = async () => {
    setLoading(true);
    try {
      const loaded = await mcpService.listMCPServers();
      setServers(loaded);
    } catch (error) {
      console.error("Failed to load MCP servers:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadServers();
    setRefreshing(false);
  };

  const handleOpenCreateDialog = () => {
    setEditingServer(null);
    setShowAddDialog(true);
  };

  const handleOpenEditDialog = (server: MCPServer) => {
    setEditingServer(server);
    setShowAddDialog(true);
  };

  const handleSaveServer = async (server: Omit<MCPServer, "id" | "createdAt">) => {
    if (editingServer) {
      await mcpService.updateMCPServer(editingServer.id, server);
    } else {
      await mcpService.createMCPServer(server);
    }
    await loadServers();
  };

  const handleDeleteServer = async (id: string) => {
    if (confirm("Are you sure you want to delete this MCP server?")) {
      try {
        await mcpService.deleteMCPServer(id);
        await loadServers();
      } catch (error) {
        console.error("Failed to delete server:", error);
      }
    }
  };

  const handleTestServer = async (server: MCPServer) => {
    setTestingServerId(server.id);
    try {
      // Use validate which tests and saves the result
      const result = await mcpService.validateMCPServer(server.id);
      await loadServers();

      if (result.success) {
        // Show success toast
        toast.success(result.message, {
          description: result.tools && result.tools.length > 0
            ? `${result.tools.length} tool${result.tools.length > 1 ? 's' : ''} available`
            : undefined,
          duration: 3000,
        });
      } else {
        // Show failure popup with details
        alert(`✗ ${result.message}`);
      }
    } catch (error) {
      console.error("Test server error:", error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      alert(`Test failed: ${errorMsg}`);
    } finally {
      setTestingServerId(null);
    }
  };

  return (
    <>
      <div className="p-6">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-2xl font-bold text-foreground">MCP Servers</h2>
            <p className="text-muted-foreground text-sm mt-1">
              Model Context Protocol servers extend AI capabilities with external tools
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              className="border-border text-foreground hover:bg-accent"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              <RefreshCw className={`size-4 ${refreshing ? "animate-spin" : ""}`} />
            </Button>
            <Button
              className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
              onClick={handleOpenCreateDialog}
            >
              <Plus className="size-4 mr-2" />
              Add Server
            </Button>
          </div>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="size-8 text-foreground animate-spin" />
          </div>
        ) : servers.length === 0 ? (
          <div className="text-center py-20">
            <Server className="size-16 text-muted-foreground mx-auto mb-4" />
            <h3 className="text-xl font-medium text-foreground mb-2">No MCP Servers</h3>
            <p className="text-muted-foreground">Add your first MCP server to get started</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {servers.map((server) => (
              <div
                key={server.id}
                className="bg-card rounded-xl p-5 border border-border hover:border-primary/50 transition-all"
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-start gap-3">
                    <div className="p-2.5 rounded-xl bg-gradient-to-br from-gray-500 to-gray-600">
                      <Server className="size-4 text-white" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2 mb-1">
                        <h3 className="text-foreground font-semibold">{server.name}</h3>
                        {server.validated && (
                          <Badge className="bg-green-500/20 text-green-300 border-green-500/30 text-xs">
                            <Check className="size-3 mr-1" />
                            Verified
                          </Badge>
                        )}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    {/* Test Button */}
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleTestServer(server)}
                      disabled={testingServerId === server.id}
                      className="text-muted-foreground hover:text-primary h-7 w-7 p-0"
                      title="Test server"
                    >
                      {testingServerId === server.id ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <Play className="size-3.5" />
                      )}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleOpenEditDialog(server)}
                      className="text-muted-foreground hover:text-foreground h-7 w-7 p-0"
                    >
                      <Edit className="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleDeleteServer(server.id)}
                      className="text-muted-foreground hover:text-destructive h-7 w-7 p-0"
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                </div>

                <p className="text-muted-foreground text-sm mb-3">{server.description}</p>

                {/* Command/URL display based on server type */}
                {server.type === 'stdio' ? (
                  <div className="bg-muted rounded-lg p-2.5 mb-3 border border-border">
                    <p className="text-xs text-muted-foreground mb-1">Command</p>
                    <code className="text-xs text-foreground font-mono block truncate">
                      {server.command} {server.args?.join(" ")}
                    </code>
                  </div>
                ) : (
                  <div className="bg-muted rounded-lg p-2.5 mb-3 border border-border">
                    <p className="text-xs text-muted-foreground mb-1">URL</p>
                    <code className="text-xs text-foreground font-mono block truncate">
                      {server.url}
                    </code>
                  </div>
                )}

                {/* Environment Variables (stdio only) */}
                {server.type === 'stdio' && server.env && Object.keys(server.env).length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {Object.entries(server.env).slice(0, 3).map(([key, _value]) => (
                      <span
                        key={key}
                        className="px-2 py-0.5 bg-blue-500/10 rounded-full text-xs text-blue-300 border border-blue-500/20"
                      >
                        {key}
                      </span>
                    ))}
                    {Object.keys(server.env).length > 3 && (
                      <span className="px-2 py-0.5 bg-muted rounded-full text-xs text-muted-foreground border border-border">
                        +{Object.keys(server.env).length - 3} more
                      </span>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {/* Info Box */}
        <div className="mt-6 bg-orange-500/10 border border-orange-500/20 rounded-xl p-4">
          <div className="flex items-start gap-3">
            <Server className="size-5 text-orange-400 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="text-sm font-medium text-orange-200 mb-2">
                About MCP Servers
              </p>
              <p className="text-xs text-orange-300">
                Model Context Protocol (MCP) servers provide AI models with access to external tools, APIs, and data sources.
                Supports command-based (stdio), HTTP, and SSE (Server-Sent Events) servers.
              </p>
              <p className="text-xs text-orange-300 mt-2">
                💾 Configuration saved to: <code className="bg-white/10 px-1.5 py-0.5 rounded">{currentVault?.path || "~/.config/zeroagent"}/mcps.json</code>
              </p>
            </div>
          </div>
        </div>
      </div>

      <AddMCPServerDialog
        open={showAddDialog}
        onClose={() => setShowAddDialog(false)}
        onSave={handleSaveServer}
        editingServer={editingServer}
      />
    </>
  );
}
