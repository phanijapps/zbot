// =============================================================================
// artifact-poll — R14d polling helpers split from useResearchSession.
//
// Gateway emits no `artifact_created` WS event, so the hook polls
// `/api/sessions/:id/artifacts` on an interval while the root turn is running
// and once more on transition to `complete`. These pure helpers are tested
// independently so the hook body stays focused on orchestration.
// =============================================================================

import type { Dispatch } from "react";
import { toast } from "sonner";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";
import type { ResearchAction } from "./reducer";
import type { ResearchArtifactRef } from "./types";

/** Poll cadence while a research turn is running. Consumed by the hook effect. */
export const ARTIFACT_POLL_INTERVAL_MS = 5000;

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

/** True when both lists contain the same set of artifact ids (order-insensitive). */
export function sameArtifactIdSet(
  a: ResearchArtifactRef[],
  b: ResearchArtifactRef[]
): boolean {
  if (a.length !== b.length) return false;
  const ids = new Set(a.map((x) => x.id));
  for (const x of b) {
    if (!ids.has(x.id)) return false;
  }
  return true;
}

/**
 * One-shot fetch. Dispatches SET_ARTIFACTS only when the id-set changed (diff
 * check). Updates `latestArtifactsRef` with the full records on success so
 * callers can resolve ref → Artifact without another fetch. Non-throwing:
 * surfaces errors via sonner.
 */
export async function fetchArtifactsOnce(
  sessionId: string,
  currentRefs: ResearchArtifactRef[],
  dispatch: Dispatch<ResearchAction>,
  latestArtifactsRef: { current: Artifact[] }
): Promise<void> {
  try {
    const transport = await getTransport();
    const result = await transport.listSessionArtifacts(sessionId);
    if (!result.success || !result.data) return;
    latestArtifactsRef.current = result.data;
    const nextRefs = result.data.map(toArtifactRef);
    if (!sameArtifactIdSet(currentRefs, nextRefs)) {
      dispatch({ type: "SET_ARTIFACTS", artifacts: nextRefs });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : "unknown";
    toast.error(`Failed to refresh artifacts: ${message}`);
  }
}

/**
 * Start an artifact-poll interval. Fires an immediate tick, then every
 * `ARTIFACT_POLL_INTERVAL_MS`. Returns a teardown function the effect
 * cleanup can call. Moved out of the hook to keep `useResearchSession.ts`
 * focused on orchestration.
 */
export function startArtifactPolling(
  sessionId: string,
  artifactsRef: { current: ResearchArtifactRef[] },
  dispatch: Dispatch<ResearchAction>,
  latestArtifactsRef: { current: Artifact[] }
): () => void {
  const tick = () => {
    void fetchArtifactsOnce(sessionId, artifactsRef.current, dispatch, latestArtifactsRef);
  };
  tick();
  const handle = setInterval(tick, ARTIFACT_POLL_INTERVAL_MS);
  return () => clearInterval(handle);
}
