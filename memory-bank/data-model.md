# Data Model

Database schema for AgentZero gateway. SQLite-based.

## Entity Relationship Diagram

```mermaid
erDiagram
    sessions ||--o{ agent_executions : contains
    agent_executions ||--o{ agent_executions : "parent-child (delegation)"
    agent_executions ||--o{ messages : has

    sessions {
        text id PK "sess-{uuid}"
        text status "running|paused|completed|crashed"
        text root_agent_id
        text title "optional, user-provided"
        text created_at
        text started_at
        text completed_at
        integer total_tokens_in
        integer total_tokens_out
        text metadata "JSON"
    }

    agent_executions {
        text id PK "exec-{uuid}"
        text session_id FK
        text agent_id
        text parent_execution_id FK "null for root"
        text delegation_type "root|sequential|parallel"
        text task "task description for subagents"
        text status "queued|running|paused|completed|crashed|cancelled"
        text started_at
        text completed_at
        integer tokens_in
        integer tokens_out
        text checkpoint "JSON for resumption"
        text error
        text log_path "relative path to log file"
    }

    messages {
        text id PK
        text execution_id FK
        text role "user|assistant|tool|system"
        text content
        text created_at
        integer token_count
        text tool_calls "JSON"
        text tool_results "JSON"
    }
```

## Tables

### sessions
Top-level container for a user's work session. Starts with `/new` or hook message.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | `sess-{uuid}` |
| status | TEXT | `running`, `paused`, `completed`, `crashed` |
| root_agent_id | TEXT | The primary agent for this session |
| title | TEXT | Optional user-provided title |
| created_at | TEXT | ISO8601 timestamp |
| started_at | TEXT | When first execution began |
| completed_at | TEXT | When session finished |
| total_tokens_in | INTEGER | Sum of all agent execution tokens |
| total_tokens_out | INTEGER | Sum of all agent execution tokens |
| metadata | TEXT | JSON metadata |

### agent_executions
An agent's participation in a session. Root agent or delegated subagent.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | `exec-{uuid}` |
| session_id | TEXT FK | References sessions.id |
| agent_id | TEXT | Agent ID from config |
| parent_execution_id | TEXT FK | Null for root, references parent for subagents |
| delegation_type | TEXT | `root`, `sequential`, `parallel` |
| task | TEXT | Task description (for subagents) |
| status | TEXT | `queued`, `running`, `paused`, `completed`, `crashed`, `cancelled` |
| started_at | TEXT | When execution began |
| completed_at | TEXT | When execution finished |
| tokens_in | INTEGER | Input tokens consumed |
| tokens_out | INTEGER | Output tokens generated |
| checkpoint | TEXT | JSON checkpoint for resumption |
| error | TEXT | Error message if crashed |
| log_path | TEXT | Relative path to execution log file |

### messages
Individual messages in an agent's conversation.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID |
| execution_id | TEXT FK | References agent_executions.id |
| role | TEXT | `user`, `assistant`, `tool`, `system` |
| content | TEXT | Message content |
| created_at | TEXT | ISO8601 timestamp |
| token_count | INTEGER | Token count for this message |
| tool_calls | TEXT | JSON array of tool calls |
| tool_results | TEXT | JSON array of tool results |

## Indexes

```sql
-- sessions
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_created ON sessions(created_at);

-- agent_executions
CREATE INDEX idx_executions_session ON agent_executions(session_id);
CREATE INDEX idx_executions_parent ON agent_executions(parent_execution_id);
CREATE INDEX idx_executions_status ON agent_executions(status);
CREATE INDEX idx_executions_agent ON agent_executions(agent_id);

-- messages
CREATE INDEX idx_messages_execution ON messages(execution_id);
CREATE INDEX idx_messages_created ON messages(created_at);
```

## Execution Logs (Filesystem)

Logs are stored as JSON Lines files on disk, not in the database.

```
data/
└── logs/
    └── {session_id}/
        ├── exec-abc123.jsonl     # root agent logs
        ├── exec-def456.jsonl     # subagent 1 logs
        └── exec-ghi789.jsonl     # subagent 2 logs
```

Each line is a JSON object:
```json
{"ts":"2024-01-15T10:00:00Z","level":"info","cat":"llm","msg":"Starting turn","meta":{}}
{"ts":"2024-01-15T10:00:01Z","level":"debug","cat":"tool","msg":"Calling read_file","meta":{"path":"/foo"}}
```

**Log levels**: `debug`, `info`, `warn`, `error`
**Categories**: `llm`, `tool`, `delegation`, `middleware`, `system`

To clean up: delete `data/logs/{session_id}/` directory when session is deleted.

## Status Semantics

### Session Status
- `running` - At least one agent execution is running
- `paused` - User paused; all executions should pause
- `completed` - All executions completed successfully
- `crashed` - Root execution crashed (subagent crash may not crash session)

### Execution Status
- `queued` - Waiting to start
- `running` - Currently executing
- `paused` - Paused (session paused or waiting)
- `completed` - Finished successfully
- `crashed` - Failed with error
- `cancelled` - Cancelled by user or parent

## Delegation Flow

```
User sends message
    │
    ▼
Session created (or resumed)
    │
    ▼
Root agent_execution created (delegation_type=root)
    │
    ▼
Root agent runs, decides to delegate
    │
    ├──► Subagent A execution (delegation_type=parallel, parent=root)
    │        └── Runs independently
    │
    ├──► Subagent B execution (delegation_type=parallel, parent=root)
    │        └── Runs independently
    │
    ▼
Root waits for parallel subagents
    │
    ▼
Root processes results, delegates again
    │
    └──► Subagent C execution (delegation_type=sequential, parent=root)
             └── Runs, returns result
    │
    ▼
Root completes → Session completes
```

## Stats Queries

**Active sessions:**
```sql
SELECT COUNT(*) FROM sessions WHERE status = 'running';
```

**Completed sessions:**
```sql
SELECT COUNT(*) FROM sessions WHERE status = 'completed';
```

**Session with all its executions:**
```sql
SELECT s.*, e.*
FROM sessions s
LEFT JOIN agent_executions e ON e.session_id = s.id
WHERE s.id = ?
ORDER BY e.started_at;
```
