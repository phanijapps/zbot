// ============================================================================
// ARTIFACT UTILITIES
// Shared helpers used by ArtifactSlideOut, ArtifactsPanel, FastChat,
// and IntelligenceFeed.
// ============================================================================

import { FileText, FileCode, Table, Globe, Image, Film, Music, File, Presentation } from "lucide-react";

export function getArtifactIcon(fileType?: string, size = 14) {
  switch (fileType) {
    case "md": case "txt": case "docx": return <FileText size={size} />;
    case "rs": case "py": case "js": case "ts": case "tsx": case "jsx": return <FileCode size={size} />;
    case "csv": case "json": case "xlsx": return <Table size={size} />;
    case "html": case "htm": return <Globe size={size} />;
    case "png": case "jpg": case "jpeg": case "gif": case "svg": return <Image size={size} />;
    case "mp4": case "webm": return <Film size={size} />;
    case "mp3": case "wav": return <Music size={size} />;
    case "pptx": return <Presentation size={size} />;
    case "pdf": return <FileText size={size} />;
    default: return <File size={size} />;
  }
}

export function formatFileSize(bytes?: number): string {
  if (!bytes) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatJson(content: string): string {
  try { return JSON.stringify(JSON.parse(content), null, 2); }
  catch { return content; }
}

export function CsvTable({ content }: { content: string }) {
  const rows = content.split("\n").filter(Boolean).map((r) => r.split(","));
  if (rows.length === 0) return <pre>{content}</pre>;
  return (
    <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
      <thead>
        <tr>
          {rows[0].map((h, i) => (
            <th
              key={i}
              style={{
                textAlign: "left",
                padding: "6px 10px",
                borderBottom: "2px solid var(--border)",
                color: "var(--foreground)",
                position: "sticky",
                top: 0,
                background: "var(--card)",
              }}
            >
              {h.trim()}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.slice(1, 100).map((row, i) => (
          <tr key={i}>
            {row.map((cell, j) => (
              <td
                key={j}
                style={{
                  padding: "4px 10px",
                  borderBottom: "1px solid var(--border)",
                  color: "var(--muted-foreground)",
                }}
              >
                {cell.trim()}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
