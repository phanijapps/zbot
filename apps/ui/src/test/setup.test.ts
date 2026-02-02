import { describe, it, expect } from 'vitest';
import { server } from './mocks/server';
import { createMockSession, createMockStats } from './mocks/handlers';

describe('Test Infrastructure', () => {
  describe('MSW Server', () => {
    it('server is defined', () => {
      expect(server).toBeDefined();
    });

    it('can fetch mocked stats endpoint', async () => {
      const response = await fetch(
        'http://localhost:18791/api/executions/stats/counts'
      );
      expect(response.ok).toBe(true);

      const data = await response.json();
      expect(data.sessions_running).toBeDefined();
      expect(data.sessions_by_source).toBeDefined();
    });

    it('can fetch mocked sessions endpoint', async () => {
      const response = await fetch(
        'http://localhost:18791/api/executions/v2/sessions/full'
      );
      expect(response.ok).toBe(true);

      const data = await response.json();
      expect(Array.isArray(data)).toBe(true);
      expect(data.length).toBeGreaterThan(0);
    });
  });

  describe('Mock Factories', () => {
    it('createMockSession creates valid session', () => {
      const session = createMockSession('sess-test', 'running', 'web');

      expect(session.id).toBe('sess-test');
      expect(session.status).toBe('running');
      expect(session.source).toBe('web');
      expect(session.executions).toHaveLength(1);
    });

    it('createMockSession handles different statuses', () => {
      const queued = createMockSession('sess-1', 'queued');
      const completed = createMockSession('sess-2', 'completed');

      expect(queued.started_at).toBeUndefined();
      expect(completed.completed_at).toBeDefined();
    });

    it('createMockStats creates valid stats', () => {
      const stats = createMockStats();

      expect(stats.sessions_running).toBe(2);
      expect(stats.sessions_queued).toBe(1);
      expect(stats.sessions_by_source.web).toBe(8);
    });
  });
});
