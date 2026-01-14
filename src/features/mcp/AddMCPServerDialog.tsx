// ============================================================================
// ADD MCP SERVER DIALOG
// Dialog for adding/editing Model Context Protocol servers
// ============================================================================

import { useState, useEffect } from "react";
import { Server, Plus, Loader2 } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import type { MCPServer, MCPServerPreset } from "./types";
import { MCP_SERVER_PRESETS } from "./types";

interface AddMCPServerDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (server: Omit<MCPServer, "id" | "createdAt">) => void;
  editingServer?: MCPServer | null;
}

interface MCPConfig {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

const DEFAULT_CONFIG = `{
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/directory"],
  "env": {}
}`;

export function AddMCPServerDialog({ open, onClose, onSave, editingServer }: AddMCPServerDialogProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [configJson, setConfigJson] = useState(DEFAULT_CONFIG);
  const [configError, setConfigError] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  // Populate form when editing an existing server
  useEffect(() => {
    if (editingServer) {
      setName(editingServer.name);
      setDescription(editingServer.description);
      const config: MCPConfig = {
        command: editingServer.command,
        args: editingServer.args,
        env: editingServer.env || {},
      };
      setConfigJson(JSON.stringify(config, null, 2));
      setConfigError("");
    } else {
      // Reset form for new server
      setName("");
      setDescription("");
      setConfigJson(DEFAULT_CONFIG);
      setConfigError("");
    }
  }, [editingServer, open]);

  // Validate and format JSON when config input changes
  const handleConfigChange = (value: string) => {
    setConfigJson(value);
    try {
      const parsed = JSON.parse(value);
      // Validate structure
      if (!parsed.command || typeof parsed.command !== "string") {
        setConfigError("Missing or invalid 'command' field (must be a string)");
      } else if (!parsed.args || !Array.isArray(parsed.args)) {
        setConfigError("Missing or invalid 'args' field (must be an array)");
      } else if (parsed.env && typeof parsed.env !== "object") {
        setConfigError("Invalid 'env' field (must be an object)");
      } else {
        // Valid JSON - format it
        setConfigJson(JSON.stringify(parsed, null, 2));
        setConfigError("");
      }
    } catch {
      setConfigError("Invalid JSON format");
    }
  };

  const handlePresetSelect = (preset: MCPServerPreset) => {
    setName(preset.name);
    setDescription(preset.description);

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

    const config: MCPConfig = {
      command: preset.command,
      args,
      env: Object.keys(envObj).length > 0 ? envObj : undefined,
    };
    setConfigJson(JSON.stringify(config, null, 2));
    setConfigError("");
  };

  const handleSave = async () => {
    // Validate config JSON
    let config: MCPConfig;
    try {
      config = JSON.parse(configJson);
    } catch {
      setConfigError("Invalid JSON format");
      return;
    }

    if (!config.command || typeof config.command !== "string") {
      setConfigError("Missing or invalid 'command' field");
      return;
    }
    if (!config.args || !Array.isArray(config.args)) {
      setConfigError("Missing or invalid 'args' field");
      return;
    }

    setIsSaving(true);
    try {
      const server: Omit<MCPServer, "id" | "createdAt"> = {
        name,
        description,
        command: config.command,
        args: config.args,
        env: config.env,
        enabled: false,
        status: "stopped",
        validated: false,
      };

      await onSave(server);

      // Reset form
      setName("");
      setDescription("");
      setConfigJson(DEFAULT_CONFIG);
      setConfigError("");
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  const isValid = name && description && !configError;

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
          {/* Presets - only show when adding new server */}
          {!editingServer && (
            <div>
              <Label className="text-white mb-3 block">Quick Presets</Label>
              <div className="grid grid-cols-2 gap-2">
                {MCP_SERVER_PRESETS.map((preset) => (
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
              placeholder={DEFAULT_CONFIG}
            />
            {!configError && (
              <p className="text-xs text-gray-500 mt-1">JSON with command, args (array), and env (object)</p>
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
