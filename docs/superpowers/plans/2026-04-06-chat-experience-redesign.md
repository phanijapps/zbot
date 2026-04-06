# Chat Experience Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace fragile log-based session reconstruction with a backend Session State API, and redesign the chat center panel to show only user messages, phase indicators, and responses — moving all execution detail to the sidebar.

**Architecture:** Backend assembles a structured `SessionState` snapshot from existing DB tables (execution_logs, messages, agent_executions). Frontend fetches this snapshot on load/reconnect and renders it directly. Live events update state via a phase state machine. Sidebar receives subagent tool calls inline under each subagent card.

**Tech Stack:** Rust (axum, serde_json, SQLite via api-logs), TypeScript/React (transport layer, React hooks, CSS)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `gateway/gateway-execution/src/session_state.rs` | **New** — `SessionStateBuilder`: queries DB, assembles `SessionState` |
| `gateway/src/http/sessions.rs` | Add `get_session_state` handler |
| `gateway/src/http/mod.rs` | Register route |
| `apps/ui/src/services/transport/types.ts` | Add `SessionState`, `SubagentState`, `ToolCallEntry` types |
| `apps/ui/src/services/transport/interface.ts` | Add `getSessionState()` method |
| `apps/ui/src/services/transport/http.ts` | Implement `getSessionState()` |
| `apps/ui/src/features/chat/PhaseIndicators.tsx` | **New** — 4-phase progress component |
| `apps/ui/src/features/chat/mission-hooks.ts` | Rewrite: snapshot hydration, phase state machine, separated sidebar state |
| `apps/ui/src/features/chat/MissionControl.tsx` | Redesign center: user message → phases → response |
| `apps/ui/src/features/chat/ExecutionNarrative.tsx` | Simplify: only user messages, phases, responses |
| `apps/ui/src/features/chat/IntelligenceFeed.tsx` | Upgrade: accept `SubagentState[]` with inline tool calls |
| `apps/ui/src/features/chat/index.ts` | Update exports |

---

## Task 1: Create `SessionStateBuilder` in Backend

**Files:**
- Create: `gateway/gateway-execution/src/session_state.rs`
- Modify: `gateway/gateway-execution/src/lib.rs` (add `pub mod session_state;` and re-export)
- Modify: `gateway/gateway-execution/Cargo.toml` (if new deps needed — likely none)

- [ ] **Step 1: Define the response types**

Create `gateway/gateway-execution/src/session_state.rs`:

```rust
//! Session State Builder
//! Assembles a structured snapshot of a session from existing DB tables.

use std::sync::Arc;
use api_logs::{LogService, ExecutionLog, LogCategory, SessionStatus};
use gateway_database::{ConversationRepository, DatabaseManager};
use serde::Serialize;

/// Complete renderable snapshot of a session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session: SessionMeta,
    pub user_message: Option<String>,
    pub phase: SessionPhase,
    pub response: Option<String>,
    pub intent_analysis: Option<serde_json::Value>,
    pub ward: Option<WardInfo>,
    pub recalled_facts: Vec<serde_json::Value>,
    pub plan: Vec<PlanStep>,
    pub subagents: Vec<SubagentState>,
    pub is_live: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub title: Option<String>,
    pub status: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub token_count: i32,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionPhase {
    #[serde(rename = "intent")]
    Intent,
    #[serde(rename = "planning")]
    Planning,
    #[serde(rename = "executing")]
    Executing,
    #[serde(rename = "responding")]
    Responding,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardInfo {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub text: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentState {
    pub agent_id: String,
    pub execution_id: String,
    pub task: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub token_count: Option<i32>,
    pub tool_calls: Vec<ToolCallEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallEntry {
    pub tool_name: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub summary: Option<String>,
}
```

- [ ] **Step 2: Implement the builder**

Add the builder to the same file, after the types:

```rust
/// Builds a SessionState from existing DB data.
pub struct SessionStateBuilder {
    log_service: Arc<LogService<DatabaseManager>>,
    conversations: Arc<ConversationRepository>,
}

impl SessionStateBuilder {
    pub fn new(
        log_service: Arc<LogService<DatabaseManager>>,
        conversations: Arc<ConversationRepository>,
    ) -> Self {
        Self { log_service, conversations }
    }

    /// Build the full session state for the given session ID.
    pub fn build(&self, session_id: &str) -> Result<SessionState, String> {
        // 1. Get session detail (session metadata + all logs)
        let detail = self.log_service
            .get_session_detail(session_id)
            .map_err(|e| format!("Failed to load session: {}", e))?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let session = &detail.session;
        let logs = &detail.logs;

        // 2. Extract user message from conversation messages
        let user_message = self.extract_user_message(session.conversation_id.as_str());

        // 3. Extract sidebar data from logs
        let intent_analysis = self.extract_intent(logs);
        let ward = self.extract_ward(logs, &intent_analysis);
        let recalled_facts = self.extract_recalled_facts(logs);
        let plan = self.extract_plan(logs);
        let response = self.extract_response(logs, session.conversation_id.as_str());

        // 4. Build subagent states from child sessions
        let subagents = self.build_subagents(session_id, &session.child_session_ids);

        // 5. Derive phase
        let phase = self.derive_phase(session.status.clone(), logs, &response);

        let is_live = matches!(session.status, SessionStatus::Running);

        Ok(SessionState {
            session: SessionMeta {
                id: session.session_id.clone(),
                title: session.title.clone(),
                status: serde_json::to_value(&session.status)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "running".to_string()),
                started_at: session.started_at.clone(),
                duration_ms: session.duration_ms.unwrap_or(0),
                token_count: session.token_count,
                model: None, // Extracted from agent_started log if needed
            },
            user_message,
            phase,
            response,
            intent_analysis,
            ward,
            recalled_facts,
            plan,
            subagents,
            is_live,
        })
    }

    fn extract_user_message(&self, conversation_id: &str) -> Option<String> {
        let messages = self.conversations.get_messages(conversation_id).ok()?;
        messages.iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
    }

    fn extract_intent(&self, logs: &[ExecutionLog]) -> Option<serde_json::Value> {
        logs.iter()
            .find(|l| matches!(l.category, LogCategory::Intent))
            .and_then(|l| l.metadata.clone())
    }

    fn extract_ward(&self, logs: &[ExecutionLog], intent: &Option<serde_json::Value>) -> Option<WardInfo> {
        // Check for explicit ward_changed tool call
        for log in logs.iter().rev() {
            if matches!(log.category, LogCategory::ToolCall) {
                if let Some(meta) = &log.metadata {
                    let tool = meta.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                    if tool == "ward" || tool == "set_ward" || tool == "enter_ward" {
                        let args = meta.get("args").cloned().unwrap_or_default();
                        let name = args.get("name")
                            .or_else(|| args.get("ward_name"))
                            .or_else(|| args.get("ward_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() {
                            return Some(WardInfo { name, content: String::new() });
                        }
                    }
                }
            }
        }
        // Fallback: extract from intent analysis ward_recommendation
        intent.as_ref().and_then(|ia| {
            let wr = ia.get("ward_recommendation")?;
            let name = wr.get("ward_name").and_then(|v| v.as_str())?.to_string();
            let content = wr.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Some(WardInfo { name, content })
        })
    }

    fn extract_recalled_facts(&self, logs: &[ExecutionLog]) -> Vec<serde_json::Value> {
        for log in logs {
            if matches!(log.category, LogCategory::ToolResult) {
                if let Some(meta) = &log.metadata {
                    let tool = meta.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                    if tool == "memory" {
                        // Try to parse the result as JSON with a facts/results array
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&log.message) {
                            let facts = parsed.get("results")
                                .or_else(|| parsed.get("facts"))
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            if !facts.is_empty() {
                                return facts;
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    }

    fn extract_plan(&self, logs: &[ExecutionLog]) -> Vec<PlanStep> {
        // Find the LAST update_plan tool call (plan gets replaced each time)
        for log in logs.iter().rev() {
            if matches!(log.category, LogCategory::ToolCall) {
                if let Some(meta) = &log.metadata {
                    let tool = meta.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                    if tool == "update_plan" {
                        let args = meta.get("args").cloned().unwrap_or_default();
                        let raw_steps = args.get("steps")
                            .or_else(|| args.get("plan"))
                            .or_else(|| args.get("content"));
                        if let Some(steps_val) = raw_steps {
                            if let Some(arr) = steps_val.as_array() {
                                return arr.iter().filter_map(|s| {
                                    if let Some(text) = s.as_str() {
                                        Some(PlanStep { text: text.to_string(), status: "pending".to_string() })
                                    } else if let Some(obj) = s.as_object() {
                                        let text = obj.get("text")
                                            .or_else(|| obj.get("step"))
                                            .or_else(|| obj.get("description"))
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let status = obj.get("status")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("pending")
                                            .to_string();
                                        Some(PlanStep { text, status })
                                    } else {
                                        None
                                    }
                                }).collect();
                            }
                        }
                    }
                }
            }
        }
        Vec::new()
    }

    fn extract_response(&self, logs: &[ExecutionLog], conversation_id: &str) -> Option<String> {
        // Primary: respond tool call args.message
        for log in logs.iter().rev() {
            if matches!(log.category, LogCategory::ToolCall) {
                if let Some(meta) = &log.metadata {
                    let tool = meta.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                    if tool == "respond" {
                        let args = meta.get("args").cloned().unwrap_or_default();
                        if let Some(msg) = args.get("message").and_then(|v| v.as_str()) {
                            return Some(msg.to_string());
                        }
                    }
                }
            }
        }
        // Fallback: last assistant message from conversation
        if let Ok(messages) = self.conversations.get_messages(conversation_id) {
            if let Some(msg) = messages.iter().rev().find(|m| m.role == "assistant" && !m.content.is_empty()) {
                return Some(msg.content.clone());
            }
        }
        None
    }

    fn build_subagents(&self, _parent_session_id: &str, child_session_ids: &[String]) -> Vec<SubagentState> {
        child_session_ids.iter().filter_map(|child_id| {
            let detail = self.log_service.get_session_detail(child_id).ok()??;
            let child = &detail.session;

            // Extract tool calls from child's logs
            let tool_calls: Vec<ToolCallEntry> = detail.logs.iter().filter_map(|log| {
                if !matches!(log.category, LogCategory::ToolCall) { return None; }
                let meta = log.metadata.as_ref()?;
                let tool_name = meta.get("tool_name").and_then(|v| v.as_str())?.to_string();
                // Skip internal tools
                if tool_name == "set_session_title" || tool_name == "update_plan" { return None; }

                Some(ToolCallEntry {
                    tool_name,
                    status: "completed".to_string(), // Will refine below
                    duration_ms: log.duration_ms,
                    summary: None,
                })
            }).collect();

            // Match tool_results to update summaries
            let mut calls_with_results = tool_calls;
            let mut result_idx = 0;
            for log in &detail.logs {
                if matches!(log.category, LogCategory::ToolResult) && result_idx < calls_with_results.len() {
                    calls_with_results[result_idx].summary = Some(
                        log.message.chars().take(100).collect()
                    );
                    if log.level == api_logs::LogLevel::Error {
                        calls_with_results[result_idx].status = "error".to_string();
                    }
                    result_idx += 1;
                }
            }

            // Extract task from delegation log
            let task = detail.logs.iter()
                .find(|l| matches!(l.category, LogCategory::Delegation))
                .map(|l| l.message.chars().take(200).collect::<String>())
                .unwrap_or_default();

            let status = match child.status {
                SessionStatus::Running => "running",
                SessionStatus::Completed => "completed",
                SessionStatus::Error => "error",
                SessionStatus::Stopped => "completed",
            };

            Some(SubagentState {
                agent_id: child.agent_id.clone(),
                execution_id: child.session_id.clone(),
                task,
                status: status.to_string(),
                duration_ms: child.duration_ms,
                token_count: Some(child.token_count),
                tool_calls: calls_with_results,
            })
        }).collect()
    }

    fn derive_phase(&self, status: SessionStatus, logs: &[ExecutionLog], response: &Option<String>) -> SessionPhase {
        match status {
            SessionStatus::Completed | SessionStatus::Stopped => return SessionPhase::Completed,
            SessionStatus::Error => return SessionPhase::Error,
            _ => {}
        }

        if response.is_some() {
            return SessionPhase::Responding;
        }

        let has_delegation = logs.iter().any(|l| matches!(l.category, LogCategory::Delegation));
        let has_non_internal_tools = logs.iter().any(|l| {
            if !matches!(l.category, LogCategory::ToolCall) { return false; }
            let tool = l.metadata.as_ref()
                .and_then(|m| m.get("tool_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            tool != "analyze_intent" && tool != "update_plan" && tool != "set_session_title"
        });

        if has_delegation || has_non_internal_tools {
            return SessionPhase::Executing;
        }

        let has_plan = logs.iter().any(|l| {
            matches!(l.category, LogCategory::ToolCall) &&
            l.metadata.as_ref()
                .and_then(|m| m.get("tool_name"))
                .and_then(|v| v.as_str()) == Some("update_plan")
        });
        if has_plan {
            return SessionPhase::Planning;
        }

        SessionPhase::Intent
    }
}
```

- [ ] **Step 3: Export from gateway-execution**

Add to `gateway/gateway-execution/src/lib.rs`:

```rust
pub mod session_state;
```

And add to the re-exports section:

```rust
pub use session_state::{SessionStateBuilder, SessionState};
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: No errors (warnings OK).

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/session_state.rs gateway/gateway-execution/src/lib.rs
git commit -m "feat(backend): add SessionStateBuilder for session state API"
```

---

## Task 2: Add HTTP Handler and Route

**Files:**
- Modify: `gateway/src/http/sessions.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add the handler**

In `gateway/src/http/sessions.rs`, add the handler function:

```rust
use gateway_execution::{SessionStateBuilder, SessionState};

/// GET /api/sessions/:id/state — returns structured session snapshot
pub async fn get_session_state(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionState>, (StatusCode, Json<SessionErrorResponse>)> {
    let builder = SessionStateBuilder::new(
        state.log_service.clone(),
        state.conversations.clone(),
    );

    match builder.build(&session_id) {
        Ok(session_state) => Ok(Json(session_state)),
        Err(e) => {
            if e.contains("not found") {
                Err((StatusCode::NOT_FOUND, Json(SessionErrorResponse { error: e })))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(SessionErrorResponse { error: e })))
            }
        }
    }
}
```

- [ ] **Step 2: Register the route**

In `gateway/src/http/mod.rs`, find where the sessions routes are registered and add:

```rust
.route("/api/sessions/:id/state", get(sessions::get_session_state))
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p gateway`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add gateway/src/http/sessions.rs gateway/src/http/mod.rs
git commit -m "feat(api): add GET /api/sessions/:id/state endpoint"
```

---

## Task 3: Add TypeScript Types and Transport Method

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`
- Modify: `apps/ui/src/services/transport/interface.ts`
- Modify: `apps/ui/src/services/transport/http.ts`

- [ ] **Step 1: Add types**

In `apps/ui/src/services/transport/types.ts`, add after `DistillationConfig`:

```typescript
// ============================================================================
// Session State (snapshot API for reconnection)
// ============================================================================

export interface SessionState {
  session: {
    id: string;
    title: string | null;
    status: "running" | "completed" | "error" | "stopped";
    startedAt: string;
    durationMs: number;
    tokenCount: number;
    model: string | null;
  };
  userMessage: string | null;
  phase: "intent" | "planning" | "executing" | "responding" | "completed" | "error";
  response: string | null;
  intentAnalysis: IntentAnalysisData | null;
  ward: { name: string; content: string } | null;
  recalledFacts: RecalledFactData[];
  plan: PlanStepData[];
  subagents: SubagentStateData[];
  isLive: boolean;
}

export interface IntentAnalysisData {
  primary_intent?: string;
  hidden_intents?: string[];
  recommended_skills?: string[];
  recommended_agents?: string[];
  ward_recommendation?: {
    action?: string;
    ward_name?: string;
    subdirectory?: string;
    reason?: string;
  };
  execution_strategy?: {
    approach?: string;
    graph?: {
      nodes?: Array<{ id: string; task: string; agent: string; skills: string[] }>;
      mermaid?: string;
    };
    explanation?: string;
  };
}

export interface RecalledFactData {
  key?: string;
  content?: string;
  category?: string;
  confidence?: number;
  score?: number;
}

export interface PlanStepData {
  text: string;
  status: string;
}

export interface SubagentStateData {
  agentId: string;
  executionId: string;
  task: string;
  status: "queued" | "running" | "completed" | "error";
  durationMs: number | null;
  tokenCount: number | null;
  toolCalls: ToolCallEntryData[];
}

export interface ToolCallEntryData {
  toolName: string;
  status: "running" | "completed" | "error";
  durationMs: number | null;
  summary: string | null;
}
```

- [ ] **Step 2: Add to Transport interface**

In `apps/ui/src/services/transport/interface.ts`, add to the Transport interface:

```typescript
  /** Get structured session state snapshot for reconnection */
  getSessionState(sessionId: string): Promise<TransportResult<SessionState>>;
```

- [ ] **Step 3: Implement in HTTP transport**

In `apps/ui/src/services/transport/http.ts`, add the implementation:

```typescript
  async getSessionState(sessionId: string): Promise<TransportResult<SessionState>> {
    return this.fetchJson<SessionState>(`/api/sessions/${encodeURIComponent(sessionId)}/state`);
  }
```

- [ ] **Step 4: Type check**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/services/transport/types.ts apps/ui/src/services/transport/interface.ts apps/ui/src/services/transport/http.ts
git commit -m "feat(ui): add SessionState types and getSessionState transport method"
```

---

## Task 4: Create PhaseIndicators Component

**Files:**
- Create: `apps/ui/src/features/chat/PhaseIndicators.tsx`

- [ ] **Step 1: Create the component**

```tsx
// ============================================================================
// PHASE INDICATORS
// Shows 4-phase execution progress in the center panel.
// ============================================================================

import type { SubagentStateData } from "@/services/transport/types";

export type Phase = "intent" | "planning" | "executing" | "responding" | "completed" | "error";

interface PhaseIndicatorsProps {
  phase: Phase;
  subagents?: SubagentStateData[];
}

interface PhaseStep {
  key: Phase;
  label: string;
  getDetail?: (props: PhaseIndicatorsProps) => string | null;
}

const STEPS: PhaseStep[] = [
  { key: "intent", label: "Analyzing intent" },
  {
    key: "planning",
    label: "Planning execution",
    getDetail: (p) => {
      const count = p.subagents?.length ?? 0;
      return count > 0 ? `${count} agent${count > 1 ? "s" : ""}` : null;
    },
  },
  {
    key: "executing",
    label: "Executing",
    getDetail: (p) => {
      const agents = p.subagents ?? [];
      if (agents.length === 0) return null;
      const done = agents.filter((a) => a.status === "completed").length;
      const active = agents.filter((a) => a.status === "running").map((a) => a.agentId);
      const parts: string[] = [];
      if (active.length > 0) parts.push(active.join(", "));
      parts.push(`(${done}/${agents.length} complete)`);
      return parts.join(" ");
    },
  },
  { key: "responding", label: "Generating response" },
];

const PHASE_ORDER: Phase[] = ["intent", "planning", "executing", "responding", "completed"];

function getStepStatus(step: Phase, currentPhase: Phase): "done" | "active" | "pending" | "error" {
  if (currentPhase === "error") {
    const ci = PHASE_ORDER.indexOf(currentPhase);
    const si = PHASE_ORDER.indexOf(step);
    // Mark phases before error as done, the rest as error/pending
    if (si < ci) return "done";
    if (si === ci) return "error";
    return "pending";
  }
  if (currentPhase === "completed") return "done";
  const currentIdx = PHASE_ORDER.indexOf(currentPhase);
  const stepIdx = PHASE_ORDER.indexOf(step);
  if (stepIdx < currentIdx) return "done";
  if (stepIdx === currentIdx) return "active";
  return "pending";
}

export function PhaseIndicators({ phase, subagents }: PhaseIndicatorsProps) {
  return (
    <div className="phase-indicators">
      <div className="phase-indicators__label">Execution Progress</div>
      <div className="phase-indicators__steps">
        {STEPS.map((step) => {
          const status = getStepStatus(step.key, phase);
          const detail = step.getDetail?.({ phase, subagents });
          return (
            <div key={step.key} className={`phase-step phase-step--${status}`}>
              <div className={`phase-step__icon phase-step__icon--${status}`}>
                {status === "done" && <span>&#x2713;</span>}
                {status === "active" && <span className="phase-step__pulse" />}
                {status === "error" && <span>&#x2717;</span>}
              </div>
              <span className="phase-step__label">
                {step.label}
                {detail && <span className="phase-step__detail"> — {detail}</span>}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS**

Add to `apps/ui/src/styles/components.css` (find the chat section):

```css
/* Phase Indicators */
.phase-indicators { padding: var(--spacing-4); background: var(--muted); border-radius: var(--radius-lg); border: 1px solid var(--border); margin-bottom: var(--spacing-4); }
.phase-indicators__label { font-size: var(--font-size-xs); color: var(--muted-foreground); text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: var(--spacing-3); }
.phase-indicators__steps { display: flex; flex-direction: column; gap: var(--spacing-2); }
.phase-step { display: flex; align-items: center; gap: var(--spacing-2); }
.phase-step__icon { width: 22px; height: 22px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 11px; flex-shrink: 0; }
.phase-step__icon--done { background: var(--success); color: white; }
.phase-step__icon--active { background: var(--primary); }
.phase-step__icon--pending { border: 1.5px solid var(--border); }
.phase-step__icon--error { background: var(--destructive); color: white; }
.phase-step__pulse { width: 8px; height: 8px; border-radius: 50%; background: white; animation: phase-pulse 1.5s infinite; }
.phase-step__label { font-size: var(--font-size-sm); color: var(--muted-foreground); }
.phase-step--active .phase-step__label { color: var(--foreground); }
.phase-step--pending .phase-step__label { color: var(--muted-foreground); opacity: 0.5; }
.phase-step__detail { color: var(--muted-foreground); }
@keyframes phase-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
```

- [ ] **Step 3: Export from index.ts**

Add to `apps/ui/src/features/chat/index.ts`:

```typescript
export { PhaseIndicators } from "./PhaseIndicators";
export type { Phase } from "./PhaseIndicators";
```

- [ ] **Step 4: Type check and build**

Run: `cd apps/ui && npx tsc --noEmit && npm run build`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat/PhaseIndicators.tsx apps/ui/src/styles/components.css apps/ui/src/features/chat/index.ts
git commit -m "feat(ui): add PhaseIndicators component for center panel"
```

---

## Task 5: Rewrite `mission-hooks.ts` — Snapshot Hydration + Phase State Machine

**Files:**
- Modify: `apps/ui/src/features/chat/mission-hooks.ts`

This is the largest task. The key changes:
1. Replace `loadSession()` with `getSessionState()` call
2. Add `phase` state and phase transition logic in `handleStreamEvent`
3. Separate sidebar state from narrative blocks
4. Keep live event handling for streaming tokens and tool routing

- [ ] **Step 1: Add phase state and snapshot hydration**

Replace the state declaration section at the top of `useMissionControl()` to add `phase`:

```typescript
const [phase, setPhase] = useState<Phase>("idle");
```

Where `Phase` is imported from `./PhaseIndicators`.

Add the phase to the returned `MissionState` interface:

```typescript
export interface MissionState {
  blocks: NarrativeBlock[];
  sessionTitle: string;
  status: "idle" | "running" | "completed" | "error";
  phase: Phase;
  tokenCount: number;
  durationMs: number;
  modelName: string;
  subagents: SubagentInfo[];
  plan: PlanStep[];
  recalledFacts: RecalledFact[];
  activeWard: { name: string; content: string } | null;
  intentAnalysis: IntentAnalysis | null;
}
```

- [ ] **Step 2: Replace `loadSession()` with snapshot-based hydration**

Replace the entire `loadSession()` function (lines ~953-1222) with:

```typescript
    const loadSession = async () => {
      try {
        const transport = await getTransport();
        const logSessionId = localStorage.getItem("agentzero_log_session_id") || activeSessionId;
        if (!logSessionId) return;

        const res = await transport.getSessionState(logSessionId);
        if (!res.success || !res.data) {
          // Fallback: try old log-based loading for very old sessions
          console.warn("[MissionControl] Session state API failed, session may be too old");
          return;
        }

        const s = res.data;

        // Session bar
        if (s.session.title) setSessionTitle(s.session.title);
        setTokenCount(s.session.tokenCount);
        setDurationMs(s.session.durationMs);

        // Status
        const statusMap: Record<string, "idle" | "running" | "completed" | "error"> = {
          running: "running", completed: "completed", error: "error", stopped: "completed",
        };
        setStatus(statusMap[s.session.status] ?? "completed");
        if (s.session.status === "running") startDurationTimer();

        // Phase
        setPhase(s.phase as Phase);

        // Sidebar
        if (s.intentAnalysis) {
          setIntentAnalysis({
            primaryIntent: s.intentAnalysis.primary_intent ?? "",
            hiddenIntents: s.intentAnalysis.hidden_intents ?? [],
            recommendedSkills: s.intentAnalysis.recommended_skills ?? [],
            recommendedAgents: s.intentAnalysis.recommended_agents ?? [],
            wardRecommendation: {
              action: s.intentAnalysis.ward_recommendation?.action ?? "",
              wardName: s.intentAnalysis.ward_recommendation?.ward_name ?? "",
              subdirectory: s.intentAnalysis.ward_recommendation?.subdirectory,
              reason: s.intentAnalysis.ward_recommendation?.reason ?? "",
            },
            executionStrategy: {
              approach: s.intentAnalysis.execution_strategy?.approach ?? "simple",
              graph: s.intentAnalysis.execution_strategy?.graph,
              explanation: s.intentAnalysis.execution_strategy?.explanation ?? "",
            },
          });
        }
        if (s.ward) setActiveWard(s.ward);
        if (s.recalledFacts.length > 0) {
          setRecalledFacts(s.recalledFacts.map((f) => ({
            key: (f.key ?? "") as string,
            content: (f.content ?? f.text ?? "") as string,
            category: (f.category ?? "") as string,
            confidence: (f.confidence ?? f.score) as number | undefined,
          })));
        }
        if (s.plan.length > 0) {
          setPlan(s.plan.map((p) => ({
            text: p.text,
            status: (p.status === "completed" ? "done" : p.status === "in_progress" ? "active" : "pending") as "done" | "active" | "pending",
          })));
        }
        if (s.subagents.length > 0) {
          setSubagents(s.subagents.map((sa) => ({
            agentId: sa.agentId,
            task: sa.task,
            status: sa.status === "running" ? "active" : sa.status as "active" | "completed" | "error",
            executionId: sa.executionId,
            toolCalls: sa.toolCalls,
            durationMs: sa.durationMs,
            tokenCount: sa.tokenCount,
          })));
          // Seed the execution→agent map for live event routing
          for (const sa of s.subagents) {
            executionAgentMapRef.current.set(sa.executionId, sa.agentId);
          }
        }

        // Center: build blocks from snapshot
        const loadedBlocks: NarrativeBlock[] = [];

        if (s.userMessage) {
          loadedBlocks.push({
            id: "user-" + logSessionId,
            type: "user",
            timestamp: s.session.startedAt,
            data: { content: s.userMessage, timestamp: s.session.startedAt },
          });
        }

        if (s.response) {
          loadedBlocks.push({
            id: "response-" + logSessionId,
            type: "response",
            timestamp: s.session.startedAt,
            data: { content: s.response, timestamp: s.session.startedAt },
            isStreaming: false,
          });
        }

        if (loadedBlocks.length > 0) setBlocks(loadedBlocks);

        // Reconnect to live session via WebSocket
        if (s.isLive) {
          // Subscribe will be handled by the existing useEffect that watches activeSessionId
        }
      } catch (err) {
        console.error("[MissionControl] Failed to load session:", err);
      } finally {
        localStorage.removeItem("agentzero_log_session_id");
      }
    };
```

- [ ] **Step 3: Add phase transitions to `handleStreamEvent`**

In the existing `handleStreamEvent` function, add phase transitions at the appropriate event handlers:

After `intent_analysis_complete` handling:
```typescript
setPhase("planning");
```

After `tool_call` handling for `delegate_to_agent`:
```typescript
setPhase((prev) => prev === "planning" || prev === "intent" ? "executing" : prev);
```

After `tool_call` handling for `respond`:
```typescript
setPhase("responding");
```

After first `token` event (when creating a new response block):
```typescript
setPhase((prev) => prev !== "responding" && prev !== "completed" ? "responding" : prev);
```

After `agent_completed`:
```typescript
setPhase("completed");
```

After `error`:
```typescript
setPhase("error");
```

On `sendMessage`, reset phase:
```typescript
setPhase("intent");
```

- [ ] **Step 4: Add `executionAgentMapRef` for subagent tool routing**

Add a new ref:
```typescript
const executionAgentMapRef = useRef<Map<string, string>>(new Map());
```

On `delegation_started` events, populate it:
```typescript
executionAgentMapRef.current.set(childExecutionId, childAgentId);
```

On `tool_call` / `tool_result` events, use the map to route to the correct subagent in sidebar state:
```typescript
const agentId = executionAgentMapRef.current.get(event.execution_id);
if (agentId) {
  // Update the matching subagent's toolCalls array
  setSubagents((prev) => prev.map((sa) =>
    sa.agentId === agentId
      ? { ...sa, toolCalls: [...(sa.toolCalls ?? []), { toolName, status: "running", durationMs: null, summary: null }] }
      : sa
  ));
}
```

- [ ] **Step 5: Type check**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/chat/mission-hooks.ts
git commit -m "feat(ui): rewrite mission-hooks with snapshot hydration and phase state machine"
```

---

## Task 6: Redesign MissionControl and ExecutionNarrative

**Files:**
- Modify: `apps/ui/src/features/chat/MissionControl.tsx`
- Modify: `apps/ui/src/features/chat/ExecutionNarrative.tsx`

- [ ] **Step 1: Update MissionControl to pass `phase` and render PhaseIndicators**

In `MissionControl.tsx`, import PhaseIndicators and update the JSX. The center area should render:
1. `ExecutionNarrative` with only user + response blocks
2. Between user message and response: `PhaseIndicators` with current phase

```tsx
import { PhaseIndicators } from "./PhaseIndicators";

// In the JSX, replace current ExecutionNarrative usage:
<div className="mission-control__main">
  <ExecutionNarrative
    blocks={state.blocks}
    status={state.status}
    phase={state.phase}
    subagents={state.subagents}
  />
  <div className="mission-control__input">
    <ChatInput onSend={sendMessage} disabled={state.status === "running"} />
  </div>
</div>
```

- [ ] **Step 2: Simplify ExecutionNarrative**

Rewrite `ExecutionNarrative.tsx` to only render user messages, phase indicators, and responses:

```tsx
import { UserMessage } from "./UserMessage";
import { AgentResponse } from "./AgentResponse";
import { PhaseIndicators, type Phase } from "./PhaseIndicators";
import type { NarrativeBlock } from "./mission-hooks";
import type { SubagentStateData } from "@/services/transport/types";
import { useEffect, useRef } from "react";

interface ExecutionNarrativeProps {
  blocks: NarrativeBlock[];
  status: string;
  phase: Phase;
  subagents?: SubagentStateData[];
}

export function ExecutionNarrative({ blocks, status, phase, subagents }: ExecutionNarrativeProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new blocks
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [blocks.length, phase]);

  const userBlocks = blocks.filter((b) => b.type === "user");
  const responseBlocks = blocks.filter((b) => b.type === "response");

  return (
    <div className="mission-control__narrative" ref={scrollRef}>
      {blocks.length === 0 && (
        <div className="empty-state">
          <p>Start a conversation to see execution progress here.</p>
        </div>
      )}

      {userBlocks.map((block, i) => (
        <div key={block.id}>
          <UserMessage
            content={block.data.content as string}
            timestamp={block.data.timestamp as string}
            attachments={block.data.attachments as Array<{ id: string; name: string }> | undefined}
          />

          {/* Show phase indicators after user message */}
          {status !== "idle" && (
            <PhaseIndicators phase={phase} subagents={subagents} />
          )}

          {/* Show matching response if it exists */}
          {responseBlocks[i] && (
            <AgentResponse
              content={responseBlocks[i].data.content as string}
              timestamp={responseBlocks[i].data.timestamp as string}
            />
          )}
        </div>
      ))}

      {/* Thinking indicator when agent is working and no response yet */}
      {status === "running" && phase !== "responding" && phase !== "completed" && (
        <div className="thinking-indicator">
          <div className="thinking-indicator__dots">
            <div className="thinking-indicator__dot" />
            <div className="thinking-indicator__dot" />
            <div className="thinking-indicator__dot" />
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Type check and build**

Run: `cd apps/ui && npx tsc --noEmit && npm run build`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/chat/MissionControl.tsx apps/ui/src/features/chat/ExecutionNarrative.tsx
git commit -m "feat(ui): redesign center panel — user message, phases, response only"
```

---

## Task 7: Upgrade IntelligenceFeed with Subagent Tool Calls

**Files:**
- Modify: `apps/ui/src/features/chat/IntelligenceFeed.tsx`

- [ ] **Step 1: Update SubagentInfo type to include tool calls**

Update the `SubagentInfo` interface (exported from IntelligenceFeed):

```typescript
export interface SubagentInfo {
  agentId: string;
  task: string;
  status: "active" | "completed" | "error";
  executionId?: string;
  toolCalls?: Array<{
    toolName: string;
    status: "running" | "completed" | "error";
    durationMs: number | null;
    summary: string | null;
  }>;
  durationMs?: number | null;
  tokenCount?: number | null;
}
```

- [ ] **Step 2: Render inline tool calls under each subagent**

In the Subagents section of `IntelligenceFeed.tsx`, replace the current subagent rendering with cards that show tool calls:

```tsx
{/* Subagents */}
<details className="intel-section" open={subagents.some((s) => s.status === "active")}>
  <summary className="intel-section__header">
    <span className="intel-icon">&#x1F916;</span>
    Subagents
    {subagents.filter((s) => s.status === "active").length > 0 && (
      <span className="intel-badge intel-badge--active">
        {subagents.filter((s) => s.status === "active").length} active
      </span>
    )}
  </summary>
  <div className="intel-section__body">
    {subagents.length === 0 ? (
      <div className="intel-empty">No subagents delegated yet</div>
    ) : (
      subagents.map((sa) => (
        <div
          key={sa.executionId ?? sa.agentId}
          className={`intel-subagent-card intel-subagent-card--${sa.status}`}
        >
          <div className="intel-subagent-card__header">
            <div className="intel-subagent-card__name">
              <span className={`intel-subagent__dot intel-subagent__dot--${sa.status}`} />
              {sa.agentId}
            </div>
            <span className="intel-subagent-card__meta">
              {sa.status === "completed" && sa.durationMs
                ? `${(sa.durationMs / 1000).toFixed(1)}s`
                : sa.status === "completed"
                ? "done"
                : ""}
              {sa.status === "completed" && sa.toolCalls && sa.toolCalls.length > 0
                ? ` · ${sa.toolCalls.length} tools`
                : ""}
            </span>
          </div>
          <div className="intel-subagent-card__task">{sa.task.slice(0, 120)}</div>
          {sa.toolCalls && sa.toolCalls.length > 0 && sa.status === "active" && (
            <div className="intel-subagent-card__tools">
              {sa.toolCalls.map((tc, i) => (
                <div key={i} className={`intel-tool-entry intel-tool-entry--${tc.status}`}>
                  <span className="intel-tool-entry__icon">
                    {tc.status === "completed" ? "✓" : tc.status === "error" ? "✗" : ""}
                    {tc.status === "running" && <span className="phase-step__pulse" />}
                  </span>
                  <span className="intel-tool-entry__name">{tc.toolName}</span>
                  <span className="intel-tool-entry__meta">
                    {tc.status === "running" ? "running..." : tc.durationMs ? `${(tc.durationMs / 1000).toFixed(1)}s` : ""}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      ))
    )}
  </div>
</details>
```

- [ ] **Step 3: Add CSS for subagent cards and tool entries**

Add to `apps/ui/src/styles/components.css`:

```css
/* Subagent Cards */
.intel-subagent-card { background: var(--muted); border-radius: var(--radius); padding: var(--spacing-2) var(--spacing-3); margin-bottom: var(--spacing-2); border: 1px solid var(--border); }
.intel-subagent-card--active { border-color: var(--primary); }
.intel-subagent-card--error { border-color: var(--destructive); }
.intel-subagent-card__header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--spacing-1); }
.intel-subagent-card__name { display: flex; align-items: center; gap: var(--spacing-1); font-weight: 600; font-size: var(--font-size-xs); }
.intel-subagent-card__meta { font-size: var(--font-size-xs); color: var(--muted-foreground); }
.intel-subagent-card__task { font-size: var(--font-size-xs); color: var(--muted-foreground); line-height: 1.4; margin-bottom: var(--spacing-2); }
.intel-subagent-card__tools { padding-left: var(--spacing-2); border-left: 2px solid var(--border); display: flex; flex-direction: column; gap: 2px; }
.intel-tool-entry { display: flex; align-items: center; gap: var(--spacing-1); font-size: var(--font-size-xs); }
.intel-tool-entry__icon { width: 14px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
.intel-tool-entry--completed .intel-tool-entry__icon { color: var(--success); }
.intel-tool-entry--error .intel-tool-entry__icon { color: var(--destructive); }
.intel-tool-entry__name { color: var(--muted-foreground); }
.intel-tool-entry--running .intel-tool-entry__name { color: var(--foreground); }
.intel-tool-entry__meta { margin-left: auto; color: var(--muted-foreground); }
.intel-badge--active { background: var(--primary); color: white; }
```

- [ ] **Step 4: Type check and build**

Run: `cd apps/ui && npx tsc --noEmit && npm run build`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat/IntelligenceFeed.tsx apps/ui/src/styles/components.css
git commit -m "feat(ui): upgrade sidebar subagent cards with inline tool calls"
```

---

## Task 8: Integration Test — End to End Verification

**Files:** None (verification only)

- [ ] **Step 1: Verify backend compiles and tests pass**

Run: `cargo check --workspace`
Expected: No errors.

Run: `cargo test --workspace`
Expected: All tests pass (except known pre-existing failures).

- [ ] **Step 2: Verify frontend builds**

Run: `cd apps/ui && npm run build`
Expected: Clean build.

- [ ] **Step 3: Manual smoke test — new session**

1. Start the daemon (`npm run daemon`)
2. Open the UI in browser
3. Send a message that triggers delegation (e.g., "Research NVDA stock price and write a summary")
4. Verify: center shows user message → phase indicators advancing → response
5. Verify: sidebar shows intent, ward, subagents with tool calls, plan

- [ ] **Step 4: Manual smoke test — reconnection**

1. While a session is running, close the browser tab
2. Reopen the UI
3. Verify: session loads from snapshot API (`/api/sessions/:id/state`)
4. Verify: center shows user message + correct phase + response (if completed)
5. Verify: sidebar shows all sections populated
6. If session was still running: verify WebSocket reconnects and live events resume

- [ ] **Step 5: Manual smoke test — past session**

1. From SessionBar, click a completed past session
2. Verify: all data loads correctly from snapshot
3. Verify: respond output is displayed

- [ ] **Step 6: Commit any fixes**

Only if integration testing reveals issues.
