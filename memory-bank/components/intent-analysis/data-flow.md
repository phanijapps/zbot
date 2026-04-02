# Intent Analysis — Data Flow

## Live Execution Flow

```
User sends message via WS
  │
  ▼
WebSocket handler: ClientMessage::Invoke
  ├── Build on_session_ready callback (captures Arc<SubscriptionManager>)
  └── Call runtime.invoke_with_hook_and_callback(...)
      │
      ▼
ExecutionRunner::invoke_with_callback()
  ├── get_or_create_session() → session_id, execution_id
  ├── start_execution(), store handle
  ├── on_session_ready(session_id).await  ← WS client now subscribed
  ├── emit AgentStarted event             ← subscriber receives ✓
  └── create_executor()
      │
      ▼  (is_root=true)
      │
  ┌─── Gate: has_intent_log(execution_id)? ──┐
  │ YES → skip (continuation turn)            │
  │ NO ↓                                       │
  └────────────────────────────────────────────┘
      │
  ┌─── Gate: fact_store available? ────┐
  │ NO → skip intent analysis          │
  │ YES ↓                              │
  └────────────────────────────────────┘
      │
      ▼
  1. index_resources()  (called in runner.rs, NOT inside analyze_intent)
     ├── SkillService::list() → save each as fact ("skill:{name}")
     ├── AgentService::list() → save each as fact ("agent:{id}")
     └── Scan vaults/wards/ dirs → save each as fact ("ward:{name}")
      │
      ▼
  2. emit IntentAnalysisStarted { session_id, execution_id }
      │
      ▼  (WS routes to subscribed client — subscription was set up by callback)
      │
      ▼  Frontend: creates NarrativeBlock { type: "intent_analysis", isStreaming: true }
      │
      ▼
  3. Create LLM client (OpenAiClient, max_tokens=8192, RetryingLlmClient wrapper)
      │
      ▼
  4. analyze_intent(llm_client, user_message, fact_store)  ← 3 params only
     ├── search_resources() → recall_facts("root", msg, 50) → filter by score ≥0.15, cap 8/5/5
     ├── Build messages: [system=INTENT_ANALYSIS_PROMPT, user=format_user_template()]
     └── LLM call → strip_markdown_fences() → serde_json::from_str()
          ├── Success → IntentAnalysis struct
          └── Failure → return Err (no repair attempt)
      │
      ▼
  5a. SUCCESS path:
     ├── emit IntentAnalysisComplete { full analysis data }
     ├── Log to execution_logs (category="intent", metadata=full IntentAnalysis JSON)
     └── agent_for_build.instructions.push_str(&format_intent_injection(&analysis))
         ↑ THIS is the critical injection — agent now sees ward name, skills, strategy
      │
  5b. FAILURE path:
     └── emit IntentAnalysisComplete { fallback: primary_intent="general", approach="simple", ward="scratch" }
      │
      ▼
  6. Continue building executor with enriched agent.instructions
```

## Session Replay Flow

```
User opens existing session (page load / session switch)
  │
  ▼
useMissionControl mount effect (mission-hooks.ts)
  │
  ▼
transport.getLogSession(sessionId)
  │
  ▼
Iterate execution_logs for this session
  │
  ▼  log.category === "intent" && log.metadata exists
  │
  ▼
Parse metadata (handle string JSON or object):
  ├── snake_case → camelCase field mapping
  ├── setIntentAnalysis(ia) → populates IntelligenceFeed sidebar
  ├── setActiveWard() from ia.wardRecommendation.wardName (fallback)
  └── loadedBlocks.push({ type: "intent_analysis", data: { analysis: ia } })
```

## OnSessionReady Callback (WS Race Condition Fix)

```
Problem: invoke() runs intent analysis synchronously before returning session_id.
         Gateway only subscribed the WS client AFTER invoke() returned.
         Intent events emitted during create_executor() had no subscriber.

Fix: invoke_with_callback(config, msg, Some(on_ready))
     ├── Creates session → session_id
     ├── Calls on_ready(session_id).await  ← Gateway subscribes WS client here
     ├── Emits AgentStarted              ← subscriber receives ✓
     └── create_executor()               ← intent events received ✓

OnSessionReady type: Box<dyn FnOnce(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>
Exported from gateway-execution crate.
```

## Event Bus Path

```
GatewayEvent (gateway-events)
  │
  ├── IntentAnalysisStarted { session_id, execution_id }
  ├── IntentAnalysisComplete { session_id, execution_id, primary_intent, ... }
  │
  ▼
Central event router (gateway/src/websocket/handler.rs)
  ├── Extracts session_id from event
  ├── Converts to ServerMessage via gateway_event_to_server_message()
  └── Routes via SubscriptionManager::route_event_scoped(session_id, msg, metadata)
       │
       ▼
ServerMessage (gateway-ws-protocol)
  ├── IntentAnalysisStarted { session_id, execution_id, seq }
  ├── IntentAnalysisComplete { session_id, ..., seq }
  │
  ▼
WebSocket → JSON → Frontend
  ├── case "intent_analysis_started" → streaming block
  └── case "intent_analysis_complete" → update with data
```
