# Services

Standalone data services with minimal framework dependencies. Each service is independently testable.

## Crates

| Crate | Purpose | Storage | Tests |
|-------|---------|---------|-------|
| `execution-state` | Session lifecycle, execution tracking, token consumption, dashboard stats | SQLite | 82 |
| `api-logs` | Execution logging and tracing with categories and filtering | SQLite | 0 |
| `knowledge-graph` | Entity extraction and relationship storage | SQLite | 19 |
| `daily-sessions` | Daily session continuity with message archiving | SQLite + moka cache | 16 |

## Build & Test

```bash
cargo test -p execution-state    # 82 tests
cargo test -p knowledge-graph    # 19 tests
cargo test -p daily-sessions     # 16 tests
```

## Design Pattern

Services expose traits that the gateway implements:

```rust
// In services/execution-state
pub trait StateDbProvider {
    fn get_connection(&self) -> Result<PooledConnection>;
}

// In services/api-logs
pub trait DbProvider {
    fn get_connection(&self) -> &Connection;
}

// Gateway's DatabaseManager implements both traits
```

This inverts dependencies — services don't depend on gateway, gateway depends on services.

## execution-state

The most critical service. Tracks sessions, agent executions, delegations, ward assignments, and token usage.

**Key types**: `Session`, `AgentExecution`, `SessionStatus`, `ExecutionStatus`, `DelegationType`, `TriggerSource`, `DashboardStats`

**HTTP routes** (11 endpoints):
- `GET /v2/sessions` — List sessions
- `GET /v2/sessions/:id/full` — Session with all executions
- `POST /sessions/:id/{pause,resume,cancel}` — Control operations
- `GET /executions/:id` — Execution details
- `GET /stats` — Dashboard statistics

## api-logs

Execution tracing with structured log entries. Each entry has level, category, timestamp, and metadata.

**Key types**: `ExecutionLog`, `LogLevel`, `LogCategory`, `LogFilter`, `LogSession`

**Categories**: Session, Token, ToolCall, ToolResult, Thinking, Delegation, System, Error

## knowledge-graph

Extracts and stores entities and relationships from conversations. LLM-powered extraction (stubbed).

**Key types**: `Entity`, `Relationship`, `EntityType`, `RelationshipType`, `ExtractedKnowledge`

**Entity types**: Person, Organization, Location, Concept, Tool, Project, Custom

## daily-sessions

Manages daily conversation sessions with context continuity and system prompt version tracking.

**Key types**: `DailySession`, `SessionMessage`, `DaySummary`, `Agent`, `SystemPromptCheck`
