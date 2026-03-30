# Chat Session Defects + Intent Analysis Restoration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 4 chat session display defects (title, thinking animation, final response) and restore intent analysis with transparent sidebar visibility.

**Architecture:** Backend adds two new LogCategory variants (Response, Intent) and a new GatewayEvent (IntentAnalysisComplete). Frontend fixes race conditions in title/thinking display and adds response block reconstruction + intent analysis sidebar section.

**Tech Stack:** Rust (gateway, services), TypeScript/React (apps/ui)

**Spec:** `docs/superpowers/specs/2026-03-30-chat-session-defects-intent-restoration-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `services/api-logs/src/types.rs` | Modify | Add `Response` + `Intent` to LogCategory enum |
| `gateway/gateway-events/src/lib.rs` | Modify | Add `IntentAnalysisComplete` variant + update accessors |
| `gateway/gateway-execution/src/runner.rs` | Modify | Emit response log, re-wire intent analysis |
| `apps/ui/src/features/chat/mission-hooks.ts` | Modify | Title fallback, agent_completed safety net, response/intent log loading, intent state |
| `apps/ui/src/features/chat/ExecutionNarrative.tsx` | Modify | Accept `status` prop, guard thinking indicator |
| `apps/ui/src/features/chat/MissionControl.tsx` | Modify | Pass `status` + `intentAnalysis` props |
| `apps/ui/src/features/chat/IntelligenceFeed.tsx` | Modify | Add intent analysis sidebar section |

---

## Task 1: Add Response + Intent LogCategory Variants

**Files:**
- Modify: `services/api-logs/src/types.rs:59-115`

- [ ] **Step 1: Add variants to LogCategory enum**

In `services/api-logs/src/types.rs`, add `Response` and `Intent` to the enum at line 76 (before the closing brace):

```rust
// After the existing Error variant (line 76):
    /// Agent's final response content
    Response,
    /// Intent analysis results
    Intent,
```

- [ ] **Step 2: Add as_str() match arms**

In the `as_str()` method (lines 79-89), add before the closing brace:

```rust
            Self::Response => "response",
            Self::Intent => "intent",
```

- [ ] **Step 3: Add FromStr arms**

In the `FromStr` impl (lines 99-115), add two new match arms before the `_ => Err(...)` line:

```rust
            "response" => Ok(Self::Response),
            "intent" => Ok(Self::Intent),
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p api-logs`
Expected: success, no errors

- [ ] **Step 5: Commit**

```bash
git add services/api-logs/src/types.rs
git commit -m "feat(api-logs): add Response and Intent log categories"
```

---

## Task 2: Add IntentAnalysisComplete GatewayEvent

**Files:**
- Modify: `gateway/gateway-events/src/lib.rs:24-376`

- [ ] **Step 1: Add the new variant to GatewayEvent enum**

Add after the `SessionTitleChanged` variant (the last variant before the closing brace of the enum):

```rust
    /// Intent analysis completed for a root session
    IntentAnalysisComplete {
        session_id: String,
        execution_id: String,
        primary_intent: String,
        hidden_intents: Vec<String>,
        recommended_skills: Vec<String>,
        recommended_agents: Vec<String>,
        ward_recommendation: serde_json::Value,
        execution_strategy: serde_json::Value,
    },
```

- [ ] **Step 2: Update agent_id() accessor**

In the `agent_id()` method (lines 276-305), add before the closing brace:

```rust
            Self::IntentAnalysisComplete { .. } => None,
```

- [ ] **Step 3: Update session_id() accessor**

In the `session_id()` method (lines 312-337), add before the closing brace:

```rust
            Self::IntentAnalysisComplete { session_id, .. } => Some(session_id),
```

- [ ] **Step 4: Update execution_id() accessor**

In the `execution_id()` method (lines 343-376), add before the closing brace:

```rust
            Self::IntentAnalysisComplete { execution_id, .. } => Some(execution_id),
```

- [ ] **Step 5: Update conversation_id() accessor**

In the `conversation_id()` method (lines 381-412), add before the closing brace:

```rust
            Self::IntentAnalysisComplete { .. } => None,
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p gateway-events`
Expected: success, no errors

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-events/src/lib.rs
git commit -m "feat(gateway-events): add IntentAnalysisComplete event variant"
```

---

## Task 3: Emit Response ExecutionLog in Runner

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:805-814`

- [ ] **Step 1: Add response log emission after session_message**

In `runner.rs`, **replace** the existing `if !accumulated_response.is_empty()` block (lines 805-814) with this version that adds the log call after `session_message`:

```rust
            // Emit final assistant response to session stream
            // (only if there's content not already emitted as part of a tool-call turn)
            if !accumulated_response.is_empty() {
                batch_writer.session_message(
                    &session_id,
                    &execution_id,
                    "assistant",
                    &accumulated_response,
                    None,
                    None,
                );

                // Log the response for session replay
                let response_log = api_logs::ExecutionLog::new(
                    &execution_id,
                    &session_id,
                    &agent_id,
                    api_logs::LogLevel::Info,
                    api_logs::LogCategory::Response,
                    &accumulated_response,
                );
                batch_writer.log(response_log);
            }
```

**Important:** This is a replacement of lines 805-814, not an addition. The `session_message` call is preserved from the original; the `batch_writer.log()` call is new.

- [ ] **Step 2: Verify the api_logs import exists**

Check that `runner.rs` already imports from `api_logs` or `api-logs`. Search for existing `ExecutionLog` usage. If not imported, add:

```rust
use api_logs::{ExecutionLog, LogCategory, LogLevel};
```

Run: `cargo check -p gateway-execution`

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --workspace`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "fix(runner): emit Response ExecutionLog for session replay"
```

---

## Task 4: Fix Session Title Fallback

**Files:**
- Modify: `apps/ui/src/features/chat/mission-hooks.ts`

- [ ] **Step 1: Add refs for title fallback**

Near the existing refs (around line 120), add:

```typescript
const titleFallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
const lastUserMessageRef = useRef<string>("");
```

The `lastUserMessageRef` tracks the most recent user message so the fallback timer can access it without stale closure issues.

- [ ] **Step 2: Track user message in the ref**

In the `sendMessage` function (where the user block is created and pushed to `setBlocks`), add after the user block is created:

```typescript
lastUserMessageRef.current = message;
```

- [ ] **Step 3: Add helper to generate fallback title**

Add this helper function inside the hook (before the event handler switch):

```typescript
const generateFallbackTitle = (message: string): string => {
  const clean = message.replace(/\s+/g, " ").trim();
  if (clean.length <= 50) return clean;
  const truncated = clean.slice(0, 50);
  const lastSpace = truncated.lastIndexOf(" ");
  return (lastSpace > 20 ? truncated.slice(0, lastSpace) : truncated) + "...";
};
```

- [ ] **Step 4: Start fallback timer in agent_started handler**

In the `agent_started` case handler (lines 222-233), add the timer logic after the existing code:

```typescript
case "agent_started": {
  setStatus("running");
  startDurationTimer();
  if (event.session_id && typeof event.session_id === "string") {
    setSessionId(event.session_id);
    setActiveSessionId(event.session_id);
  }
  if (event.model && typeof event.model === "string") {
    setModelName(event.model);
  }

  // Clear any previous fallback timer
  if (titleFallbackTimerRef.current) {
    clearTimeout(titleFallbackTimerRef.current);
    titleFallbackTimerRef.current = null;
  }

  // Start fallback title timer — if no title arrives in 10s, generate from user message
  titleFallbackTimerRef.current = setTimeout(() => {
    setSessionTitle((current) => {
      if (current) return current; // Title already set
      const msg = lastUserMessageRef.current;
      if (!msg) return current;
      return generateFallbackTitle(msg);
    });
    titleFallbackTimerRef.current = null;
  }, 10_000);
  break;
}
```

- [ ] **Step 5: Cancel timer when title arrives**

In the `session_title_changed` handler (lines 566-570), add timer cancellation:

```typescript
case "session_title_changed": {
  const title = (event.title ?? "") as string;
  if (title) {
    setSessionTitle(title);
    // Cancel fallback timer — real title arrived
    if (titleFallbackTimerRef.current) {
      clearTimeout(titleFallbackTimerRef.current);
      titleFallbackTimerRef.current = null;
    }
  }
  break;
}
```

- [ ] **Step 6: Cancel timer in set_session_title tool_call handler**

In the `tool_call` handler where `set_session_title` is detected (around line 270), add the same cancellation:

```typescript
if (toolName === "set_session_title") {
  const title = (args.title ?? args.name ?? "") as string;
  if (title) {
    setSessionTitle(title);
    if (titleFallbackTimerRef.current) {
      clearTimeout(titleFallbackTimerRef.current);
      titleFallbackTimerRef.current = null;
    }
  }
  break;
}
```

- [ ] **Step 7: Add cleanup on unmount**

Add a `useEffect` cleanup for the timer ref (near other effect cleanups):

```typescript
useEffect(() => {
  return () => {
    if (titleFallbackTimerRef.current) {
      clearTimeout(titleFallbackTimerRef.current);
    }
  };
}, []);
```

- [ ] **Step 8: Verify it builds**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 9: Commit**

```bash
git add apps/ui/src/features/chat/mission-hooks.ts
git commit -m "fix(ui): add fallback title generation from user message"
```

---

## Task 5: Fix Thinking Animation on Resumed Chats

**Files:**
- Modify: `apps/ui/src/features/chat/ExecutionNarrative.tsx:19-21, 149-171`
- Modify: `apps/ui/src/features/chat/MissionControl.tsx:59`

- [ ] **Step 1: Add status prop to ExecutionNarrative**

In `ExecutionNarrative.tsx`, update the props interface (lines 19-21):

```typescript
export interface ExecutionNarrativeProps {
  blocks: NarrativeBlock[];
  status: string;
}
```

Update the component signature to destructure the new prop:

```typescript
export function ExecutionNarrative({ blocks, status }: ExecutionNarrativeProps) {
```

- [ ] **Step 2: Add outer guard to thinking indicator**

In `ExecutionNarrative.tsx`, wrap the thinking indicator section (lines 149-171) with a status guard. Replace:

```typescript
{blocks.length > 0 && !blocks.some(b => b.type === 'response' && b.isStreaming) && (
```

With:

```typescript
{status === "running" && blocks.length > 0 && !blocks.some(b => b.type === 'response' && b.isStreaming) && (
```

This adds `status === "running"` as the first condition. The rest of the inner logic stays exactly the same.

- [ ] **Step 3: Pass status from MissionControl**

In `MissionControl.tsx` (line 59), update the ExecutionNarrative rendering:

```typescript
<ExecutionNarrative blocks={state.blocks} status={state.status} />
```

- [ ] **Step 4: Verify it builds**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat/ExecutionNarrative.tsx apps/ui/src/features/chat/MissionControl.tsx
git commit -m "fix(ui): suppress thinking indicator on completed/resumed sessions"
```

---

## Task 6: Fix Final Response Display (Frontend)

> **Note:** Tasks 4, 6, and 8 all modify `mission-hooks.ts`. Line numbers reference the original file. After Task 4, line numbers will have shifted — locate insertion points by searching for the code patterns shown, not by line number.

**Files:**
- Modify: `apps/ui/src/features/chat/mission-hooks.ts`

- [ ] **Step 1: Add safety net to agent_completed handler**

In `mission-hooks.ts`, update the `agent_completed` handler (lines 612-623). After `flushTokenBuffer()` and before `setStatus("completed")`, add the safety net:

```typescript
case "agent_completed": {
  if (rafIdRef.current !== null) {
    cancelAnimationFrame(rafIdRef.current);
    rafIdRef.current = null;
  }
  flushTokenBuffer();

  // Safety net: create response block from result if none exists
  const result = event.result as string | undefined;
  if (result) {
    setBlocks((prev) => {
      const hasResponse = prev.some((b) => b.type === "response");
      if (hasResponse) return prev;
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "response",
          timestamp: now(),
          data: { content: result, timestamp: now() },
          isStreaming: false,
        },
      ];
    });
  }

  setStatus("completed");
  stopDurationTimer();
  // Finalize any streaming blocks
  setBlocks((prev) => prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)));
  break;
}
```

- [ ] **Step 2: Add response log handler in session loading**

In the session-loading useEffect's log-to-block conversion loop (around line 744-799), add a new `else if` branch after the `tool_result` handler and before the `session` handler:

```typescript
  } else if (log.category === "response" && log.message.length > 0) {
    loadedBlocks.push({
      id: log.id,
      type: "response",
      timestamp: log.timestamp,
      data: { content: log.message, timestamp: log.timestamp },
      isStreaming: false,
    });
```

- [ ] **Step 3: Verify it builds**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/chat/mission-hooks.ts
git commit -m "fix(ui): display final response for new and resumed sessions"
```

---

## Task 7: Re-wire Intent Analysis in Runner

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:1095-1213`

- [ ] **Step 1: Restore the import**

At the top of `runner.rs` (line 22 area), change the import from:

```rust
use crate::middleware::intent_analysis::index_resources;
```

To:

```rust
use crate::middleware::intent_analysis::{analyze_intent, index_resources, inject_intent_context};
```

- [ ] **Step 2: Un-underscore user_message parameter**

In `create_executor` (line 1103), change:

```rust
        _user_message: Option<&str>,
```

To:

```rust
        user_message: Option<&str>,
```

- [ ] **Step 3: Add Serialize derive to IntentAnalysis types (MUST be done before Step 4)**

In `gateway/gateway-execution/src/middleware/intent_analysis.rs`, the structs (`IntentAnalysis`, `WardRecommendation`, `ExecutionStrategy`, `ExecutionGraph`, `GraphNode`, `GraphEdge`, `EdgeCondition`) currently only derive `Deserialize`. Add `Serialize` to each:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
```

Also update the import at line 3:

```rust
use serde::{Deserialize, Serialize};
```

- [ ] **Step 4: Build temp LLM client and call analyze_intent**

In `create_executor`, replace the resource indexing block (lines 1167-1180) with:

```rust
        // Intent analysis for root agent first turns
        let mut agent_for_build = agent.clone();
        if is_root {
            if let Some(ref fs) = fact_store_for_indexing {
                // Index resources (fast DB upsert — no LLM call)
                index_resources(
                    fs.as_ref(),
                    &self.skill_service,
                    &self.agent_service,
                    &self.paths,
                )
                .await;
                tracing::info!("Resource indexing complete (skills, agents, wards)");

                // Run intent analysis if user message is present
                if let Some(msg) = user_message {
                    // Build temporary LLM client for analysis
                    let llm_config = agent_runtime::LlmConfig::new(
                        provider.base_url.clone(),
                        provider.api_key.clone(),
                        agent.model.clone(),
                        provider.id.clone().unwrap_or_else(|| provider.name.clone()),
                    );
                    match agent_runtime::OpenAiClient::new(llm_config) {
                        Ok(raw_client) => {
                            let retrying = agent_runtime::RetryingLlmClient::new(
                                std::sync::Arc::new(raw_client),
                                agent_runtime::RetryPolicy::default(),
                            );

                            match analyze_intent(
                                &retrying,
                                msg,
                                fs.as_ref(),
                                &self.skill_service,
                                &self.agent_service,
                                &self.paths,
                            )
                            .await
                            {
                                Ok(analysis) => {
                                    tracing::info!(
                                        primary_intent = %analysis.primary_intent,
                                        approach = %analysis.execution_strategy.approach,
                                        "Intent analysis succeeded"
                                    );

                                    // Inject into system prompt
                                    inject_intent_context(
                                        &mut agent_for_build.instructions,
                                        &analysis,
                                    );

                                    // Emit IntentAnalysisComplete event
                                    self.event_bus
                                        .publish(GatewayEvent::IntentAnalysisComplete {
                                            session_id: session_id.to_string(),
                                            execution_id: config.execution_id.clone().unwrap_or_default(),
                                            primary_intent: analysis.primary_intent.clone(),
                                            hidden_intents: analysis.hidden_intents.clone(),
                                            recommended_skills: analysis.recommended_skills.clone(),
                                            recommended_agents: analysis.recommended_agents.clone(),
                                            ward_recommendation: serde_json::to_value(&analysis.ward_recommendation).unwrap_or_default(),
                                            execution_strategy: serde_json::to_value(&analysis.execution_strategy).unwrap_or_default(),
                                        })
                                        .await;

                                    // Log for session replay
                                    if let Ok(meta) = serde_json::to_value(&analysis) {
                                        let log_entry = api_logs::ExecutionLog::new(
                                            config.execution_id.as_deref().unwrap_or(""),
                                            session_id,
                                            &config.agent_id,
                                            api_logs::LogLevel::Info,
                                            api_logs::LogCategory::Intent,
                                            format!("Intent: {}", analysis.primary_intent),
                                        )
                                        .with_metadata(meta);
                                        let _ = self.log_service.log(log_entry);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Intent analysis failed (non-fatal): {}", e);
                                    // Continue without intent analysis — agent will rely on first_turn_protocol shard
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create LLM client for intent analysis: {}", e);
                        }
                    }
                }
            }
        }
```

- [ ] **Step 5: Update builder.build() to use enriched agent**

At lines 1200-1212, change `agent` to `&agent_for_build` in the builder.build() call:

```rust
        builder
            .build(
                &agent_for_build,
                provider,
                &config.conversation_id,
                session_id,
                &available_agents,
                &available_skills,
                hook_context.as_ref(),
                &self.mcp_service,
                ward_id,
            )
            .await
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check --workspace`
Expected: success

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(runner): restore intent analysis with event emission and logging"
```

---

## Task 8: Add Intent Analysis State + Event Handling (Frontend)

**Files:**
- Modify: `apps/ui/src/features/chat/mission-hooks.ts`

- [ ] **Step 1: Add IntentAnalysis type**

Near the top of `mission-hooks.ts` (after the existing type definitions), add:

```typescript
export interface IntentAnalysis {
  primaryIntent: string;
  hiddenIntents: string[];
  recommendedSkills: string[];
  recommendedAgents: string[];
  wardRecommendation: {
    action: string;
    wardName: string;
    subdirectory?: string;
    reason: string;
  };
  executionStrategy: {
    approach: string;
    graph?: {
      nodes: Array<{ id: string; task: string; agent: string; skills: string[] }>;
      mermaid?: string;
    };
    explanation: string;
  };
}
```

- [ ] **Step 2: Add intentAnalysis state**

In the state declarations (around line 112), add:

```typescript
const [intentAnalysis, setIntentAnalysis] = useState<IntentAnalysis | null>(null);
```

- [ ] **Step 3: Add event handler for intent_analysis_complete**

In the event handler switch statement, add a new case (after `session_title_changed` around line 570):

```typescript
case "intent_analysis_complete": {
  const ia: IntentAnalysis = {
    primaryIntent: (event.primary_intent ?? "") as string,
    hiddenIntents: (event.hidden_intents ?? []) as string[],
    recommendedSkills: (event.recommended_skills ?? []) as string[],
    recommendedAgents: (event.recommended_agents ?? []) as string[],
    wardRecommendation: {
      action: (event.ward_recommendation?.action ?? "") as string,
      wardName: (event.ward_recommendation?.ward_name ?? "") as string,
      subdirectory: event.ward_recommendation?.subdirectory as string | undefined,
      reason: (event.ward_recommendation?.reason ?? "") as string,
    },
    executionStrategy: {
      approach: (event.execution_strategy?.approach ?? "simple") as string,
      graph: event.execution_strategy?.graph as IntentAnalysis["executionStrategy"]["graph"],
      explanation: (event.execution_strategy?.explanation ?? "") as string,
    },
  };
  setIntentAnalysis(ia);
  break;
}
```

- [ ] **Step 4: Add intent log handler in session loading**

In the session-loading log-to-block conversion loop, add another `else if` branch (after the `response` handler added in Task 6):

```typescript
  } else if (log.category === "intent" && log.metadata) {
    try {
      const meta = typeof log.metadata === "string" ? JSON.parse(log.metadata) : log.metadata;
      setIntentAnalysis({
        primaryIntent: meta.primary_intent ?? "",
        hiddenIntents: meta.hidden_intents ?? [],
        recommendedSkills: meta.recommended_skills ?? [],
        recommendedAgents: meta.recommended_agents ?? [],
        wardRecommendation: {
          action: meta.ward_recommendation?.action ?? "",
          wardName: meta.ward_recommendation?.ward_name ?? "",
          subdirectory: meta.ward_recommendation?.subdirectory,
          reason: meta.ward_recommendation?.reason ?? "",
        },
        executionStrategy: {
          approach: meta.execution_strategy?.approach ?? "simple",
          graph: meta.execution_strategy?.graph,
          explanation: meta.execution_strategy?.explanation ?? "",
        },
      });
    } catch {
      // Ignore malformed intent metadata
    }
```

- [ ] **Step 5: Add intentAnalysis to the return state**

In the `MissionState` interface definition, add:

```typescript
intentAnalysis: IntentAnalysis | null;
```

And in the state object (around line 914-927), add:

```typescript
intentAnalysis,
```

- [ ] **Step 6: Reset intentAnalysis in startNewSession**

In the `startNewSession` function (around line 894), add alongside the other state resets:

```typescript
setIntentAnalysis(null);
```

- [ ] **Step 7: Verify it builds**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 8: Commit**

```bash
git add apps/ui/src/features/chat/mission-hooks.ts
git commit -m "feat(ui): add intent analysis state and event handling"
```

---

## Task 9: Add Intent Analysis Sidebar Section

**Files:**
- Modify: `apps/ui/src/features/chat/IntelligenceFeed.tsx:25-30, 56-135`
- Modify: `apps/ui/src/features/chat/MissionControl.tsx:62-68`

- [ ] **Step 1: Import IntentAnalysis type and update props**

In `IntelligenceFeed.tsx`, import the type and update props (lines 25-30):

```typescript
import type { IntentAnalysis } from "./mission-hooks";

export interface IntelligenceFeedProps {
  ward: { name: string; content: string } | null;
  recalledFacts: RecalledFact[];
  subagents: SubagentInfo[];
  plan: PlanStep[];
  intentAnalysis: IntentAnalysis | null;
}
```

- [ ] **Step 2: Add intent analysis section**

In the `IntelligenceFeed` component, destructure the new prop and add the intent section as the **first section** (above Active Ward). Add this JSX before the existing ward section:

```tsx
{intentAnalysis && (
  <details className="intel-section">
    <summary className="intel-section__header">
      <span className="intel-section__icon">&#x1f9e0;</span>
      Intent Analysis
      <span className="intel-badge">{intentAnalysis.executionStrategy.approach}</span>
    </summary>
    <div className="intel-section__body">
      <div className="intel-item">
        <span className="intel-label">Primary Intent</span>
        <span className="intel-value">{intentAnalysis.primaryIntent}</span>
      </div>

      {intentAnalysis.hiddenIntents.length > 0 && (
        <div className="intel-item">
          <span className="intel-label">Hidden Intents</span>
          <ul className="intel-list">
            {intentAnalysis.hiddenIntents.map((h, i) => (
              <li key={i}>{h}</li>
            ))}
          </ul>
        </div>
      )}

      {intentAnalysis.recommendedSkills.length > 0 && (
        <div className="intel-item">
          <span className="intel-label">Skills</span>
          <div className="intel-tags">
            {intentAnalysis.recommendedSkills.map((s) => (
              <span key={s} className="intel-tag">{s}</span>
            ))}
          </div>
        </div>
      )}

      {intentAnalysis.recommendedAgents.length > 0 && (
        <div className="intel-item">
          <span className="intel-label">Agents</span>
          <div className="intel-tags">
            {intentAnalysis.recommendedAgents.map((a) => (
              <span key={a} className="intel-tag">{a}</span>
            ))}
          </div>
        </div>
      )}

      <div className="intel-item">
        <span className="intel-label">Ward</span>
        <span className="intel-value">
          {intentAnalysis.wardRecommendation.wardName} ({intentAnalysis.wardRecommendation.action})
        </span>
        <span className="intel-detail">{intentAnalysis.wardRecommendation.reason}</span>
      </div>

      {intentAnalysis.executionStrategy.graph && (
        <div className="intel-item">
          <span className="intel-label">Execution Graph</span>
          <ul className="intel-list">
            {intentAnalysis.executionStrategy.graph.nodes.map((n) => (
              <li key={n.id}>
                <strong>{n.id}:</strong> {n.task} <em>({n.agent})</em>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="intel-item">
        <span className="intel-label">Strategy</span>
        <span className="intel-detail">{intentAnalysis.executionStrategy.explanation}</span>
      </div>
    </div>
  </details>
)}
```

- [ ] **Step 3: Pass intentAnalysis from MissionControl**

In `MissionControl.tsx` (lines 62-68), add the new prop:

```typescript
<IntelligenceFeed
  ward={state.activeWard}
  recalledFacts={state.recalledFacts}
  subagents={state.subagents}
  plan={state.plan}
  intentAnalysis={state.intentAnalysis}
/>
```

- [ ] **Step 4: Add CSS for intent section**

Check the existing styles file for IntelligenceFeed (likely `apps/ui/src/styles/components.css` or similar). Add styles for the new intent-specific classes. Use the existing `intel-section` pattern and add:

```css
.intel-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 0.7rem;
  font-weight: 600;
  text-transform: uppercase;
  background: var(--color-accent, #3b82f6);
  color: var(--color-bg, #fff);
  margin-left: 8px;
}

.intel-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.intel-tag {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 0.7rem;
  background: var(--color-surface, #1e1e2e);
  border: 1px solid var(--color-border, #333);
}

.intel-detail {
  display: block;
  font-size: 0.75rem;
  color: var(--color-text-muted, #888);
  margin-top: 2px;
}
```

- [ ] **Step 5: Export IntentAnalysis from index**

If `apps/ui/src/features/chat/index.ts` re-exports types, add `IntentAnalysis` to the exports.

- [ ] **Step 6: Verify it builds**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 7: Commit**

```bash
git add apps/ui/src/features/chat/IntelligenceFeed.tsx apps/ui/src/features/chat/MissionControl.tsx apps/ui/src/styles/
git commit -m "feat(ui): add intent analysis sidebar with progressive disclosure"
```

---

## Task 10: Final Workspace Verification

- [ ] **Step 1: Full backend build**

Run: `cargo check --workspace`
Expected: success, no warnings about unused imports

- [ ] **Step 2: Full frontend build**

Run: `cd apps/ui && npm run build`
Expected: success

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p api-logs && cargo test -p gateway-events`
Expected: all pass

Run: `cargo test -p gateway-execution -- intent_analysis`
Expected: all 5 existing intent analysis tests pass

- [ ] **Step 4: Commit any final cleanup**

```bash
git add -A
git commit -m "chore: final cleanup for chat session fixes + intent restoration"
```
