# Chat Experience

The chat page is the primary user interface for z-Bot. It has three panels: session bar (top), center conversation, and right sidebar.

## Architecture

```
MissionControl (orchestrator)
├── HeroInput (landing — no session active)
│   ├── Zero ring SVG logo + brand
│   ├── Textarea + file attach + send
│   ├── Suggestion chips
│   └── Recent sessions (5 root sessions)
│
└── Session View (active/loaded session)
    ├── SessionBar (top: +New, title, status, tokens, duration, stop)
    ├── Center Panel (ExecutionNarrative)
    │   ├── UserMessage
    │   ├── PhaseIndicators (only on latest turn)
    │   │   ├── ✓ Analyzing intent
    │   │   ├── ✓ Planning execution
    │   │   ├── ⟳ Executing (agent names, n/m complete)
    │   │   └── ○ Generating response
    │   ├── AgentResponse (markdown rendered)
    │   └── ChatInput (textarea + file attach + send)
    │
    └── Right Sidebar (IntelligenceFeed)
        ├── Intent Analysis (collapsed)
        ├── Active Ward
        ├── Recalled Facts (collapsed, badge count)
        ├── Subagents (expanded when active)
        │   ├── Active: blue border, pulsing dot, inline tool calls
        │   ├── Completed: ✓ tick, strikethrough, dimmed
        │   └── Error: ✗, red border
        └── Execution Plan (task checklist)
```

## Key Design Decisions

- **Center panel is minimal**: only user message → phase indicators → response. No tool calls, delegations, or recall blocks in center.
- **Sidebar shows execution detail**: subagents with inline tool calls, plan steps, recalled facts, intent analysis.
- **Phase indicators only on latest turn**: multi-turn conversations show previous turns as message+response pairs without phases.
- **Session State API for reconnection**: `GET /api/sessions/:id/state` returns a structured snapshot. No more log parsing on the frontend.

## Data Flow

### New Session (live)
```
User sends message
  → POST /api/agents/:id/execute → sessionId
  → Subscribe WebSocket (sessionId)
  → Events update: phase state machine + sidebar sections
  → respond tool or token stream → response in center
  → agent_completed → phase=completed, enable input
```

### Reconnect / Load Past Session
```
Page loads → check localStorage for sessionId
  → GET /api/sessions/:id/state → SessionState snapshot
  → Render center: user message + phases + response
  → Render sidebar: intent, ward, facts, subagents, plan
  → If isLive: subscribe WebSocket for live events
```

### Phase State Machine
```
idle → intent → planning → executing → responding → completed
                                                  → error
```

| Event | Transition |
|-------|-----------|
| User sends message | idle → intent |
| intent_analysis_complete | intent → planning |
| delegate_to_agent or update_plan | planning → executing |
| respond tool or first token | executing → responding |
| agent_completed | → completed |
| error | → error |

## Files

### Backend
| File | Purpose |
|------|---------|
| `gateway/gateway-execution/src/session_state.rs` | SessionStateBuilder — assembles snapshot from DB |
| `gateway/src/http/sessions.rs` | GET /api/sessions/:id/state handler |
| `services/api-logs/src/repository.rs` | listLogSessions with root_only filter, title JOIN |
| `services/api-logs/src/service.rs` | get_session_detail with crashed status detection |

### Frontend
| File | Purpose |
|------|---------|
| `apps/ui/src/features/chat/MissionControl.tsx` | Top-level orchestrator, HeroInput vs session view |
| `apps/ui/src/features/chat/mission-hooks.ts` | useMissionControl hook: state, events, snapshot hydration |
| `apps/ui/src/features/chat/ExecutionNarrative.tsx` | Center panel: user messages, phases, responses |
| `apps/ui/src/features/chat/PhaseIndicators.tsx` | 4-phase progress component |
| `apps/ui/src/features/chat/IntelligenceFeed.tsx` | Right sidebar: 5 collapsible sections |
| `apps/ui/src/features/chat/ChatInput.tsx` | Textarea + file attach + send |
| `apps/ui/src/features/chat/HeroInput.tsx` | Landing page input + recent sessions |
| `apps/ui/src/features/chat/SessionBar.tsx` | Top bar: title, status, metrics |
| `apps/ui/src/features/chat/AgentResponse.tsx` | Markdown-rendered response |
| `apps/ui/src/features/chat/UserMessage.tsx` | User message bubble |
| `apps/ui/src/features/chat/PlanBlock.tsx` | Task checklist (sidebar) |
| `apps/ui/src/features/chat/IntentAnalysisBlock.tsx` | Intent display (sidebar) |

### Transport
| File | Purpose |
|------|---------|
| `apps/ui/src/services/transport/types.ts` | SessionState, SubagentStateData, ToolCallEntryData |
| `apps/ui/src/services/transport/interface.ts` | getSessionState() method |
| `apps/ui/src/services/transport/http.ts` | HTTP implementation |

### Tests
| File | Purpose |
|------|---------|
| `gateway/gateway-execution/tests/session_state_tests.rs` | 10 automated tests for SessionStateBuilder |

## Known Issues

- **Session status inheritance**: Root session stays "crashed" after successful continuation. See memory bank defect_session_status_inheritance.md.
- **React strict mode**: causes double listLogSessions call in dev — harmless, doesn't happen in production.

## CSS Conventions

All components use the project's CSS variable system. Sidebar uses fixed pixel sizes for consistency:
- Section headers: 9px, uppercase
- Content text: 10-11px
- Tool entries: 10px
- Phase icons: 18px (center), 12px (sidebar tools)
- Subagent status: ✓/✗ icons + strikethrough for completed (matches plan step styling)
