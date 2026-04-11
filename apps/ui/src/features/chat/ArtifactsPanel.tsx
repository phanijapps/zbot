import { useState, useEffect } from "react";
import { Download, ChevronDown, ChevronRight, Paperclip } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";
import { getArtifactIcon, formatFileSize, formatJson, CsvTable } from "./artifact-utils";

interface ArtifactsPanelProps {
  sessionId: string;
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
    return <div className="artifacts-panel__viewer"><video src={contentUrl} controls style={{ maxWidth: "100%" }}><track kind="captions" /></video></div>;
  }
  if (["mp3", "wav"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><audio src={contentUrl} controls><track kind="captions" /></audio></div>;
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

