import { describe, it, expect } from 'vitest';
import { formatDuration, formatTokens, isInternalTool, extractToolSummary } from './trace-types';

describe('formatDuration', () => {
  it('returns empty for undefined', () => expect(formatDuration(undefined)).toBe(''));
  it('formats milliseconds', () => expect(formatDuration(500)).toBe('500ms'));
  it('formats seconds', () => expect(formatDuration(2500)).toBe('2.5s'));
  it('formats minutes', () => expect(formatDuration(90000)).toBe('1m 30s'));
});

describe('formatTokens', () => {
  it('returns empty for 0', () => expect(formatTokens(0)).toBe(''));
  it('formats small numbers', () => expect(formatTokens(500)).toBe('500 tok'));
  it('formats thousands', () => expect(formatTokens(3500)).toBe('3.5k tok'));
  it('formats millions', () => expect(formatTokens(1500000)).toBe('1.5M tok'));
});

describe('isInternalTool', () => {
  it('identifies internal tools', () => {
    expect(isInternalTool('analyze_intent')).toBe(true);
    expect(isInternalTool('update_plan')).toBe(true);
    expect(isInternalTool('set_session_title')).toBe(true);
  });
  it('identifies external tools', () => {
    expect(isInternalTool('shell')).toBe(false);
    expect(isInternalTool('read')).toBe(false);
  });
});

describe('extractToolSummary', () => {
  it('extracts shell command', () => {
    expect(extractToolSummary('shell', '{"command":"ls -la"}')).toBe('ls -la');
  });
  it('extracts file path for read', () => {
    expect(extractToolSummary('read', '{"path":"src/main.rs"}')).toBe('src/main.rs');
  });
  it('extracts grep pattern', () => {
    expect(extractToolSummary('grep', '{"pattern":"TODO"}')).toBe('/TODO/');
  });
  it('returns empty for unknown tool', () => {
    expect(extractToolSummary('unknown', '{"data":"value"}')).toBe('');
  });
  it('returns empty for invalid JSON', () => {
    expect(extractToolSummary('shell', 'not json')).toBe('');
  });
  it('truncates long commands', () => {
    const longCmd = 'a'.repeat(100);
    const result = extractToolSummary('shell', JSON.stringify({ command: longCmd }));
    expect(result.length).toBeLessThanOrEqual(60);
    expect(result.endsWith('...')).toBe(true);
  });
});
