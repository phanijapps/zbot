// ============================================================================
// ADD MCP SERVER DIALOG
// Dialog for adding/editing Model Context Protocol servers
// Supports both stdio (command-based) and HTTP-based MCP servers
// ============================================================================

import { useState, useEffect } from "react";
import { Server, Plus, Loader2, Globe, Terminal, Radio } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import type { MCPServer, MCPServerPreset, MCPServerType } from "./types";
import { MCP_SERVER_PRESETS, MCP_HTTP_PRESETS, MCP_SSE_PRESETS, MCP_STREAMABLE_HTTP_PRESETS } from "./types";

interface AddMCPServerDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (server: Omit<MCPServer, "id" | "createdAt">) => void;
  editingServer?: MCPServer | null;
}

const DEFAULT_STDIO_CONFIG = `{
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/directory"],
  "env": {}
}`;

const DEFAULT_HTTP_CONFIG = `{
  "url": "https://api.z.ai/api/mcp/web_search_prime/mcp",
  "headers": {
    "Authorization": "Bearer your_api_key"
  }
}`;

const DEFAULT_SSE_CONFIG = `{
  "url": "https://api.z.ai/api/mcp/web_search_prime/sse",
  "headers": {
    "Authorization": "Bearer your_api_key"
  }
}`;

const DEFAULT_STREAMABLE_HTTP_CONFIG = `{
  "url": "https://api.z.ai/api/mcp/web_search_prime/mcp",
  "headers": {
    "Authorization": "Bearer your_api_key"
  }
}`;

export function AddMCPServerDialog({ open, onClose, onSave, editingServer }: AddMCPServerDialogProps) {
  const [serverType, setServerType] = useState<MCPServerType>('stdio');
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [configJson, setConfigJson] = useState(DEFAULT_STDIO_CONFIG);
  const [configError, setConfigError] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  // Populate form when editing an existing server
  useEffect(() => {
    if (editingServer) {
      setServerType(editingServer.type || 'stdio');
      setName(editingServer.name);
      setDescription(editingServer.description);

      if (editingServer.type === 'http' || editingServer.type === 'sse' || editingServer.type === 'streamable-http') {
        const config = {
          url: editingServer.url || '',
          headers: editingServer.headers || {},
        };
        setConfigJson(JSON.stringify(config, null, 2));
      } else {
        const config = {
          command: editingServer.command || '',
          args: editingServer.args || [],
          env: editingServer.env || {},
        };
        setConfigJson(JSON.stringify(config, null, 2));
      }
      setConfigError("");
    } else {
      // Reset form for new server
      setServerType('stdio');
      setName("");
      setDescription("");
      setConfigJson(DEFAULT_STDIO_CONFIG);
      setConfigError("");
    }
  }, [editingServer, open]);

  // Update default config when type changes
  useEffect(() => {
    if (!editingServer && open) {
      if (serverType === 'streamable-http') {
        setConfigJson(DEFAULT_STREAMABLE_HTTP_CONFIG);
      } else if (serverType === 'sse') {
        setConfigJson(DEFAULT_SSE_CONFIG);
      } else if (serverType === 'http') {
        setConfigJson(DEFAULT_HTTP_CONFIG);
      } else {
        setConfigJson(DEFAULT_STDIO_CONFIG);
      }
      setConfigError("");
    }
  }, [serverType, editingServer, open]);

  // Validate and format JSON when config input changes
  const handleConfigChange = (value: string) => {
    setConfigJson(value);
    try {
      const parsed = JSON.parse(value);

      if (serverType === 'stdio') {
        // Validate stdio configuration
        if (!parsed.command || typeof parsed.command !== "string") {
          setConfigError("Missing or invalid 'command' field (must be a string)");
        } else if (!parsed.args || !Array.isArray(parsed.args)) {
          setConfigError("Missing or invalid 'args' field (must be an array)");
        } else if (parsed.env && typeof parsed.env !== "object") {
          setConfigError("Invalid 'env' field (must be an object)");
        } else {
          setConfigJson(JSON.stringify(parsed, null, 2));
          setConfigError("");
        }
      } else if (serverType === 'http' || serverType === 'sse' || serverType === 'streamable-http') {
        // Validate HTTP/SSE/StreamableHttp configuration
        if (!parsed.url || typeof parsed.url !== "string") {
          setConfigError("Missing or invalid 'url' field (must be a string)");
        } else if (parsed.headers && typeof parsed.headers !== "object") {
          setConfigError("Invalid 'headers' field (must be an object)");
        } else {
          setConfigJson(JSON.stringify(parsed, null, 2));
          setConfigError("");
        }
      } else {
        setConfigError("Invalid JSON format");
      }
    } catch {
      setConfigError("Invalid JSON format");
    }
  };

  const handlePresetSelect = (preset: MCPServerPreset) => {
    setName(preset.name);
    setDescription(preset.description);

    if (serverType === 'stdio') {
      const args = preset.args.split(" ").filter((a) => a.trim());
      const envObj: Record<string, string> = {};
      if (preset.env) {
        preset.env.split("\n").forEach((line) => {
          const [key, ...valueParts] = line.split("=");
          if (key && valueParts.length) {
            envObj[key.trim()] = valueParts.join("=").trim();
          }
        });
      }

      const config = {
        command: preset.command,
        args,
        env: Object.keys(envObj).length > 0 ? envObj : undefined,
      };
      setConfigJson(JSON.stringify(config, null, 2));
    } else {
      // HTTP/SSE preset - args contains the URL
      const config = {
        url: preset.args,
        headers: {},
      };
      setConfigJson(JSON.stringify(config, null, 2));
    }
    setConfigError("");
  };

  const handleSave = async () => {
    // Validate config JSON
    let config: Record<string, unknown>;
    try {
      config = JSON.parse(configJson);
    } catch {
      setConfigError("Invalid JSON format");
      return;
    }

    setIsSaving(true);
    try {
      const server: Omit<MCPServer, "id" | "createdAt"> = {
        name,
        description,
        type: serverType,
        enabled: false,
        status: "stopped",
        validated: false,
      };

      if (serverType === 'stdio') {
        if (!config.command || typeof config.command !== "string") {
          setConfigError("Missing or invalid 'command' field");
          return;
        }
        if (!config.args || !Array.isArray(config.args)) {
          setConfigError("Missing or invalid 'args' field");
          return;
        }
        server.command = config.command;
        server.args = config.args;
        server.env = config.env as Record<string, string> | undefined;
      } else if (serverType === 'http' || serverType === 'sse' || serverType === 'streamable-http') {
        if (!config.url || typeof config.url !== "string") {
          setConfigError("Missing or invalid 'url' field");
          return;
        }
        server.url = config.url;
        server.headers = config.headers as Record<string, string> | undefined;
      }

      await onSave(server);

      // Reset form
      setServerType('stdio');
      setName("");
      setDescription("");
      setConfigJson(DEFAULT_STDIO_CONFIG);
      setConfigError("");
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  const isValid = name && description && !configError;
  const presets = serverType === 'http' ? MCP_HTTP_PRESETS
    : serverType === 'sse' ? MCP_SSE_PRESETS
    : serverType === 'streamable-http' ? MCP_STREAMABLE_HTTP_PRESETS
    : MCP_SERVER_PRESETS;

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-2xl font-bold flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-green-500 to-emerald-600">
              <Server className="size-6 text-white" />
            </div>
            {editingServer ? "Edit MCP Server" : "Add MCP Server"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-6 mt-4">
          {/* Type Selector - only show when adding new server */}
          {!editingServer && (
            <div>
              <Label className="text-white mb-3 block">Server Type</Label>
              <div className="grid grid-cols-2 gap-3">
                <button
                  onClick={() => setServerType('stdio')}
                  className={`p-4 rounded-lg border-2 transition-all flex flex-col items-center gap-2 ${
                    serverType === 'stdio'
                      ? 'bg-green-500/20 border-green-500 text-green-400'
                      : 'bg-white/5 border-white/10 text-gray-400 hover:border-white/20'
                  }`}
                >
                  <Terminal className="size-5" />
                  <span className="font-medium text-sm">Command</span>
                  <span className="text-xs opacity-70">stdio subprocess</span>
                </button>
                <button
                  onClick={() => setServerType('http')}
                  className={`p-4 rounded-lg border-2 transition-all flex flex-col items-center gap-2 ${
                    serverType === 'http'
                      ? 'bg-blue-500/20 border-blue-500 text-blue-400'
                      : 'bg-white/5 border-white/10 text-gray-400 hover:border-white/20'
                  }`}
                >
                  <Globe className="size-5" />
                  <span className="font-medium text-sm">HTTP</span>
                  <span className="text-xs opacity-70">JSON-RPC over HTTP</span>
                </button>
                <button
                  onClick={() => setServerType('sse')}
                  className={`p-4 rounded-lg border-2 transition-all flex flex-col items-center gap-2 ${
                    serverType === 'sse'
                      ? 'bg-purple-500/20 border-purple-500 text-purple-400'
                      : 'bg-white/5 border-white/10 text-gray-400 hover:border-white/20'
                  }`}
                >
                  <Radio className="size-5" />
                  <span className="font-medium text-sm">SSE</span>
                  <span className="text-xs opacity-70">Server-Sent Events</span>
                </button>
                <button
                  onClick={() => setServerType('streamable-http')}
                  className={`p-4 rounded-lg border-2 transition-all flex flex-col items-center gap-2 ${
                    serverType === 'streamable-http'
                      ? 'bg-orange-500/20 border-orange-500 text-orange-400'
                      : 'bg-white/5 border-white/10 text-gray-400 hover:border-white/20'
                  }`}
                >
                  <Radio className="size-5" />
                  <span className="font-medium text-sm">Streamable</span>
                  <span className="text-xs opacity-70">Streamable-HTTP</span>
                </button>
              </div>
            </div>
          )}

          {/* Presets - only show when adding new server */}
          {!editingServer && (
            <div>
              <Label className="text-white mb-3 block">Quick Presets</Label>
              <div className="grid grid-cols-2 gap-2">
                {presets.map((preset) => (
                  <button
                    key={preset.name}
                    onClick={() => handlePresetSelect(preset)}
                    className="p-3 bg-white/5 hover:bg-white/10 border border-white/10 hover:border-white/20 rounded-lg text-left transition-all"
                  >
                    <p className="text-sm font-medium text-white mb-1">{preset.name}</p>
                    <p className="text-xs text-gray-400">{preset.description}</p>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Name and Description */}
          <div className="space-y-4">
            <div>
              <Label className="text-white mb-2 block">Name</Label>
              <Input
                placeholder="e.g., Filesystem Server"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
            </div>

            <div>
              <Label className="text-white mb-2 block">Description</Label>
              <Input
                placeholder="What does this server do?"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
            </div>
          </div>

          {/* Configuration JSON */}
          <div>
            <Label className="text-white mb-2 block">
              Configuration (JSON)
              {configError && <span className="text-red-400 ml-2">{configError}</span>}
            </Label>
            <textarea
              value={configJson}
              onChange={(e) => handleConfigChange(e.target.value)}
              className={`w-full min-h-[180px] bg-white/5 ${configError ? "border-red-500/50" : "border-white/10"} text-white placeholder:text-gray-500 rounded-lg px-3 py-2 font-mono text-sm`}
              placeholder={serverType === 'streamable-http' ? DEFAULT_STREAMABLE_HTTP_CONFIG : serverType === 'sse' ? DEFAULT_SSE_CONFIG : serverType === 'http' ? DEFAULT_HTTP_CONFIG : DEFAULT_STDIO_CONFIG}
            />
            {!configError && (
              <p className="text-xs text-gray-500 mt-1">
                {serverType === 'streamable-http' || serverType === 'sse'
                  ? 'JSON with url (string) and optional headers (object)'
                  : serverType === 'http'
                  ? 'JSON with url (string) and optional headers (object)'
                  : 'JSON with command, args (array), and optional env (object)'}
              </p>
            )}
          </div>

          {/* Actions */}
          <div className="flex gap-3 pt-4">
            <Button
              onClick={onClose}
              variant="outline"
              className="flex-1 border-white/20 text-white hover:bg-white/5"
              disabled={isSaving}
            >
              Cancel
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isValid || isSaving}
              className="flex-1 bg-gradient-to-br from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white"
            >
              {isSaving ? (
                <>
                  <Loader2 className="size-4 mr-2 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Plus className="size-4 mr-2" />
                  {editingServer ? "Update Server" : "Add Server"}
                </>
              )}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
