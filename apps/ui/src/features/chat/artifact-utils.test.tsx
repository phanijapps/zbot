import { describe, it, expect } from 'vitest';
import { formatFileSize, formatJson } from './artifact-utils';

describe('formatFileSize', () => {
  it('returns empty for undefined', () => expect(formatFileSize(undefined)).toBe(''));
  it('returns empty for 0', () => expect(formatFileSize(0)).toBe(''));
  it('formats bytes', () => expect(formatFileSize(500)).toBe('500 B'));
  it('formats KB', () => expect(formatFileSize(2048)).toBe('2.0 KB'));
  it('formats MB', () => expect(formatFileSize(1500000)).toBe('1.4 MB'));
});

describe('formatJson', () => {
  it('formats valid JSON', () => {
    expect(formatJson('{"a":1}')).toBe('{\n  "a": 1\n}');
  });
  it('returns invalid JSON as-is', () => {
    expect(formatJson('not json')).toBe('not json');
  });
});
