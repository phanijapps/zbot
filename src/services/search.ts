// ============================================================================
// SEARCH SERVICE
// API wrapper for search functionality
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type {
  SearchResult,
  SearchQuery,
  IndexedDocument,
} from "@/shared/types";

/**
 * Initialize search index for active vault
 */
export async function initializeSearchIndex(): Promise<void> {
  return invoke<void>("initialize_search_index");
}

/**
 * Search messages across active and archived
 */
export async function searchMessages(
  query: SearchQuery
): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_messages", { query });
}

/**
 * Index a new message (called when message is created)
 */
export async function indexMessage(doc: IndexedDocument): Promise<void> {
  return invoke<void>("index_message", { doc });
}

/**
 * Batch index multiple messages
 */
export async function indexMessages(docs: IndexedDocument[]): Promise<void> {
  return invoke<void>("index_messages", { docs });
}

/**
 * Rebuild index from scratch
 */
export async function rebuildSearchIndex(): Promise<string> {
  return invoke<string>("rebuild_search_index");
}

/**
 * Delete messages from search index by session
 */
export async function deleteSessionFromIndex(
  sessionId: string
): Promise<number> {
  return invoke<number>("delete_session_from_index", { sessionId });
}

/**
 * Delete messages from search index by agent
 */
export async function deleteAgentFromIndex(agentId: string): Promise<number> {
  return invoke<number>("delete_agent_from_index", { agentId });
}

/**
 * Clear search index
 */
export async function clearSearchIndex(): Promise<void> {
  return invoke<void>("clear_search_index");
}
