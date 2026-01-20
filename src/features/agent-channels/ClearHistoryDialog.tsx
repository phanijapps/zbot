// ============================================================================
// CLEAR HISTORY DIALOG
// Chrome-style history clearing for agent channels
// ============================================================================

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/shared/ui/dialog";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import type { DeletionResult, DeletionScope } from "@/shared/types";
import { isDeletionResultEmpty } from "@/shared/types";

interface ClearHistoryDialogProps {
  open: boolean;
  onClose: () => void;
  agentId: string;
  agentName: string;
}

type ScopeType = "last_7_days" | "last_30_days" | "all_time";

export function ClearHistoryDialog({
  open,
  onClose,
  agentId,
  agentName,
}: ClearHistoryDialogProps) {
  const [scope, setScope] = useState<ScopeType>("last_30_days");
  const [deleting, setDeleting] = useState(false);
  const [result, setResult] = useState<DeletionResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleClear = async () => {
    setDeleting(true);
    setError(null);
    setResult(null);

    try {
      const deletionScope: DeletionScope =
        scope === "last_7_days"
          ? { type: "last_7_days" }
          : scope === "last_30_days"
            ? { type: "last_30_days" }
            : { type: "all_time" };

      const deletionResult = await invoke<DeletionResult>("delete_agent_history_with_scope", {
        agentId,
        scope: deletionScope,
      });

      setResult(deletionResult);

      // Auto-close after showing result briefly
      setTimeout(() => {
        onClose();
        setResult(null);
      }, 2000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  };

  const handleClose = () => {
    if (!deleting) {
      onClose();
      setResult(null);
      setError(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-md">
        <DialogHeader>
          <DialogTitle className="text-xl">Clear Chat History</DialogTitle>
          <p className="text-sm text-gray-400 mt-2">
            Delete messages from{" "}
            <span className="text-white font-semibold">{agentName}</span>
          </p>
        </DialogHeader>

        <div className="space-y-3 py-4">
          <Label className="text-white text-sm">Time range to delete:</Label>

          {[
            { value: "last_7_days" as const, label: "Last 7 days" },
            { value: "last_30_days" as const, label: "Last 30 days" },
            { value: "all_time" as const, label: "All time" },
          ].map((option) => (
            <label
              key={option.value}
              className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                scope === option.value
                  ? "bg-red-500/20 border-red-500/50"
                  : "border-white/10 hover:bg-white/5"
              }`}
            >
              <input
                type="radio"
                name="scope"
                value={option.value}
                checked={scope === option.value}
                onChange={(e) => setScope(e.target.value as ScopeType)}
                className="size-4 accent-red-500"
                disabled={deleting}
              />
              <span className="text-white">{option.label}</span>
            </label>
          ))}
        </div>

        {/* Result display */}
        {result && !isDeletionResultEmpty(result) && (
          <div className="bg-green-500/10 border border-green-500/20 rounded-lg p-3">
            <p className="text-sm text-green-200">
              ✓ Deleted {result.sessionsDeleted} sessions and{" "}
              {result.messagesDeleted} messages
            </p>
          </div>
        )}

        {/* Empty result */}
        {result && isDeletionResultEmpty(result) && (
          <div className="bg-yellow-500/10 border border-yellow-500/20 rounded-lg p-3">
            <p className="text-sm text-yellow-200">
              No messages found in the selected time range
            </p>
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="size-4 text-red-400 shrink-0 mt-0.5" />
              <p className="text-sm text-red-200">{error}</p>
            </div>
          </div>
        )}

        <DialogFooter>
          <Button
            variant="outline"
            onClick={handleClose}
            disabled={deleting}
            className="border-white/20 text-white hover:bg-white/5"
          >
            Cancel
          </Button>
          <Button
            onClick={handleClear}
            disabled={deleting || !!result}
            className="bg-red-600 hover:bg-red-700 text-white min-w-[140px]"
          >
            {deleting ? (
              <>
                <Loader2 className="size-4 mr-2 animate-spin" />
                Deleting...
              </>
            ) : result ? (
              "Deleted!"
            ) : (
              "Delete Messages"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
