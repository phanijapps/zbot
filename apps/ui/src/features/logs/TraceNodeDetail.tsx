import type { TraceNode } from "./trace-types";

interface TraceNodeDetailProps {
  node: TraceNode;
}

export function TraceNodeDetail({ node }: TraceNodeDetailProps) {
  return (
    <div className="trace-node-detail">
      {node.args && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Arguments</div>
          <div className="trace-node-detail__content">{formatArgs(node.args)}</div>
        </div>
      )}
      {node.result && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Result</div>
          <div className="trace-node-detail__content">{truncateResult(node.result)}</div>
        </div>
      )}
      {node.error && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Error</div>
          <div className="trace-node-detail__content" style={{ color: "var(--destructive)" }}>
            {node.error}
          </div>
        </div>
      )}
      {node.type === "delegation" && node.summary && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Task</div>
          <div className="trace-node-detail__content">{node.summary}</div>
        </div>
      )}
    </div>
  );
}

function formatArgs(args: string): string {
  try {
    const parsed = JSON.parse(args);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return args;
  }
}

function truncateResult(result: string): string {
  if (result.length <= 2000) return result;
  return result.slice(0, 1000) + "\n\n--- truncated ---\n\n" + result.slice(-500);
}
