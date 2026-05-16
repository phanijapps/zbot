// ============================================================================
// PropagationChainVis — placeholder linear visualization of one cascade
// ============================================================================
//
// B-3 effectively keeps `max_propagation_depth = 1` (fact → belief). This
// component renders a small SVG "fact → belief" arrow only when a recent
// propagation event actually fired. Higher-order cascades land in a
// future phase; the layout is intentionally simple so it can grow.

import type { BeliefPropagationStats } from "../types.beliefNetwork";

const NODE_W = 80;
const NODE_H = 24;
const HSPACE = 36;
const SVG_H = NODE_H + 16;

export interface PropagationChainVisProps {
  latest: BeliefPropagationStats;
}

export function PropagationChainVis(props: PropagationChainVisProps) {
  const { latest } = props;

  if (latest.beliefs_invalidated === 0) {
    return (
      <div
        className="propagation-chain propagation-chain--idle"
        data-testid="propagation-chain-idle"
      >
        <p>No propagation events yet.</p>
      </div>
    );
  }

  const totalDepth = Math.max(1, latest.max_propagation_depth);
  const nodes = totalDepth + 1; // +1 source fact node
  const svgWidth = nodes * NODE_W + (nodes - 1) * HSPACE;

  return (
    <div className="propagation-chain" data-testid="propagation-chain">
      <svg
        width={svgWidth}
        height={SVG_H}
        role="img"
        aria-label={`Propagation chain of depth ${totalDepth}`}
      >
        {Array.from({ length: nodes }).map((_, i) => {
          const x = i * (NODE_W + HSPACE);
          const isSource = i === 0;
          const label = isSource ? "fact" : `belief ${i}`;
          return (
            <g key={i} data-testid="propagation-chain-node">
              <rect
                x={x}
                y={8}
                width={NODE_W}
                height={NODE_H}
                rx={4}
                fill={isSource ? "var(--muted, #888)" : "var(--primary, #4af)"}
              />
              <text
                x={x + NODE_W / 2}
                y={8 + NODE_H / 2 + 4}
                textAnchor="middle"
                fontSize="11"
                fill="var(--primary-foreground, #fff)"
              >
                {label}
              </text>
              {i < nodes - 1 && (
                <line
                  x1={x + NODE_W}
                  y1={8 + NODE_H / 2}
                  x2={x + NODE_W + HSPACE}
                  y2={8 + NODE_H / 2}
                  stroke="currentColor"
                  strokeWidth={1.5}
                  markerEnd="url(#chain-arrow)"
                />
              )}
            </g>
          );
        })}
        <defs>
          <marker
            id="chain-arrow"
            markerWidth="6"
            markerHeight="6"
            refX="6"
            refY="3"
            orient="auto"
          >
            <path d="M0,0 L6,3 L0,6 Z" fill="currentColor" />
          </marker>
        </defs>
      </svg>
      <p className="propagation-chain__caption">
        depth {totalDepth} · {latest.beliefs_invalidated} invalidated
      </p>
    </div>
  );
}
