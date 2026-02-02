// ============================================================================
// SOURCE BADGE COMPONENT
// Displays a badge for the trigger source of a session
// ============================================================================

import { Globe, Terminal, Timer, Zap, Puzzle } from 'lucide-react';
import type { TriggerSource } from '../../../services/transport/types';

// Source configuration for badge display
export const SOURCE_CONFIG: Record<
  TriggerSource,
  { label: string; icon: React.ReactNode; color: string }
> = {
  web: { label: 'Web', icon: <Globe size={10} />, color: 'var(--primary)' },
  cli: { label: 'CLI', icon: <Terminal size={10} />, color: 'var(--muted-foreground)' },
  cron: { label: 'Cron', icon: <Timer size={10} />, color: 'var(--warning)' },
  api: { label: 'API', icon: <Zap size={10} />, color: 'var(--success)' },
  plugin: { label: 'Plugin', icon: <Puzzle size={10} />, color: 'var(--primary)' },
};

export interface SourceBadgeProps {
  /** The trigger source to display */
  source: TriggerSource;
  /** Optional additional CSS class */
  className?: string;
}

/**
 * SourceBadge displays a colored badge indicating the trigger source of a session.
 *
 * Sources:
 * - web: User initiated from web UI
 * - cli: User initiated from command line
 * - cron: Scheduled/cron job
 * - api: External API call
 * - plugin: Plugin trigger (Python, JS, etc.)
 */
export function SourceBadge({ source, className = '' }: SourceBadgeProps) {
  const config = SOURCE_CONFIG[source] || SOURCE_CONFIG.web;

  return (
    <span
      className={`badge flex items-center gap-1 text-[10px] ${className}`}
      style={{
        backgroundColor: `color-mix(in srgb, ${config.color} 15%, transparent)`,
        color: config.color,
        padding: '2px 6px',
      }}
      title={`Source: ${config.label}`}
      data-testid={`source-badge-${source}`}
    >
      {config.icon}
      <span data-testid="source-badge-label">{config.label}</span>
    </span>
  );
}

export default SourceBadge;
