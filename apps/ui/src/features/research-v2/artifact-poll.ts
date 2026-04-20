// =============================================================================
// artifact-poll — pure helpers kept after the R14f rewrite removed polling.
//
// Under R14f the research hook no longer polls `/api/sessions/:id/artifacts`
// on an interval; snapshotSession() fetches once on open and again on the
// root's `agent_completed`. The `toArtifactRef` mapper and the `fetchArtifactsOnce`
// helper are still used by snapshot and by the slide-out's cache-miss fallback.
//
// The former `startArtifactPolling` + `sameArtifactIdSet` + `ARTIFACT_POLL_INTERVAL_MS`
// exports were deleted together with the timer machinery they supported.
// =============================================================================

import type { Dispatch } from "react";
import { toast } from "sonner";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";
import type { ResearchAction } from "./reducer";
import type { ResearchArtifactRef } from "./types";

/**
 * Pure mapper: full transport `Artifact` → lightweight `ResearchArtifactRef`.
 * Only surfaces the fields the strip needs to render a chip. The full record
 * is kept in the caller's ref for lookups when opening the slide-out.
 */
export function toArtifactRef(a: Artifact): ResearchArtifactRef {
  return {
    id: a.id,
    fileName: a.fileName,
    fileType: a.fileType,
    fileSize: a.fileSize,
    label: a.label,
  };
}

/**
 * One-shot fetch. Dispatches SET_ARTIFACTS unconditionally with the server's
 * list (the reducer's patch is idempotent — re-setting the same refs doesn't
 * cause a re-render because React bails on === state). Updates
 * `latestArtifactsRef` with the full records on success so callers can resolve
 * ref → Artifact without another fetch. Non-throwing: surfaces errors via sonner.
 *
 * Previously this call also diffed the id-set before dispatching; that check is
 * now redundant because snapshotSession() is the only live caller and runs at
 * most twice per session-open (once on hydrate, once on agent_completed).
 */
export async function fetchArtifactsOnce(
  sessionId: string,
  _currentRefs: ResearchArtifactRef[],
  dispatch: Dispatch<ResearchAction>,
  latestArtifactsRef: { current: Artifact[] },
): Promise<void> {
  try {
    const transport = await getTransport();
    const result = await transport.listSessionArtifacts(sessionId);
    if (!result.success || !result.data) return;
    latestArtifactsRef.current = result.data;
    dispatch({ type: "SET_ARTIFACTS", artifacts: result.data.map(toArtifactRef) });
  } catch (err) {
    const message = err instanceof Error ? err.message : "unknown";
    toast.error(`Failed to refresh artifacts: ${message}`);
  }
}
