# Unit Test Coverage Plan

> **Purpose**: Define unit test coverage targets and priorities for AgentZero
> **Status**: Planning Phase
> **Last Updated**: 2026-02-01

## Table of Contents

1. [Coverage Goals](#coverage-goals)
2. [Backend (Rust) Coverage](#backend-rust-coverage)
3. [Frontend (TypeScript) Coverage](#frontend-typescript-coverage)
4. [Priority Matrix](#priority-matrix)
5. [Testing Patterns](#testing-patterns)

---

## Coverage Goals

### Overall Targets

| Category | Target | Minimum |
|----------|--------|---------|
| Line Coverage | 70% | 60% |
| Branch Coverage | 65% | 55% |
| Critical Paths | 100% | 90% |

### Coverage by Risk Level

| Risk Level | Coverage Target | Examples |
|------------|-----------------|----------|
| Critical | 100% | State transitions, data persistence, auth |
| High | 80% | API handlers, core business logic |
| Medium | 60% | Utility functions, formatting |
| Low | 40% | Logging, debug helpers |

---

## Backend (Rust) Coverage

### Crate Priority List

| Priority | Crate | Target Coverage | Critical Modules |
|----------|-------|-----------------|------------------|
| P0 | `execution-state` | 80% | `repository.rs`, `types.rs`, `service.rs` |
| P0 | `gateway` | 75% | `bus/`, `http/`, `execution/` |
| P1 | `zero-core` | 70% | Message types, agent traits |
| P1 | `zero-provider` | 70% | Provider interface, response parsing |
| P2 | `zero-tool` | 60% | Tool execution, result handling |
| P2 | `api-logs` | 60% | Log persistence, queries |
| P3 | `zero-mcp` | 50% | Protocol implementation |

### execution-state Coverage

```
services/execution-state/src/
├── types.rs           → 90% (Critical data structures)
│   ├── Session            ✓ Construction, serialization
│   ├── AgentExecution     ✓ Status transitions
│   ├── SessionStatus      ✓ All variants
│   ├── ExecutionStatus    ✓ All variants
│   ├── TriggerSource      ✓ Serialization/deserialization
│   └── DashboardStats     ✓ Aggregation logic
│
├── repository.rs      → 85% (Data persistence)
│   ├── create_session          ✓ Success, duplicate handling
│   ├── update_session_status   ✓ Valid transitions, invalid states
│   ├── create_execution        ✓ Success, foreign key violations
│   ├── get_dashboard_stats     ✓ Empty DB, populated DB
│   └── get_sessions_with_executions ✓ Filtering, pagination
│
├── service.rs         → 80% (Business logic)
│   ├── create_session_queued   ✓ State initialization
│   ├── start_session           ✓ Valid/invalid transitions
│   ├── complete_session        ✓ With/without executions
│   └── get_stats               ✓ Caching behavior
│
└── http_routes.rs     → 70% (HTTP layer)
    ├── get_stats_counts        ✓ Response format
    ├── list_sessions_full      ✓ Pagination, filters
    └── error_responses         ✓ All error types
```

### gateway Coverage

```
gateway/src/
├── bus/
│   ├── types.rs       → 90% (Request/Response types)
│   │   ├── SessionRequest      ✓ Builder pattern, validation
│   │   ├── SessionHandle       ✓ Construction
│   │   └── BusError            ✓ All variants, Display impl
│   │
│   └── http_bus.rs    → 80% (Bus implementation)
│       ├── submit              ✓ New session, continue session
│       ├── status              ✓ Found, not found
│       ├── cancel              ✓ Running, already stopped
│       ├── pause               ✓ Valid states, invalid states
│       └── resume              ✓ Paused, not paused
│
├── http/
│   └── gateway_bus.rs → 75% (HTTP endpoints)
│       ├── submit_session      ✓ Success, runner not ready
│       ├── get_status          ✓ Found, not found
│       └── error_mapping       ✓ All BusError variants
│
├── execution/
│   ├── runner.rs      → 75% (Execution orchestration)
│   │   ├── start_execution     ✓ Success, agent not found
│   │   ├── handle_completion   ✓ Normal, error, delegation
│   │   └── handle_delegation   ✓ Subagent creation
│   │
│   └── delegation.rs  → 70% (Subagent management)
│       ├── register            ✓ New, duplicate
│       ├── complete            ✓ Found, not found
│       └── get_parent          ✓ Hierarchy traversal
│
└── state.rs           → 60% (Application state)
    └── AppState construction   ✓ With/without optional components
```

### Unit Test Examples (Rust)

```rust
// services/execution-state/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_transitions() {
        // Queued can go to Running or Cancelled
        assert!(SessionStatus::Queued.can_transition_to(SessionStatus::Running));
        assert!(SessionStatus::Queued.can_transition_to(SessionStatus::Cancelled));
        assert!(!SessionStatus::Queued.can_transition_to(SessionStatus::Completed));
        
        // Running can go to Completed, Crashed, or Cancelled
        assert!(SessionStatus::Running.can_transition_to(SessionStatus::Completed));
        assert!(SessionStatus::Running.can_transition_to(SessionStatus::Crashed));
        assert!(!SessionStatus::Running.can_transition_to(SessionStatus::Queued));
    }

    #[test]
    fn trigger_source_serialization() {
        let sources = vec![
            (TriggerSource::Web, "web"),
            (TriggerSource::Cli, "cli"),
            (TriggerSource::Cron, "cron"),
            (TriggerSource::Api, "api"),
            (TriggerSource::Plugin, "plugin"),
        ];
        
        for (source, expected) in sources {
            let json = serde_json::to_string(&source).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
            
            let parsed: TriggerSource = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, source);
        }
    }

    #[test]
    fn dashboard_stats_default() {
        let stats = DashboardStats::default();
        assert_eq!(stats.sessions_running, 0);
        assert_eq!(stats.sessions_queued, 0);
        assert!(stats.sessions_by_source.is_empty());
    }
}
```

```rust
// gateway/src/bus/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_request_builder() {
        let request = SessionRequest::new("root", "Hello!")
            .with_source(TriggerSource::Plugin)
            .with_priority(10)
            .with_external_ref("test-123");
        
        assert_eq!(request.agent_id, "root");
        assert_eq!(request.message, "Hello!");
        assert_eq!(request.source, TriggerSource::Plugin);
        assert_eq!(request.priority, Some(10));
        assert_eq!(request.external_ref, Some("test-123".to_string()));
    }

    #[test]
    fn session_request_minimal() {
        let request = SessionRequest::new("agent", "message");
        
        assert_eq!(request.source, TriggerSource::Web); // default
        assert!(request.session_id.is_none());
        assert!(request.priority.is_none());
    }

    #[test]
    fn bus_error_display() {
        let errors = vec![
            (BusError::SessionNotFound("sess-123".into()), "Session not found: sess-123"),
            (BusError::AgentError("Missing agent".into()), "Agent error: Missing agent"),
            (BusError::InvalidState { current: "running".into(), action: "start".into() }, 
             "Invalid state transition: cannot start while running"),
        ];
        
        for (error, expected_substr) in errors {
            let msg = error.to_string();
            assert!(msg.contains(expected_substr), "Expected '{}' in '{}'", expected_substr, msg);
        }
    }
}
```

---

## Frontend (TypeScript) Coverage

### Module Priority List

| Priority | Module | Target Coverage | Critical Files |
|----------|--------|-----------------|----------------|
| P0 | `services/transport/` | 80% | `http.ts`, `types.ts`, `websocket.ts` |
| P0 | `features/ops/` | 75% | `WebOpsDashboard.tsx`, components |
| P1 | `features/chat/` | 70% | Message handling, state |
| P1 | `components/ui/` | 65% | Reusable components |
| P2 | `hooks/` | 60% | Custom hooks |
| P2 | `utils/` | 70% | Utility functions |

### services/transport Coverage

```
apps/ui/src/services/transport/
├── types.ts           → 90% (Type definitions + guards)
│   ├── Type guards         ✓ All interfaces
│   ├── Utility types       ✓ Construction helpers
│   └── Validation          ✓ Runtime checks
│
├── http.ts            → 80% (HTTP client)
│   ├── listExecutionSessions   ✓ Success, empty, error
│   ├── getDashboardStats       ✓ Success, error
│   ├── submitSession           ✓ Success, validation error
│   └── Error handling          ✓ Network, 4xx, 5xx
│
└── websocket.ts       → 75% (WebSocket client)
    ├── connect                 ✓ Success, failure, reconnect
    ├── subscribe               ✓ Event filtering
    └── Message parsing         ✓ Valid, invalid, unknown
```

### features/ops Coverage

```
apps/ui/src/features/ops/
├── WebOpsDashboard.tsx    → 75%
│   ├── Initial load           ✓ Loading state, error state, success
│   ├── Auto-refresh           ✓ Interval updates
│   ├── Source filtering       ✓ All sources, single source
│   └── Session expansion      ✓ Toggle behavior
│
├── components/
│   ├── SessionCard.tsx    → 80%
│   │   ├── Status display     ✓ All statuses
│   │   ├── Execution list     ✓ Empty, single, multiple
│   │   └── Actions            ✓ Cancel, view details
│   │
│   ├── SourceBadge.tsx    → 90%
│   │   └── Badge variants     ✓ All TriggerSource values
│   │
│   ├── SourceFilter.tsx   → 85%
│   │   ├── Selection          ✓ Single select, all
│   │   └── Change callback    ✓ Event emission
│   │
│   └── StatsPanel.tsx     → 75%
│       ├── Stat display       ✓ Zero values, large numbers
│       └── Source breakdown   ✓ Empty, populated
```

### Unit Test Examples (TypeScript)

```typescript
// apps/ui/src/services/transport/types.test.ts
import { describe, it, expect } from 'vitest';
import { 
  isSessionWithExecutions, 
  isAgentExecution,
  SessionStatus,
  ExecutionStatus,
  TriggerSource 
} from './types';

describe('Type Guards', () => {
  describe('isSessionWithExecutions', () => {
    it('returns true for valid SessionWithExecutions', () => {
      const valid = {
        session: {
          id: 'sess-123',
          status: 'running' as SessionStatus,
          source: 'web' as TriggerSource,
          created_at: '2026-01-01T00:00:00Z',
        },
        executions: [],
        subagent_count: 0,
      };
      expect(isSessionWithExecutions(valid)).toBe(true);
    });

    it('returns false for missing session', () => {
      expect(isSessionWithExecutions({ executions: [] })).toBe(false);
    });

    it('returns false for null', () => {
      expect(isSessionWithExecutions(null)).toBe(false);
    });
  });

  describe('isAgentExecution', () => {
    it('returns true for valid AgentExecution', () => {
      const valid = {
        id: 'exec-123',
        session_id: 'sess-123',
        agent_id: 'root',
        status: 'running' as ExecutionStatus,
        conversation_id: 'conv-123',
        turn_count: 5,
        started_at: '2026-01-01T00:00:00Z',
      };
      expect(isAgentExecution(valid)).toBe(true);
    });
  });
});

describe('Type Enums', () => {
  it('SessionStatus has expected values', () => {
    const statuses: SessionStatus[] = ['queued', 'running', 'completed', 'crashed', 'cancelled'];
    expect(statuses).toHaveLength(5);
  });

  it('TriggerSource has expected values', () => {
    const sources: TriggerSource[] = ['web', 'cli', 'cron', 'api', 'plugin'];
    expect(sources).toHaveLength(5);
  });
});
```

```typescript
// apps/ui/src/features/ops/components/SourceBadge.test.tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SourceBadge } from './SourceBadge';
import type { TriggerSource } from '@/services/transport/types';

describe('SourceBadge', () => {
  const sources: TriggerSource[] = ['web', 'cli', 'cron', 'api', 'plugin'];

  it.each(sources)('renders badge for source: %s', (source) => {
    render(<SourceBadge source={source} />);
    expect(screen.getByText(source)).toBeInTheDocument();
  });

  it('applies correct color classes', () => {
    const { container } = render(<SourceBadge source="web" />);
    expect(container.firstChild).toHaveClass('bg-blue');
  });

  it('renders with custom className', () => {
    const { container } = render(<SourceBadge source="cli" className="custom" />);
    expect(container.firstChild).toHaveClass('custom');
  });
});
```

```typescript
// apps/ui/src/services/transport/http.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getDashboardStats, listExecutionSessions } from './http';

// Mock fetch globally
const mockFetch = vi.fn();
global.fetch = mockFetch;

describe('HTTP Transport', () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  describe('getDashboardStats', () => {
    it('returns stats on success', async () => {
      const mockStats = {
        sessions_running: 2,
        sessions_queued: 1,
        sessions_completed: 10,
        executions_running: 3,
        sessions_by_source: { web: 5, cli: 3 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockStats),
      });

      const stats = await getDashboardStats();
      expect(stats).toEqual(mockStats);
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/api/executions/stats/counts'),
        expect.any(Object)
      );
    });

    it('throws on network error', async () => {
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      await expect(getDashboardStats()).rejects.toThrow('Network error');
    });

    it('throws on non-ok response', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
      });

      await expect(getDashboardStats()).rejects.toThrow();
    });
  });

  describe('listExecutionSessions', () => {
    it('returns sessions array on success', async () => {
      const mockSessions = [
        {
          session: { id: 'sess-1', status: 'running', source: 'web' },
          executions: [],
          subagent_count: 0,
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockSessions),
      });

      const sessions = await listExecutionSessions();
      expect(sessions).toHaveLength(1);
      expect(sessions[0].session.id).toBe('sess-1');
    });

    it('returns empty array when no sessions', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve([]),
      });

      const sessions = await listExecutionSessions();
      expect(sessions).toEqual([]);
    });
  });
});
```

---

## Priority Matrix

### What to Test First

```
                  Business Impact
                       HIGH
                        │
    ┌───────────────────┼───────────────────┐
    │                   │                   │
    │   Medium Priority │   HIGH PRIORITY   │
    │   (Nice to have)  │   (Test first!)   │
    │                   │                   │
    │   - UI polish     │   - State mgmt    │
    │   - Animations    │   - Data persist  │
    │   - Themes        │   - Auth/Security │
    │                   │   - API contracts │
LOW ├───────────────────┼───────────────────┤ HIGH
Bug │                   │                   │ Bug
Risk│   LOW PRIORITY    │   Medium Priority │ Risk
    │   (Skip or basic) │   (Cover basics)  │
    │                   │                   │
    │   - Static text   │   - Validation    │
    │   - Config files  │   - Error msgs    │
    │   - Dev tooling   │   - Edge cases    │
    │                   │                   │
    └───────────────────┼───────────────────┘
                        │
                       LOW
```

### Critical Path Coverage (Must be 100%)

1. **Session Lifecycle**
   - Create session → Running → Completed/Crashed/Cancelled
   - Subagent delegation chain
   - State persistence across restarts

2. **Data Integrity**
   - Foreign key relationships (execution → session)
   - Status transition validation
   - Concurrent update handling

3. **API Contracts**
   - Request validation
   - Response format consistency
   - Error response structure

---

## Testing Patterns

### Rust: Table-Driven Tests

```rust
#[test]
fn status_can_transition() {
    let cases = vec![
        (SessionStatus::Queued, SessionStatus::Running, true),
        (SessionStatus::Queued, SessionStatus::Cancelled, true),
        (SessionStatus::Queued, SessionStatus::Completed, false),
        (SessionStatus::Running, SessionStatus::Completed, true),
        (SessionStatus::Running, SessionStatus::Crashed, true),
        (SessionStatus::Running, SessionStatus::Queued, false),
        (SessionStatus::Completed, SessionStatus::Running, false),
    ];

    for (from, to, expected) in cases {
        assert_eq!(
            from.can_transition_to(to),
            expected,
            "{:?} -> {:?} should be {}",
            from,
            to,
            expected
        );
    }
}
```

### TypeScript: Parameterized Tests

```typescript
describe.each([
  ['queued', 'bg-yellow-500'],
  ['running', 'bg-blue-500'],
  ['completed', 'bg-green-500'],
  ['crashed', 'bg-red-500'],
  ['cancelled', 'bg-gray-500'],
])('StatusBadge for %s', (status, expectedClass) => {
  it(`applies ${expectedClass}`, () => {
    const { container } = render(<StatusBadge status={status as SessionStatus} />);
    expect(container.firstChild).toHaveClass(expectedClass);
  });
});
```

### React: Component Testing Pattern

```typescript
// Arrange - Act - Assert pattern
describe('SessionCard', () => {
  const defaultProps = {
    session: mockSession(),
    executions: [],
    onCancel: vi.fn(),
    onExpand: vi.fn(),
  };

  it('shows loading state while fetching details', () => {
    // Arrange
    render(<SessionCard {...defaultProps} isLoading={true} />);
    
    // Act (none needed for initial render)
    
    // Assert
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('calls onCancel when cancel button clicked', async () => {
    // Arrange
    const onCancel = vi.fn();
    render(<SessionCard {...defaultProps} onCancel={onCancel} />);
    
    // Act
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    
    // Assert
    expect(onCancel).toHaveBeenCalledWith(defaultProps.session.id);
  });
});
```

---

## Coverage Measurement

### Rust (cargo-tarpaulin)

```bash
# Install
cargo install cargo-tarpaulin

# Run with coverage
cargo tarpaulin --workspace --out Html --output-dir coverage/

# CI command (threshold enforcement)
cargo tarpaulin --workspace --fail-under 60
```

### TypeScript (Vitest)

```typescript
// vitest.config.ts
export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      exclude: ['**/*.test.{ts,tsx}', '**/node_modules/**'],
      thresholds: {
        lines: 60,
        branches: 55,
        functions: 60,
        statements: 60,
      },
    },
  },
});
```

```bash
# Run with coverage
npm run test -- --coverage

# CI command (threshold enforcement)
npm run test -- --coverage --coverage.thresholdAutoUpdate=false
```

---

## Related Documents

- [Test Automation Plan](./test-automation-plan.md)
- [Backend API Tests](./backend-api-tests.md)
- [Frontend UI Tests](./frontend-ui-tests.md)
- [E2E Scenarios](./e2e-scenarios.md)
