// ============================================================================
// SOURCE BADGE COMPONENT TESTS
// Unit tests for SourceBadge component
// ============================================================================

import { describe, it, expect } from 'vitest';
import { render, screen } from '@/test/utils';
import { SourceBadge, SOURCE_CONFIG } from './SourceBadge';
import type { TriggerSource } from '@/services/transport/types';

// All valid trigger sources
const ALL_SOURCES: TriggerSource[] = ['web', 'cli', 'cron', 'api', 'connector'];

describe('SourceBadge', () => {
  // ==================== Rendering Tests ====================

  describe('Rendering', () => {
    it.each(ALL_SOURCES)('renders badge for source: %s', (source) => {
      render(<SourceBadge source={source} />);

      const label = screen.getByTestId('source-badge-label');
      expect(label).toBeInTheDocument();
      expect(label.textContent).toBe(SOURCE_CONFIG[source].label);
    });

    it.each(ALL_SOURCES)('has correct test id for source: %s', (source) => {
      render(<SourceBadge source={source} />);

      const badge = screen.getByTestId(`source-badge-${source}`);
      expect(badge).toBeInTheDocument();
    });

    it('renders with correct label text', () => {
      render(<SourceBadge source="web" />);
      expect(screen.getByText('Web')).toBeInTheDocument();

      render(<SourceBadge source="cli" />);
      expect(screen.getByText('CLI')).toBeInTheDocument();

      render(<SourceBadge source="cron" />);
      expect(screen.getByText('Cron')).toBeInTheDocument();

      render(<SourceBadge source="api" />);
      expect(screen.getByText('API')).toBeInTheDocument();

      render(<SourceBadge source="connector" />);
      expect(screen.getByText('Connector')).toBeInTheDocument();
    });
  });

  // ==================== Styling Tests ====================

  describe('Styling', () => {
    it('applies badge base class', () => {
      render(<SourceBadge source="web" />);

      const badge = screen.getByTestId('source-badge-web');
      expect(badge).toHaveClass('badge');
    });

    it('applies flex layout classes', () => {
      render(<SourceBadge source="cli" />);

      const badge = screen.getByTestId('source-badge-cli');
      expect(badge).toHaveClass('flex', 'items-center', 'gap-1');
    });

    it('applies custom className when provided', () => {
      render(<SourceBadge source="cron" className="custom-class" />);

      const badge = screen.getByTestId('source-badge-cron');
      expect(badge).toHaveClass('custom-class');
    });

    it('applies inline styles for color', () => {
      render(<SourceBadge source="api" />);

      const badge = screen.getByTestId('source-badge-api');
      expect(badge).toHaveStyle({ color: SOURCE_CONFIG.api.color });
    });
  });

  // ==================== Accessibility Tests ====================

  describe('Accessibility', () => {
    it.each(ALL_SOURCES)('has title attribute for source: %s', (source) => {
      render(<SourceBadge source={source} />);

      const badge = screen.getByTestId(`source-badge-${source}`);
      expect(badge).toHaveAttribute('title', `Source: ${SOURCE_CONFIG[source].label}`);
    });
  });

  // ==================== Configuration Tests ====================

  describe('SOURCE_CONFIG', () => {
    it('has configuration for all sources', () => {
      ALL_SOURCES.forEach((source) => {
        expect(SOURCE_CONFIG[source]).toBeDefined();
        expect(SOURCE_CONFIG[source].label).toBeDefined();
        expect(SOURCE_CONFIG[source].icon).toBeDefined();
        expect(SOURCE_CONFIG[source].color).toBeDefined();
      });
    });

    it('all labels are non-empty strings', () => {
      ALL_SOURCES.forEach((source) => {
        expect(typeof SOURCE_CONFIG[source].label).toBe('string');
        expect(SOURCE_CONFIG[source].label.length).toBeGreaterThan(0);
      });
    });

    it('all colors are CSS custom properties', () => {
      ALL_SOURCES.forEach((source) => {
        expect(SOURCE_CONFIG[source].color).toMatch(/^var\(--/);
      });
    });
  });

  // ==================== Edge Cases ====================

  describe('Edge Cases', () => {
    it('falls back to web config for unknown source', () => {
      // TypeScript would normally catch this, but testing runtime behavior
      // @ts-expect-error - Testing unknown source fallback
      render(<SourceBadge source="unknown" />);

      // Should fall back to web config based on the implementation
      const label = screen.getByTestId('source-badge-label');
      expect(label.textContent).toBe('Web');
    });

    it('handles multiple instances', () => {
      render(
        <>
          <SourceBadge source="web" />
          <SourceBadge source="cli" />
          <SourceBadge source="api" />
        </>
      );

      expect(screen.getByTestId('source-badge-web')).toBeInTheDocument();
      expect(screen.getByTestId('source-badge-cli')).toBeInTheDocument();
      expect(screen.getByTestId('source-badge-api')).toBeInTheDocument();
    });

    it('renders empty className correctly', () => {
      render(<SourceBadge source="connector" className="" />);

      const badge = screen.getByTestId('source-badge-connector');
      expect(badge).toHaveClass('badge');
    });
  });
});
