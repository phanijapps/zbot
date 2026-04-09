import { useState, useRef, useCallback, useMemo } from 'react';
import type { LogSession, ExecutionLog } from '../../services/transport/types';
import { WaterfallTooltip, type TooltipData } from './WaterfallTooltip';
import { WaterfallSlideOut, type SlideOutData } from './WaterfallSlideOut';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LABEL_WIDTH = 70;
const BAR_WIDTH = 530; // 600 - LABEL_WIDTH

/** Map a timestamp to an x-coordinate within the SVG viewBox. */
function timeToX(ts: Date, sessionStart: number, totalDuration: number): number {
  const elapsed = ts.getTime() - sessionStart;
  return LABEL_WIDTH + (elapsed / totalDuration) * BAR_WIDTH;
}

/** Pick dot color and radius for a log entry. */
function dotStyle(log: ExecutionLog): { color: string; r: number } {
  if (log.level === 'error') return { color: '#ef4444', r: 3 };
  if (log.category === 'tool_call' && log.message.toLowerCase().includes('memory'))
    return { color: '#8b5cf6', r: 2 };
  if (log.category === 'delegation') return { color: '#10b981', r: 2 };
  // default tool_call or anything else
  return { color: '#f59e0b', r: 2 };
}

/** Format a duration in ms as a compact time label (0s, 5m, 1h 2m, etc.) */
function formatAxisLabel(ms: number): string {
  if (ms < 1000) return '0s';
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`;
  if (ms < 3_600_000) return `${Math.round(ms / 60_000)}m`;
  const h = Math.floor(ms / 3_600_000);
  const m = Math.round((ms % 3_600_000) / 60_000);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

/** Truncate a label to fit in the reserved label area. */
function truncateLabel(name: string, maxChars = 8): string {
  return name.length > maxChars ? name.slice(0, maxChars) + '\u2026' : name;
}

/**
 * Convert an SVG-coordinate (viewBox space) to pixel-space
 * relative to the container div.
 */
function svgToContainer(
  svgX: number,
  svgY: number,
  svgEl: SVGSVGElement,
  containerEl: HTMLDivElement,
): { x: number; y: number } {
  // Get the SVG's CTM (current transformation matrix)
  const pt = svgEl.createSVGPoint();
  pt.x = svgX;
  pt.y = svgY;
  const ctm = svgEl.getScreenCTM();
  if (!ctm) return { x: 0, y: 0 };
  const screenPt = pt.matrixTransform(ctm);
  const containerRect = containerEl.getBoundingClientRect();
  return {
    x: screenPt.x - containerRect.left,
    y: screenPt.y - containerRect.top,
  };
}

// ---------------------------------------------------------------------------
// Types for interactive items
// ---------------------------------------------------------------------------

interface NavigableItem {
  type: 'tool' | 'delegation' | 'error';
  log?: ExecutionLog;
  childSession?: LogSession;
  childLogs?: ExecutionLog[];
}

// ---------------------------------------------------------------------------
// SessionWaterfall
// ---------------------------------------------------------------------------

interface SessionWaterfallProps {
  session: LogSession;
  childSessions: LogSession[];
  logs: ExecutionLog[];
}

export function SessionWaterfall({ session, childSessions, logs }: SessionWaterfallProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  const [hoveredItem, setHoveredItem] = useState<TooltipData | null>(null);
  const [selectedItem, setSelectedItem] = useState<SlideOutData | null>(null);

  // ---- Timing envelope ----------------------------------------------------
  const sessionStart = new Date(session.started_at).getTime();

  // End: use ended_at if available, otherwise latest log timestamp or now
  const endCandidates: number[] = [];
  if (session.ended_at) endCandidates.push(new Date(session.ended_at).getTime());
  for (const child of childSessions) {
    if (child.ended_at) endCandidates.push(new Date(child.ended_at).getTime());
  }
  for (const log of logs) {
    endCandidates.push(new Date(log.timestamp).getTime());
  }
  const sessionEnd = endCandidates.length > 0 ? Math.max(...endCandidates) : sessionStart + 1000;
  const totalDuration = Math.max(sessionEnd - sessionStart, 1); // avoid division by zero

  // ---- Agent lanes --------------------------------------------------------
  const agents = [session, ...childSessions];
  const laneHeight = 18;
  const laneY0 = 10;
  const agentLanesBottom = laneY0 + agents.length * laneHeight;
  const dotRowY = agentLanesBottom + 10;
  const axisY = dotRowY + 16;
  const svgHeight = axisY + 14; // a bit of padding

  // ---- Time axis ticks ----------------------------------------------------
  const tickCount = 5;
  const ticks: { x: number; label: string }[] = [];
  for (let i = 0; i < tickCount; i++) {
    const frac = i / (tickCount - 1);
    const ms = frac * totalDuration;
    ticks.push({
      x: LABEL_WIDTH + frac * BAR_WIDTH,
      label: formatAxisLabel(ms),
    });
  }

  // ---- Tool dots ----------------------------------------------------------
  const toolLogs = useMemo(
    () =>
      logs.filter(
        (l) => l.category === 'tool_call' || l.category === 'delegation' || l.level === 'error',
      ),
    [logs],
  );

  // ---- Build navigable items list (tool dots + delegation bars) -----------
  const navigableItems: NavigableItem[] = useMemo(() => {
    const items: NavigableItem[] = [];

    // Add delegation bars (child sessions)
    for (const child of childSessions) {
      const childLogs = logs.filter((l) => l.agent_id === child.agent_id);
      items.push({
        type: 'delegation',
        childSession: child,
        childLogs,
      });
    }

    // Add tool dots
    for (const log of toolLogs) {
      items.push({
        type: log.level === 'error' ? 'error' : 'tool',
        log,
      });
    }

    // Sort chronologically
    items.sort((a, b) => {
      const tsA = a.log?.timestamp ?? a.childSession?.started_at ?? '';
      const tsB = b.log?.timestamp ?? b.childSession?.started_at ?? '';
      return tsA.localeCompare(tsB);
    });

    return items;
  }, [childSessions, logs, toolLogs]);

  // ---- Compute the time range of the currently hovered delegation --------
  const hoveredDelegationRange = useMemo(() => {
    if (!hoveredItem || hoveredItem.type !== 'delegation' || !hoveredItem.childSession) return null;
    const cs = hoveredItem.childSession;
    const start = new Date(cs.started_at).getTime();
    const end = cs.ended_at ? new Date(cs.ended_at).getTime() : sessionEnd;
    return { start, end };
  }, [hoveredItem, sessionEnd]);

  // ---- Get surrounding logs for the selected item (error context) --------
  const surroundingLogs = useMemo(() => {
    if (!selectedItem?.log) return [];
    const logIndex = logs.findIndex((l) => l.id === selectedItem.log!.id);
    if (logIndex < 0) return [];
    const start = Math.max(0, logIndex - 3);
    const end = Math.min(logs.length, logIndex + 4);
    return logs.slice(start, end);
  }, [selectedItem, logs]);

  // ---- Event helpers ------------------------------------------------------
  const getContainerRect = useCallback((): DOMRect | null => {
    return containerRef.current?.getBoundingClientRect() ?? null;
  }, []);

  const handleDotHover = useCallback(
    (log: ExecutionLog, svgX: number, svgY: number) => {
      if (!svgRef.current || !containerRef.current) return;
      const pos = svgToContainer(svgX, svgY, svgRef.current, containerRef.current);
      setHoveredItem({
        type: log.level === 'error' ? 'error' : 'tool',
        x: pos.x,
        y: pos.y,
        log,
      });
    },
    [],
  );

  const handleBarHover = useCallback(
    (child: LogSession, svgX: number, svgY: number) => {
      if (!svgRef.current || !containerRef.current) return;
      const pos = svgToContainer(svgX, svgY, svgRef.current, containerRef.current);
      const childLogs = logs.filter((l) => l.agent_id === child.agent_id);
      setHoveredItem({
        type: 'delegation',
        x: pos.x,
        y: pos.y,
        childSession: child,
        childLogs,
      });
    },
    [logs],
  );

  const handleDotClick = useCallback(
    (log: ExecutionLog) => {
      const idx = navigableItems.findIndex(
        (item) => item.log?.id === log.id,
      );
      setSelectedItem({
        type: log.level === 'error' ? 'error' : 'tool',
        log,
        index: idx >= 0 ? idx : 0,
      });
    },
    [navigableItems],
  );

  const handleBarClick = useCallback(
    (child: LogSession) => {
      const childLogs = logs.filter((l) => l.agent_id === child.agent_id);
      const idx = navigableItems.findIndex(
        (item) => item.childSession?.session_id === child.session_id,
      );
      setSelectedItem({
        type: 'delegation',
        childSession: child,
        childLogs,
        index: idx >= 0 ? idx : 0,
      });
    },
    [navigableItems, logs],
  );

  const handleNavigate = useCallback(
    (direction: 'prev' | 'next') => {
      if (!selectedItem) return;
      const newIdx =
        direction === 'prev'
          ? Math.max(0, selectedItem.index - 1)
          : Math.min(navigableItems.length - 1, selectedItem.index + 1);
      const item = navigableItems[newIdx];
      if (!item) return;
      setSelectedItem({
        type: item.type,
        log: item.log,
        childSession: item.childSession,
        childLogs: item.childLogs,
        index: newIdx,
      });
    },
    [selectedItem, navigableItems],
  );

  const handleMouseLeave = useCallback(() => {
    setHoveredItem(null);
  }, []);

  // ---- Is a dot within the hovered delegation's time range? ---------------
  const isDotInDelegationRange = useCallback(
    (logTs: string): boolean => {
      if (!hoveredDelegationRange) return false;
      const t = new Date(logTs).getTime();
      return t >= hoveredDelegationRange.start && t <= hoveredDelegationRange.end;
    },
    [hoveredDelegationRange],
  );

  return (
    <div className="waterfall waterfall--interactive" ref={containerRef} style={{ position: 'relative' }}>
      <svg
        ref={svgRef}
        viewBox={`0 0 600 ${svgHeight}`}
        preserveAspectRatio="xMidYMid meet"
        onMouseLeave={handleMouseLeave}
      >
        {/* Agent lanes */}
        {agents.map((agent, i) => {
          const y = laneY0 + i * laneHeight;
          const isRoot = i === 0;

          // Root lane: translucent full-width background + opaque start/end segments
          if (isRoot) {
            const startEnd = session.ended_at
              ? timeToX(new Date(session.ended_at), sessionStart, totalDuration)
              : LABEL_WIDTH + BAR_WIDTH;
            return (
              <g key={agent.session_id}>
                {/* label */}
                <text
                  x={2}
                  y={y + 10}
                  fontSize="9"
                  fill="var(--muted-foreground)"
                  fontFamily="var(--font-mono)"
                >
                  {truncateLabel(agent.agent_name)}
                </text>
                {/* translucent background */}
                <rect
                  x={LABEL_WIDTH}
                  y={y}
                  width={BAR_WIDTH}
                  height={12}
                  rx={2}
                  fill="var(--primary)"
                  opacity={0.15}
                />
                {/* opaque start segment */}
                <rect
                  x={LABEL_WIDTH}
                  y={y}
                  width={Math.min(20, BAR_WIDTH)}
                  height={12}
                  rx={2}
                  fill="var(--primary)"
                  opacity={0.7}
                />
                {/* opaque end segment */}
                <rect
                  x={Math.max(startEnd - 20, LABEL_WIDTH)}
                  y={y}
                  width={20}
                  height={12}
                  rx={2}
                  fill="var(--primary)"
                  opacity={0.7}
                />
              </g>
            );
          }

          // Child lanes: colored bar positioned by start/end time
          const childStart = new Date(agent.started_at).getTime();
          const childEnd = agent.ended_at
            ? new Date(agent.ended_at).getTime()
            : sessionEnd;
          const x1 = timeToX(new Date(childStart), sessionStart, totalDuration);
          const x2 = timeToX(new Date(childEnd), sessionStart, totalDuration);
          const barWidth = Math.max(x2 - x1, 4);
          const barMidX = x1 + barWidth / 2;
          const barMidY = y + 6;

          const isSelected =
            selectedItem?.type === 'delegation' &&
            selectedItem.childSession?.session_id === agent.session_id;
          const isHovered =
            hoveredItem?.type === 'delegation' &&
            hoveredItem.childSession?.session_id === agent.session_id;

          return (
            <g key={agent.session_id}>
              <text
                x={2}
                y={y + 10}
                fontSize="9"
                fill="var(--muted-foreground)"
                fontFamily="var(--font-mono)"
              >
                {truncateLabel(agent.agent_name)}
              </text>
              {/* Selection highlight ring */}
              {isSelected && (
                <rect
                  x={x1 - 1}
                  y={y - 1}
                  width={barWidth + 2}
                  height={14}
                  rx={3}
                  fill="none"
                  stroke="var(--primary)"
                  strokeWidth={1.5}
                  className="waterfall-bar--selected-ring"
                />
              )}
              <rect
                className="waterfall-bar"
                x={x1}
                y={y}
                width={barWidth}
                height={12}
                rx={2}
                fill="var(--success)"
                opacity={isHovered ? 0.95 : 0.75}
                onMouseEnter={() => handleBarHover(agent, barMidX, barMidY)}
                onMouseLeave={handleMouseLeave}
                onClick={(e) => { e.stopPropagation(); handleBarClick(agent); }}
              />
            </g>
          );
        })}

        {/* Tool dots */}
        {toolLogs.map((log) => {
          const ts = new Date(log.timestamp);
          const cx = timeToX(ts, sessionStart, totalDuration);
          const { color, r } = dotStyle(log);

          const isSelected =
            selectedItem?.type !== 'delegation' &&
            selectedItem?.log?.id === log.id;
          const isInDelegationRange = isDotInDelegationRange(log.timestamp);

          return (
            <g key={log.id}>
              {/* Selection pulsing ring */}
              {isSelected && (
                <circle
                  cx={cx}
                  cy={dotRowY}
                  r={r + 3}
                  fill="none"
                  stroke={color}
                  strokeWidth={1}
                  className="waterfall-dot--selected-ring"
                />
              )}
              {/* Connected highlight when hovering delegation */}
              {isInDelegationRange && (
                <circle
                  cx={cx}
                  cy={dotRowY}
                  r={r + 2}
                  fill={color}
                  opacity={0.2}
                  className="waterfall-dot--delegation-highlight"
                />
              )}
              <circle
                className="waterfall-dot"
                cx={cx}
                cy={dotRowY}
                r={r}
                fill={color}
                onMouseEnter={() => handleDotHover(log, cx, dotRowY)}
                onMouseLeave={handleMouseLeave}
                onClick={(e) => { e.stopPropagation(); handleDotClick(log); }}
              />
            </g>
          );
        })}

        {/* Time axis */}
        <line
          x1={LABEL_WIDTH}
          y1={axisY}
          x2={LABEL_WIDTH + BAR_WIDTH}
          y2={axisY}
          stroke="var(--border)"
          strokeWidth={1}
        />
        {ticks.map((t, i) => (
          <text
            key={i}
            x={t.x}
            y={axisY + 10}
            fontSize="8"
            fill="var(--muted-foreground)"
            fontFamily="var(--font-mono)"
            textAnchor="middle"
          >
            {t.label}
          </text>
        ))}
      </svg>

      {/* HTML tooltip overlay — positioned over the SVG */}
      <WaterfallTooltip data={hoveredItem} containerRect={getContainerRect()} />

      {/* Slide-out detail panel */}
      {selectedItem && (
        <WaterfallSlideOut
          data={selectedItem}
          totalItems={navigableItems.length}
          surroundingLogs={surroundingLogs}
          onClose={() => setSelectedItem(null)}
          onNavigate={handleNavigate}
        />
      )}
    </div>
  );
}
