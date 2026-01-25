// ============================================================================
// ATTACHMENTS PANEL
// Side panel for viewing and managing transcript attachments
// ============================================================================

import { useState, useMemo } from "react";
import { FileText, Search, X, Trash2, Send } from "lucide-react";
import type { Transcript } from "./TranscriptViewer";

export interface Attachment {
  id: string;
  type: "transcript" | "file";
  filename: string;
  filePath: string;
  createdAt: number;
  metadata: {
    duration?: number;
    speakerCount?: number;
    transcript?: Transcript;
  };
}

interface AttachmentsPanelProps {
  open: boolean;
  onClose: () => void;
  attachments: Attachment[];
  onView: (attachment: Attachment) => void;
  onSend: (attachment: Attachment) => void;
  onDelete: (attachmentId: string) => void;
}

export function AttachmentsPanel({
  open,
  onClose,
  attachments,
  onView,
  onSend,
  onDelete,
}: AttachmentsPanelProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [filterType, setFilterType] = useState<"all" | "transcript" | "file">("all");

  // Filter and search attachments
  const filteredAttachments = useMemo(() => {
    let filtered = attachments;

    // Apply type filter
    if (filterType !== "all") {
      filtered = filtered.filter(a => a.type === filterType);
    }

    // Apply search
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(a =>
        a.filename.toLowerCase().includes(query) ||
        a.metadata.transcript?.segments.some(s =>
          s.text.toLowerCase().includes(query)
        )
      );
    }

    // Sort by date (newest first)
    return filtered.sort((a, b) => b.createdAt - a.createdAt);
  }, [attachments, searchQuery, filterType]);

  // Group by date
  const groupedAttachments = useMemo(() => {
    const groups: Record<string, Attachment[]> = {};
    const now = Date.now();
    const dayMs = 24 * 60 * 60 * 1000;

    filteredAttachments.forEach(attachment => {
      const daysAgo = Math.floor((now - attachment.createdAt) / dayMs);
      let group: string;

      if (daysAgo === 0) {
        group = "Today";
      } else if (daysAgo === 1) {
        group = "Yesterday";
      } else if (daysAgo < 7) {
        group = "This Week";
      } else if (daysAgo < 30) {
        group = "This Month";
      } else {
        group = "Older";
      }

      if (!groups[group]) {
        groups[group] = [];
      }
      groups[group].push(attachment);
    });

    return groups;
  }, [filteredAttachments]);

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  if (!open) return null;

  return (
    <div className="fixed right-0 top-0 bottom-0 w-80 bg-gray-900 border-l border-white/10 z-40 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-white/10">
        <div className="flex items-center gap-2">
          <FileText className="size-5 text-violet-400" />
          <h2 className="text-lg font-semibold text-white">Attachments</h2>
        </div>
        <button
          onClick={onClose}
          className="p-1 text-gray-400 hover:text-white transition-colors"
        >
          <X className="size-5" />
        </button>
      </div>

      {/* Search and filters */}
      <div className="p-4 border-b border-white/10 space-y-3">
        {/* Search */}
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-gray-500" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search attachments..."
            className="w-full bg-gray-800 border border-white/10 rounded-lg pl-10 pr-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500"
          />
        </div>

        {/* Type filter */}
        <div className="flex gap-2">
          <button
            onClick={() => setFilterType("all")}
            className={`px-3 py-1 text-xs rounded-full transition-colors ${
              filterType === "all"
                ? "bg-violet-600 text-white"
                : "bg-gray-800 text-gray-400 hover:text-white"
            }`}
          >
            All
          </button>
          <button
            onClick={() => setFilterType("transcript")}
            className={`px-3 py-1 text-xs rounded-full transition-colors ${
              filterType === "transcript"
                ? "bg-violet-600 text-white"
                : "bg-gray-800 text-gray-400 hover:text-white"
            }`}
          >
            Transcripts
          </button>
          <button
            onClick={() => setFilterType("file")}
            className={`px-3 py-1 text-xs rounded-full transition-colors ${
              filterType === "file"
                ? "bg-violet-600 text-white"
                : "bg-gray-800 text-gray-400 hover:text-white"
            }`}
          >
            Files
          </button>
        </div>
      </div>

      {/* Attachments list */}
      <div className="flex-1 overflow-y-auto p-4">
        {Object.keys(groupedAttachments).length === 0 ? (
          <div className="text-center py-8">
            <FileText className="size-8 text-gray-600 mx-auto mb-2" />
            <p className="text-sm text-gray-500">
              {searchQuery || filterType !== "all"
                ? "No matching attachments"
                : "No attachments yet"}
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            {Object.entries(groupedAttachments).map(([group, items]) => (
              <div key={group}>
                <h3 className="text-xs font-medium text-gray-400 uppercase tracking-wide mb-2">
                  {group}
                </h3>
                <div className="space-y-2">
                  {items.map((attachment) => (
                    <div
                      key={attachment.id}
                      className="bg-gray-800/50 border border-white/10 rounded-lg p-3 hover:bg-gray-800 transition-colors group"
                    >
                      <div className="flex items-start justify-between mb-2">
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium text-white truncate">
                            {attachment.filename}
                          </p>
                          <p className="text-xs text-gray-500">
                            {attachment.type === "transcript" && attachment.metadata.duration !== undefined
                              ? `${formatTime(attachment.metadata.duration)}  ${attachment.metadata.speakerCount || 1} speaker${(attachment.metadata.speakerCount || 1) !== 1 ? 's' : ''}`
                              : "File attachment"}
                          </p>
                        </div>
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                        <button
                          onClick={() => onView(attachment)}
                          className="flex-1 px-2 py-1 text-xs bg-violet-600/20 text-violet-300 hover:bg-violet-600/30 rounded transition-colors"
                        >
                          View
                        </button>
                        <button
                          onClick={() => onSend(attachment)}
                          className="flex-1 px-2 py-1 text-xs bg-gray-700 text-gray-300 hover:bg-gray-600 rounded transition-colors flex items-center justify-center gap-1"
                        >
                          <Send className="size-3" />
                          Send
                        </button>
                        <button
                          onClick={() => onDelete(attachment.id)}
                          className="p-1 text-gray-500 hover:text-red-400 transition-colors"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-white/10">
        <p className="text-xs text-gray-500 text-center">
          {attachments.length} attachment{attachments.length !== 1 ? 's' : ''}
        </p>
      </div>
    </div>
  );
}
