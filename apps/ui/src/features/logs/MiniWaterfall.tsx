interface MiniWaterfallProps {
  startedAt: string;
  endedAt?: string;
  durationMs?: number;
  status: string;
  childCount: number;
}

export function MiniWaterfall({ endedAt, status, childCount }: MiniWaterfallProps) {
  const isRunning = !endedAt;

  // Root span always fills the viewBox width; opacity signals running state.
  const rootWidth = 300;

  const rootOpacity = isRunning ? 0.5 : 1;

  // Distribute child spans evenly across the root duration
  const childRects: { x: number; width: number }[] = [];
  if (childCount > 0) {
    const childWidth = Math.max(4, (rootWidth - 4) / (childCount * 1.5));
    const gap = childCount > 1 ? (rootWidth - childWidth * childCount) / (childCount - 1) : 0;
    for (let i = 0; i < childCount; i++) {
      childRects.push({
        x: i * (childWidth + gap),
        width: childWidth,
      });
    }
  }

  return (
    <svg viewBox="0 0 300 16" preserveAspectRatio="none" aria-label={`Session waterfall: ${status}`}>
      {/* Root span */}
      <rect
        x={0}
        y={0}
        width={rootWidth}
        height={7}
        rx={1}
        fill="var(--primary)"
        opacity={rootOpacity}
      />
      {/* Child session spans */}
      {childRects.map((child, i) => (
        <rect
          key={i}
          x={child.x}
          y={10}
          width={child.width}
          height={6}
          rx={1}
          fill="var(--success)"
          opacity={0.8}
        />
      ))}
    </svg>
  );
}
