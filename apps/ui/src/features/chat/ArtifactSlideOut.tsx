// ============================================================================
// ARTIFACT SLIDE-OUT VIEWER
// Full-height panel sliding in from the right to preview artifact content.
// ============================================================================

import { useEffect, useState } from "react";
import {
  X, Download, FileText, FileCode, Table, Globe, Image, Film, Music,
  File, Presentation
} from "lucide-react";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";

interface ArtifactSlideOutProps {
  artifact: Artifact;
  onClose: () => void;
}

export function ArtifactSlideOut({ artifact, onClose }: ArtifactSlideOutProps) {
  const [content, setContent] = useState<string | null>(null);
  const [contentUrl, setContentUrl] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      const transport = await getTransport();
      const url = transport.getArtifactContentUrl(artifact.id);
      setContentUrl(url);

      const textTypes = ["md", "txt", "html", "htm", "csv", "json",
        "rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml",
        "xml", "sql", "sh", "bash", "css", "go", "java", "c", "cpp", "h"];

      if (textTypes.includes(artifact.fileType || "")) {
        try {
          const resp = await fetch(url);
          if (resp.ok && !cancelled) setContent(await resp.text());
        } catch (e) {
          console.error("Failed to load artifact:", e);
        }
      }
      if (!cancelled) setLoading(false);
    }
    load();
    return () => { cancelled = true; };
  }, [artifact.id, artifact.fileType]);

  // Close on Escape
  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  return (
    <>
      <div className="artifact-slideout__backdrop" onClick={onClose} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onClose(); }} />
      <div className="artifact-slideout">
        <div className="artifact-slideout__header">
          <div className="artifact-slideout__title">
            <span className="artifact-slideout__icon">{getIcon(artifact.fileType)}</span>
            <span>{artifact.label || artifact.fileName}</span>
            <span className="artifact-slideout__meta">{artifact.fileName} · {formatSize(artifact.fileSize)}</span>
          </div>
          <div className="artifact-slideout__actions">
            <a href={contentUrl} download={artifact.fileName} className="btn btn--ghost btn--sm" title="Download">
              <Download size={14} />
            </a>
            <button className="btn btn--ghost btn--sm" onClick={onClose} title="Close">
              <X size={14} />
            </button>
          </div>
        </div>
        <div className="artifact-slideout__body">
          {loading ? (
            <div style={{ display: "flex", justifyContent: "center", padding: "40px" }}>
              <span className="loading-spinner" />
            </div>
          ) : (
            renderContent(artifact, content, contentUrl)
          )}
        </div>
      </div>
    </>
  );
}

function renderContent(artifact: Artifact, content: string | null, contentUrl: string) {
  const ft = artifact.fileType || "";

  if (ft === "md" || ft === "txt") return <pre className="artifact-slideout__pre">{content}</pre>;
  if (ft === "html" || ft === "htm") return <iframe srcDoc={content || ""} style={{ width: "100%", height: "100%", border: "none" }} sandbox="allow-scripts" title="Artifact preview" />;
  if (ft === "csv") return <CsvTable content={content || ""} />;
  if (ft === "json") return <pre className="artifact-slideout__pre">{formatJson(content || "")}</pre>;
  if (["rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml", "xml", "sql", "sh", "css", "go", "java", "c", "cpp", "h"].includes(ft)) {
    return <pre className="artifact-slideout__pre"><code>{content}</code></pre>;
  }
  if (["png", "jpg", "jpeg", "gif", "svg"].includes(ft)) {
    return <img src={contentUrl} alt={artifact.fileName} style={{ maxWidth: "100%", maxHeight: "80vh", objectFit: "contain" }} />;
  }
  if (["mp4", "webm"].includes(ft)) return <video src={contentUrl} controls style={{ maxWidth: "100%" }}><track kind="captions" /></video>;
  if (["mp3", "wav"].includes(ft)) return <audio src={contentUrl} controls style={{ width: "100%" }}><track kind="captions" /></audio>;
  if (ft === "pdf") return <embed src={contentUrl} type="application/pdf" width="100%" height="100%" />;
  return (
    <div style={{ textAlign: "center", padding: "40px" }}>
      <p style={{ marginBottom: "16px", color: "var(--muted-foreground)" }}>Preview not available for .{ft} files</p>
      <a href={contentUrl} download={artifact.fileName} className="btn btn--outline btn--sm">
        <Download size={14} /> Download {artifact.fileName}
      </a>
    </div>
  );
}

function getIcon(fileType?: string) {
  const size = 16;
  switch (fileType) {
    case "md": case "txt": case "docx": return <FileText size={size} />;
    case "rs": case "py": case "js": case "ts": case "tsx": case "jsx": return <FileCode size={size} />;
    case "csv": case "json": case "xlsx": return <Table size={size} />;
    case "html": case "htm": return <Globe size={size} />;
    case "png": case "jpg": case "jpeg": case "gif": case "svg": return <Image size={size} />;
    case "mp4": case "webm": return <Film size={size} />;
    case "mp3": case "wav": return <Music size={size} />;
    case "pptx": return <Presentation size={size} />;
    default: return <File size={size} />;
  }
}

function formatSize(bytes?: number): string {
  if (!bytes) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function CsvTable({ content }: { content: string }) {
  const rows = content.split("\n").filter(Boolean).map(r => r.split(","));
  if (rows.length === 0) return <pre>{content}</pre>;
  return (
    <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
      <thead>
        <tr>{rows[0].map((h, i) => <th key={i} style={{ textAlign: "left", padding: "6px 10px", borderBottom: "2px solid var(--border)", color: "var(--foreground)", position: "sticky", top: 0, background: "var(--card)" }}>{h.trim()}</th>)}</tr>
      </thead>
      <tbody>
        {rows.slice(1, 100).map((row, i) => (
          <tr key={i}>{row.map((cell, j) => <td key={j} style={{ padding: "4px 10px", borderBottom: "1px solid var(--border)", color: "var(--muted-foreground)" }}>{cell.trim()}</td>)}</tr>
        ))}
      </tbody>
    </table>
  );
}

function formatJson(content: string): string {
  try { return JSON.stringify(JSON.parse(content), null, 2); }
  catch { return content; }
}
