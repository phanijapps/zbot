import type { LogSession, ExecutionLog } from '../../services/transport/types';

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

// ---------------------------------------------------------------------------
// SessionWaterfall
// ---------------------------------------------------------------------------

interface SessionWaterfallProps {
  session: LogSession;
  childSessions: LogSession[];
  logs: ExecutionLog[];
}

export function SessionWaterfall({ session, childSessions, logs }: SessionWaterfallProps) {
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
  const toolLogs = logs.filter(
    (l) => l.category === 'tool_call' || l.category === 'delegation' || l.level === 'error',
  );

  return (
    <div className="waterfall">
      <svg viewBox={`0 0 600 ${svgHeight}`} preserveAspectRatio="xMidYMid meet">
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
              <rect
                x={x1}
                y={y}
                width={Math.max(x2 - x1, 4)}
                height={12}
                rx={2}
                fill="var(--success)"
                opacity={0.75}
              />
            </g>
          );
        })}

        {/* Tool dots */}
        {toolLogs.map((log) => {
          const ts = new Date(log.timestamp);
          const cx = timeToX(ts, sessionStart, totalDuration);
          const { color, r } = dotStyle(log);
          return (
            <circle
              key={log.id}
              cx={cx}
              cy={dotRowY}
              r={r}
              fill={color}
            />
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
    </div>
  );
}
