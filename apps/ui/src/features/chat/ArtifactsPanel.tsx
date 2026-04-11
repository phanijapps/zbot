import { useState, useEffect } from "react";
import {
  FileText, FileCode, Table, Globe, Image, Film, Music,
  File, Download, ChevronDown, ChevronRight, Paperclip, Presentation
} from "lucide-react";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";

interface ArtifactsPanelProps {
  sessionId: string;
}

function getArtifactIcon(fileType?: string) {
  const size = 14;
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

function formatFileSize(bytes?: number): string {
  if (!bytes) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ArtifactsPanel({ sessionId }: ArtifactsPanelProps) {
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [expanded, setExpanded] = useState(true);
  const [viewingId, setViewingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      const transport = await getTransport();
      const result = await transport.listSessionArtifacts(sessionId);
      if (!cancelled && result.success && result.data) {
        setArtifacts(result.data);
      }
    }
    load();
    return () => { cancelled = true; };
  }, [sessionId]);

  if (artifacts.length === 0) return null;

  return (
    <div className="artifacts-panel">
      <div className="artifacts-panel__header" onClick={() => setExpanded(!expanded)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") setExpanded(!expanded); }}>
        <Paperclip size={14} />
        <span>{artifacts.length} artifact{artifacts.length !== 1 ? "s" : ""}</span>
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </div>
      {expanded && (
        <div className="artifacts-panel__list">
          {artifacts.map((art) => (
            <ArtifactRow
              key={art.id}
              artifact={art}
              isViewing={viewingId === art.id}
              onToggleView={() => setViewingId(viewingId === art.id ? null : art.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ArtifactRow({ artifact, isViewing, onToggleView }: {
  artifact: Artifact;
  isViewing: boolean;
  onToggleView: () => void;
}) {
  return (
    <div>
      <div className="artifacts-panel__row" onClick={onToggleView} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggleView(); }}>
        <span className="artifacts-panel__icon">{getArtifactIcon(artifact.fileType)}</span>
        <span className="artifacts-panel__label">{artifact.label || artifact.fileName}</span>
        <span className="artifacts-panel__path">{artifact.fileName}</span>
        <span className="artifacts-panel__size">{formatFileSize(artifact.fileSize)}</span>
      </div>
      {isViewing && <ArtifactViewer artifact={artifact} />}
    </div>
  );
}

function ArtifactViewer({ artifact }: { artifact: Artifact }) {
  const [content, setContent] = useState<string | null>(null);
  const [contentUrl, setContentUrl] = useState<string>("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      const transport = await getTransport();
      const url = transport.getArtifactContentUrl(artifact.id);
      setContentUrl(url);

      const isText = ["md", "txt", "html", "htm", "csv", "json",
        "rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml",
        "xml", "sql", "sh", "bash", "css"].includes(artifact.fileType || "");

      if (isText) {
        try {
          const resp = await fetch(url);
          if (resp.ok && !cancelled) {
            setContent(await resp.text());
          }
        } catch (e) {
          console.error("Failed to load artifact:", e);
        }
      }

      if (!cancelled) setLoading(false);
    }
    load();
    return () => { cancelled = true; };
  }, [artifact.id, artifact.fileType]);

  if (loading) {
    return <div className="artifacts-panel__viewer"><span className="loading-spinner" /></div>;
  }

  const ft = artifact.fileType || "";

  if (ft === "md" || ft === "txt") {
    return <div className="artifacts-panel__viewer"><pre>{content}</pre></div>;
  }
  if (ft === "html" || ft === "htm") {
    return <div className="artifacts-panel__viewer"><iframe srcDoc={content || ""} style={{ width: "100%", height: 400, border: "none" }} sandbox="allow-scripts" title="Artifact preview" /></div>;
  }
  if (ft === "csv") {
    return <div className="artifacts-panel__viewer"><CsvTable content={content || ""} /></div>;
  }
  if (ft === "json") {
    return <div className="artifacts-panel__viewer"><pre>{formatJson(content || "")}</pre></div>;
  }
  if (["rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml", "xml", "sql", "sh", "css"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><pre><code>{content}</code></pre></div>;
  }
  if (["png", "jpg", "jpeg", "gif", "svg"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><img src={contentUrl} alt={artifact.fileName} style={{ maxWidth: "100%", maxHeight: 500 }} /></div>;
  }
  if (["mp4", "webm"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><video src={contentUrl} controls style={{ maxWidth: "100%" }} /></div>;
  }
  if (["mp3", "wav"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><audio src={contentUrl} controls /></div>;
  }
  if (ft === "pdf") {
    return <div className="artifacts-panel__viewer"><embed src={contentUrl} type="application/pdf" width="100%" height="500px" /></div>;
  }
  return (
    <div className="artifacts-panel__viewer">
      <a href={contentUrl} download={artifact.fileName} className="btn btn--outline btn--sm">
        <Download size={14} /> Download {artifact.fileName}
      </a>
    </div>
  );
}

function CsvTable({ content }: { content: string }) {
  const rows = content.split("\n").filter(Boolean).map((row) => row.split(","));
  if (rows.length === 0) return <pre>{content}</pre>;
  const header = rows[0];
  const body = rows.slice(1, 51);
  return (
    <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
      <thead>
        <tr>{header.map((h, i) => <th key={i} style={{ textAlign: "left", padding: "4px 8px", borderBottom: "1px solid var(--border)", color: "var(--foreground)" }}>{h.trim()}</th>)}</tr>
      </thead>
      <tbody>
        {body.map((row, i) => (
          <tr key={i}>{row.map((cell, j) => <td key={j} style={{ padding: "4px 8px", borderBottom: "1px solid var(--border)", color: "var(--muted-foreground)" }}>{cell.trim()}</td>)}</tr>
        ))}
      </tbody>
    </table>
  );
}

function formatJson(content: string): string {
  try { return JSON.stringify(JSON.parse(content), null, 2); }
  catch { return content; }
}
