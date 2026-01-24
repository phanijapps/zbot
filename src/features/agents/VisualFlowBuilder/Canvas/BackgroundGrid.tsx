// ============================================================================
// VISUAL FLOW BUILDER - BACKGROUND GRID
// Renders a dotted grid background for the canvas
// ============================================================================

import { memo } from "react";
import { CANVAS_CONFIG } from "../constants";

interface BackgroundGridProps {
  x: number;
  y: number;
  zoom: number;
  width: number;
  height: number;
  dotSize?: number;
  dotSpacing?: number;
  dotColor?: string;
}

export const BackgroundGrid = memo(({
  x,
  y,
  zoom,
  width,
  height,
  dotSize = 1,
  dotSpacing = CANVAS_CONFIG.GRID_SIZE,
  dotColor = "rgba(255, 255, 255, 0.1)",
}: BackgroundGridProps) => {
  // Calculate effective dot spacing and size based on zoom
  const effectiveSpacing = dotSpacing * zoom;
  const effectiveDotSize = dotSize * zoom;

  // Only show dots if they're not too dense
  if (effectiveSpacing < 8) {
    return null;
  }

  // Calculate the range of dots to render
  const startX = Math.floor(-x / zoom / dotSpacing) * dotSpacing - dotSpacing;
  const startY = Math.floor(-y / zoom / dotSpacing) * dotSpacing - dotSpacing;
  const endX = startX + (width / zoom) + dotSpacing * 2;
  const endY = startY + (height / zoom) + dotSpacing * 2;

  // Generate dot positions
  const dots: Array<{ cx: number; cy: number }> = [];
  for (let gridX = startX; gridX <= endX; gridX += dotSpacing) {
    for (let gridY = startY; gridY <= endY; gridY += dotSpacing) {
      dots.push({ cx: gridX, cy: gridY });
    }
  }

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      style={{ transform: `translate(${x}px, ${y}px) scale(${zoom})` }}
      width={width / zoom}
      height={height / zoom}
    >
      {dots.map((dot, i) => (
        <circle
          key={`${dot.cx}-${dot.cy}-${i}`}
          cx={dot.cx}
          cy={dot.cy}
          r={effectiveDotSize / zoom}
          fill={dotColor}
        />
      ))}
    </svg>
  );
});

BackgroundGrid.displayName = "BackgroundGrid";

// -----------------------------------------------------------------------------
// Grid Pattern Variant (more efficient for large canvases)
// -----------------------------------------------------------------------------

interface GridPatternProps {
  x: number;
  y: number;
  zoom: number;
  width: number;
  height: number;
  spacing?: number;
  strokeColor?: string;
  strokeWidth?: number;
}

export const GridPattern = memo(({
  x,
  y,
  zoom,
  width,
  height,
  spacing = CANVAS_CONFIG.GRID_SIZE,
  strokeColor = "rgba(255, 255, 255, 0.05)",
  strokeWidth = 1,
}: GridPatternProps) => {
  const effectiveSpacing = spacing * zoom;

  // Use solid lines for very zoomed out views
  if (effectiveSpacing < 8) {
    return null;
  }

  const patternId = `grid-pattern-${effectiveSpacing}`;

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      width={width}
      height={height}
    >
      <defs>
        <pattern
          id={patternId}
          x={x % effectiveSpacing}
          y={y % effectiveSpacing}
          width={effectiveSpacing}
          height={effectiveSpacing}
          patternUnits="userSpaceOnUse"
        >
          <path
            d={`M ${effectiveSpacing} 0 L 0 0 0 ${effectiveSpacing}`}
            fill="none"
            stroke={strokeColor}
            strokeWidth={strokeWidth * zoom}
          />
        </pattern>
      </defs>
      <rect
        width="100%"
        height="100%"
        fill={`url(#${patternId})`}
      />
    </svg>
  );
});

GridPattern.displayName = "GridPattern";
