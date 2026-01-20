// ============================================================================
// CREATE VAULT DIALOG
// Dialog for creating a new vault
// ============================================================================

import { useState } from "react";
import { X } from "lucide-react";
import { createVault } from "@/services/vaults";
import type { Vault } from "@/shared/types";

interface CreateVaultDialogProps {
  open: boolean;
  onClose: () => void;
  onCreated: (vault: Vault) => void;
}

export function CreateVaultDialog({
  open,
  onClose,
  onCreated,
}: CreateVaultDialogProps) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!name.trim()) {
      setError("Vault name is required");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const vault = await createVault({
        name: name.trim(),
        path: path.trim() || undefined,
      });
      onCreated(vault);
      setName("");
      setPath("");
    } catch (err) {
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  };

  const handleClose = () => {
    setName("");
    setPath("");
    setError(null);
    onClose();
  };

  if (!open) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-gray-800 rounded-lg shadow-xl w-full max-w-md border border-white/10">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <h2 className="text-lg font-semibold text-white">Create New Vault</h2>
          <button
            onClick={handleClose}
            className="text-gray-400 hover:text-white transition-colors"
          >
            <X className="size-5" />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-4 py-4 space-y-4">
          {/* Name */}
          <div>
            <label htmlFor="vault-name" className="block text-sm font-medium text-gray-300 mb-1">
              Vault Name <span className="text-red-400">*</span>
            </label>
            <input
              id="vault-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Project Vault"
              className="w-full px-3 py-2 bg-gray-700 border border-white/10 rounded-md text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              disabled={isLoading}
            />
          </div>

          {/* Path (Optional) */}
          <div>
            <label htmlFor="vault-path" className="block text-sm font-medium text-gray-300 mb-1">
              Path (Optional)
            </label>
            <input
              id="vault-path"
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="~/Documents/my-vault"
              className="w-full px-3 py-2 bg-gray-700 border border-white/10 rounded-md text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              disabled={isLoading}
            />
            <p className="text-xs text-gray-400 mt-1">
              If not specified, vault will be created in ~/Documents/
            </p>
          </div>

          {/* Error */}
          {error && (
            <div className="bg-red-500/10 border border-red-500/50 rounded-md px-3 py-2">
              <p className="text-sm text-red-400">{error}</p>
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={handleClose}
              disabled={isLoading}
              className="px-4 py-2 text-sm font-medium text-gray-300 bg-gray-700 hover:bg-gray-600 rounded-md transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading || !name.trim()}
              className="px-4 py-2 text-sm font-medium text-white bg-purple-600 hover:bg-purple-700 rounded-md transition-colors disabled:opacity-50"
            >
              {isLoading ? "Creating..." : "Create Vault"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
