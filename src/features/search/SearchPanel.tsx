// ============================================================================
// SEARCH PANEL
// Full-text search across all messages with filtering
// ============================================================================

import { useState, useEffect } from "react";
import { Search, Clock, Archive, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { searchMessages, initializeSearchIndex } from "@/services/search";
import type { SearchResult, SearchQuery } from "@/shared/types";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/shared/ui/select";
import { Label } from "@/shared/ui/label";

// Simple relative time formatter (replaces date-fns)
function formatRelativeTime(dateStr: string): string {
  const now = Date.now();
  const date = new Date(dateStr).getTime();
  const diffInSeconds = Math.floor((now - date) / 1000);

  if (diffInSeconds < 60) return "just now";
  if (diffInSeconds < 3600) {
    const mins = Math.floor(diffInSeconds / 60);
    return `${mins}m ago`;
  }
  if (diffInSeconds < 86400) {
    const hours = Math.floor(diffInSeconds / 3600);
    return `${hours}h ago`;
  }
  if (diffInSeconds < 604800) {
    const days = Math.floor(diffInSeconds / 86400);
    return `${days}d ago`;
  }
  if (diffInSeconds < 2592000) {
    const weeks = Math.floor(diffInSeconds / 604800);
    return `${weeks}w ago`;
  }
  const months = Math.floor(diffInSeconds / 2592000);
  return `${months}mo ago`;
}

interface SearchPanelProps {
  onResultClick?: (result: SearchResult) => void;
}

interface Agent {
  id: string;
  name: string;
}

export function SearchPanel({ onResultClick }: SearchPanelProps) {
  const [agents, setAgents] = useState<Array<{ id: string; name: string }>>([]);
  const [query, setQuery] = useState("");
  const [selectedAgentId, setSelectedAgentId] = useState<string | "all">("all");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [initialized, setInitialized] = useState(false);
  const [initializedError, setInitializedError] = useState<string | null>(null);
  const [limit, setLimit] = useState(50);

  // Fetch agents and initialize search index on mount
  useEffect(() => {
    // Fetch agents
    invoke<Agent[]>("list_agents")
      .then((agentList) => {
        // Filter out agent-creator - it's only accessible via + button in agent channels
        setAgents(agentList.filter(a => a.id !== "agent-creator").map(a => ({ id: a.id, name: a.name })));
      })
      .catch((err) => {
        console.error("Failed to load agents:", err);
      });

    // Initialize search index
    initializeSearchIndex()
      .then(() => setInitialized(true))
      .catch((err) => setInitializedError(err instanceof Error ? err.message : String(err)));
  }, []);

  // Debounced search
  useEffect(() => {
    if (!query.trim() || !initialized) {
      setResults([]);
      return;
    }

    const timeoutId = setTimeout(() => {
      performSearch();
    }, 300);

    return () => clearTimeout(timeoutId);
  }, [query, selectedAgentId, limit, initialized]);

  const performSearch = async () => {
    setSearching(true);
    try {
      const searchQuery: SearchQuery = {
        query: query.trim(),
        limit,
      };

      if (selectedAgentId !== "all") {
        searchQuery.agentId = selectedAgentId;
      }

      const searchResults = await searchMessages(searchQuery);
      setResults(searchResults);
    } catch (err) {
      console.error("Search failed:", err);
      setResults([]);
    } finally {
      setSearching(false);
    }
  };

  return (
    <div className="flex flex-col h-full bg-[#141414]">
      {/* Search Header */}
      <div className="border-b border-white/10 p-4">
        <div className="flex items-center gap-2 mb-3">
          <Search className="size-5 text-gray-400" />
          <h2 className="text-lg font-semibold text-white">Search Messages</h2>
        </div>

        {!initialized && !initializedError && (
          <div className="flex items-center gap-2 text-sm text-gray-400 py-2">
            <Loader2 className="size-4 animate-spin" />
            Initializing search index...
          </div>
        )}

        {initializedError && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-3 mb-3">
            <p className="text-sm text-red-200">
              Search index unavailable: {initializedError}
            </p>
          </div>
        )}

        {initialized && (
          <>
            {/* Search Input */}
            <div className="relative mb-3">
              <input
                type="text"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search all messages..."
                className="w-full bg-white/5 border border-white/10 rounded-lg px-4 py-2.5 pl-10 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500/50"
                disabled={!initialized}
              />
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-gray-500" />
            </div>

            {/* Filters */}
            <div className="flex items-center gap-3">
              <div className="flex-1">
                <Label className="text-gray-400 text-xs mb-1 block">Filter by Agent</Label>
                <Select value={selectedAgentId} onValueChange={setSelectedAgentId}>
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-8">
                    <SelectValue placeholder="All Agents" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All Agents</SelectItem>
                    {agents.map((agent) => (
                      <SelectItem key={agent.id} value={agent.id}>
                        {agent.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div>
                <Label className="text-gray-400 text-xs mb-1 block">Results</Label>
                <Select value={String(limit)} onValueChange={(v) => setLimit(Number(v))}>
                  <SelectTrigger className="bg-white/5 border-white/10 text-white h-8 w-[120px]">
                    <SelectValue placeholder="50" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="20">20</SelectItem>
                    <SelectItem value="50">50</SelectItem>
                    <SelectItem value="100">100</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto">
        {searching && (
          <div className="flex items-center justify-center py-12">
            <div className="flex items-center gap-2 text-gray-400">
              <Loader2 className="size-5 animate-spin" />
              <span>Searching...</span>
            </div>
          </div>
        )}

        {!searching && query.trim() && results.length === 0 && initialized && (
          <div className="flex flex-col items-center justify-center py-12 text-gray-500">
            <Search className="size-12 mb-3 opacity-50" />
            <p className="text-lg font-medium">No results found</p>
            <p className="text-sm">Try different keywords or filters</p>
          </div>
        )}

        {!searching && !query.trim() && (
          <div className="flex flex-col items-center justify-center py-12 text-gray-500">
            <Search className="size-12 mb-3 opacity-50" />
            <p className="text-lg font-medium">Search your messages</p>
            <p className="text-sm">Type to search across all conversations</p>
          </div>
        )}

        {!searching && results.length > 0 && (
          <div className="p-4 space-y-3">
            <p className="text-sm text-gray-400 px-1">
              Found {results.length} result{results.length !== 1 ? "s" : ""}
            </p>

            {results.map((result) => (
              <SearchResultItem
                key={result.messageId}
                result={result}
                query={query}
                onClick={() => onResultClick?.(result)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface SearchResultItemProps {
  result: SearchResult;
  query: string;
  onClick: () => void;
}

function SearchResultItem({ result, query, onClick }: SearchResultItemProps) {
  // Highlight matching text
  const highlightMatch = (text: string, query: string) => {
    if (!query.trim()) return text;

    const regex = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
    const parts = text.split(regex);

    return parts.map((part, i) =>
      regex.test(part) ? (
        <mark key={i} className="bg-yellow-500/30 text-yellow-200 rounded px-0.5">
          {part}
        </mark>
      ) : (
        part
      )
    );
  };

  const getSourceIcon = () => {
    if (result.source.type === "parquet") {
      return <Archive className="size-3 text-yellow-400" />;
    }
    return null;
  };

  return (
    <button
      onClick={onClick}
      className="w-full text-left bg-white/5 hover:bg-white/10 border border-white/10 hover:border-white/20 rounded-lg p-4 transition-colors"
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex items-center gap-2">
          <span className="font-medium text-white text-sm">
            {result.agentName}
          </span>
          <span className="text-gray-500">•</span>
          <span className="text-xs text-gray-400 capitalize">
            {result.role}
          </span>
          {getSourceIcon()}
        </div>
        <div className="flex items-center gap-1 text-xs text-gray-500">
          <Clock className="size-3" />
          <span>{formatRelativeTime(result.createdAt)}</span>
        </div>
      </div>

      {/* Content */}
      <div className="text-sm text-gray-300 line-clamp-3">
        {highlightMatch(result.content, query)}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between mt-2 pt-2 border-t border-white/5">
        <div className="text-xs text-gray-500">
          Score: {result.score.toFixed(2)}
        </div>
        {result.source.type === "parquet" && (
          <div className="text-xs text-yellow-400/80">
            Archived
          </div>
        )}
      </div>
    </button>
  );
}
