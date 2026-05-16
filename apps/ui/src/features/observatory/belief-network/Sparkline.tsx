// ============================================================================
// Sparkline — inline SVG bar chart for cycle-over-time stats.
// ============================================================================
//
// Stays inline (no extra dep) and dimensionally tiny. Bars scale to the
// max value in `values`; an all-zero series renders a flat baseline so
// the panel doesn't collapse to zero height.

import type { CSSProperties } from "react";

const DEFAULT_WIDTH = 100;
const DEFAULT_HEIGHT = 24;
const BAR_GAP = 1;
const MIN_BAR_HEIGHT = 1;

export interface SparklineProps {
  values: number[];
  width?: number;
  height?: number;
  /** CSS colour for the bars — defaults to `currentColor` so the parent
   * can style via a wrapping `color` rule. */
  color?: string;
  ariaLabel?: string;
}

export function Sparkline(props: SparklineProps) {
  const {
    values,
    width = DEFAULT_WIDTH,
    height = DEFAULT_HEIGHT,
    color = "currentColor",
    ariaLabel,
  } = props;

  if (values.length === 0) {
    return (
      <svg
        width={width}
        height={height}
        role="img"
        aria-label={ariaLabel ?? "Empty sparkline"}
        data-testid="sparkline-empty"
      />
    );
  }

  const max = Math.max(...values, 1);
  const barWidth = Math.max(
    1,
    (width - BAR_GAP * (values.length - 1)) / values.length,
  );

  const baseStyle: CSSProperties = {
    display: "block",
  };

  return (
    <svg
      width={width}
      height={height}
      role="img"
      aria-label={ariaLabel ?? `Sparkline of ${values.length} values`}
      data-testid="sparkline"
      style={baseStyle}
    >
      {values.map((v, i) => {
        const ratio = max > 0 ? v / max : 0;
        const barHeight = Math.max(MIN_BAR_HEIGHT, ratio * height);
        const x = i * (barWidth + BAR_GAP);
        const y = height - barHeight;
        return (
          <rect
            key={i}
            data-testid="sparkline-bar"
            x={x}
            y={y}
            width={barWidth}
            height={barHeight}
            fill={color}
          />
        );
      })}
    </svg>
  );
}
