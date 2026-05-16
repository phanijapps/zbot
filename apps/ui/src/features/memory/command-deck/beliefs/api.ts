// ============================================================================
// Belief Network API service — fetch wrappers over the HTTP endpoints
// defined in `gateway/src/http/beliefs.rs`.
//
// We deliberately keep this off the main `Transport` interface to avoid
// surface-area churn — Belief Network is opt-in, and shipping a tiny
// dedicated service keeps the failure mode (503 when disabled) local to
// the UI surfaces that consume it.
// ============================================================================

import { getTransport } from "@/services/transport";
import type {
  Belief,
  BeliefContradiction,
  BeliefDetailResponse,
  BeliefListResponse,
  ContradictionListResponse,
  ContradictionResolution,
} from "../types.beliefs";

export interface ApiResult<T> {
  success: boolean;
  data?: T;
  error?: string;
  /** Distinguishes "feature disabled" (503) from any other failure. */
  disabled?: boolean;
}

/** Returns the gateway's HTTP base URL from the global transport. */
async function httpBase(): Promise<string> {
  const transport = (await getTransport()) as unknown as {
    config?: { httpUrl?: string };
  };
  return transport.config?.httpUrl ?? "";
}

async function fetchJson<T>(
  path: string,
  init?: RequestInit,
): Promise<ApiResult<T>> {
  try {
    const base = await httpBase();
    const response = await fetch(`${base}${path}`, {
      ...init,
      headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
    });
    if (response.status === 503) {
      const body = await safeReadError(response);
      return { success: false, disabled: true, error: body };
    }
    if (!response.ok) {
      const body = await safeReadError(response);
      return {
        success: false,
        error: body || `HTTP ${response.status}: ${response.statusText}`,
      };
    }
    if (response.status === 204) {
      return { success: true };
    }
    const data = (await response.json()) as T;
    return { success: true, data };
  } catch (err) {
    return { success: false, error: err instanceof Error ? err.message : String(err) };
  }
}

async function safeReadError(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string };
    return body.error ?? "";
  } catch {
    return "";
  }
}

export async function listBeliefs(
  agentId: string,
  limit = 50,
  offset = 0,
): Promise<ApiResult<Belief[]>> {
  const params = new URLSearchParams({
    limit: String(limit),
    offset: String(offset),
  });
  const res = await fetchJson<BeliefListResponse>(
    `/api/beliefs/${encodeURIComponent(agentId)}?${params.toString()}`,
  );
  if (!res.success || !res.data) {
    return { success: res.success, error: res.error, disabled: res.disabled };
  }
  return { success: true, data: res.data.beliefs };
}

export async function getBeliefDetail(
  agentId: string,
  beliefId: string,
): Promise<ApiResult<BeliefDetailResponse>> {
  return fetchJson<BeliefDetailResponse>(
    `/api/beliefs/${encodeURIComponent(agentId)}/${encodeURIComponent(beliefId)}`,
  );
}

export async function listContradictions(
  agentId: string,
  limit = 20,
): Promise<ApiResult<BeliefContradiction[]>> {
  const params = new URLSearchParams({ limit: String(limit) });
  const res = await fetchJson<ContradictionListResponse>(
    `/api/contradictions/${encodeURIComponent(agentId)}?${params.toString()}`,
  );
  if (!res.success || !res.data) {
    return { success: res.success, error: res.error, disabled: res.disabled };
  }
  return { success: true, data: res.data.contradictions };
}

export async function resolveContradiction(
  contradictionId: string,
  resolution: ContradictionResolution,
): Promise<ApiResult<void>> {
  if (resolution === "unresolved") {
    return {
      success: false,
      error: "Cannot resolve a contradiction to 'unresolved'",
    };
  }
  return fetchJson<void>(
    `/api/contradictions/${encodeURIComponent(contradictionId)}/resolve`,
    {
      method: "POST",
      body: JSON.stringify({ resolution }),
    },
  );
}
