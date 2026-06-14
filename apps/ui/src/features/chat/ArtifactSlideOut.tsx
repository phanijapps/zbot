// ============================================================================
// ARTIFACT SLIDE-OUT VIEWER
// Full-height panel sliding in from the right to preview artifact content.
// ============================================================================

import { useEffect, useState } from "react";
import { X, Download } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";
import { getArtifactIcon, formatFileSize, formatJson, CsvTable } from "./artifact-utils";
import { Markdown } from "../shared/markdown";
import { parseOfficePreview, type OfficePreview } from "./officePreview";

interface ArtifactSlideOutProps {
  artifact: Artifact;
  onClose: () => void;
}

export function ArtifactSlideOut({ artifact, onClose }: ArtifactSlideOutProps) {
  const [content, setContent] = useState<string | null>(null);
  const [officePreview, setOfficePreview] = useState<OfficePreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [contentUrl, setContentUrl] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      setContent(null);
      setOfficePreview(null);
      setPreviewError(null);
      const transport = await getTransport();
      const url = transport.getArtifactContentUrl(artifact.id);
      setContentUrl(url);

      const fileType = artifact.fileType || "";
      const textTypes = ["md", "txt", "html", "htm", "csv", "json",
        "rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml",
        "xml", "sql", "sh", "bash", "css", "go", "java", "c", "cpp", "h"];
      const officeTypes = ["docx", "xlsx", "pptx"];

      if (textTypes.includes(fileType)) {
        try {
          const resp = await fetch(url);
          if (resp.ok && !cancelled) setContent(await resp.text());
        } catch (e) {
          console.error("Failed to load artifact:", e);
        }
      }

      if (officeTypes.includes(fileType)) {
        try {
          const resp = await fetch(url);
          if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
          const preview = await parseOfficePreview(await resp.arrayBuffer(), fileType as "docx" | "xlsx" | "pptx");
          if (!cancelled) setOfficePreview(preview);
        } catch (e) {
          console.error("Failed to preview artifact:", e);
          if (!cancelled) setPreviewError(e instanceof Error ? e.message : "Unable to preview this file");
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
            <span className="artifact-slideout__icon">{getArtifactIcon(artifact.fileType, 16)}</span>
            <span>{artifact.label || artifact.fileName}</span>
            <span className="artifact-slideout__meta">{artifact.fileName} · {formatFileSize(artifact.fileSize)}</span>
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
            renderContent(artifact, content, contentUrl, officePreview, previewError)
          )}
        </div>
      </div>
    </>
  );
}

function renderContent(
  artifact: Artifact,
  content: string | null,
  contentUrl: string,
  officePreview: OfficePreview | null,
  previewError: string | null,
) {
  const ft = artifact.fileType || "";

  if (ft === "md") return <Markdown className="artifact-slideout__md">{content ?? ""}</Markdown>;
  if (ft === "txt") return <pre className="artifact-slideout__pre">{content}</pre>;
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
  if (["docx", "xlsx", "pptx"].includes(ft)) {
    if (officePreview) return <OfficePreviewView preview={officePreview} />;
    return <PreviewUnavailable fileType={ft} fileName={artifact.fileName} contentUrl={contentUrl} error={previewError} />;
  }
  return <PreviewUnavailable fileType={ft} fileName={artifact.fileName} contentUrl={contentUrl} />;
}

function OfficePreviewView({ preview }: { preview: OfficePreview }) {
  if (preview.kind === "docx") {
    return (
      <article className="artifact-office artifact-office--document">
        {preview.blocks.map((block, index) => {
          if (block.type === "table") return <PreviewTable key={index} rows={block.rows} />;
          const className = block.style?.toLowerCase().startsWith("heading")
            ? "artifact-office__heading"
            : block.list
              ? "artifact-office__list-item"
              : "artifact-office__paragraph";
          return <p key={index} className={className}>{block.text}</p>;
        })}
      </article>
    );
  }

  if (preview.kind === "xlsx") {
    return (
      <div className="artifact-office artifact-office--workbook">
        {preview.sheets.map((sheet) => (
          <section key={sheet.name} className="artifact-office__sheet">
            <h3>{sheet.name}</h3>
            <PreviewTable rows={sheet.rows} />
          </section>
        ))}
      </div>
    );
  }

  return (
    <div className="artifact-office artifact-office--presentation">
      {preview.slides.map((slide) => (
        <section key={slide.number} className="artifact-office__slide">
          <div className="artifact-office__slide-number">Slide {slide.number}</div>
          <h3>{slide.title}</h3>
          {slide.lines.slice(1).map((line, index) => <p key={index}>{line}</p>)}
        </section>
      ))}
    </div>
  );
}

function PreviewTable({ rows }: { rows: string[][] }) {
  if (rows.length === 0) return <p className="settings-hint">No previewable rows found.</p>;
  return (
    <div className="artifact-office__table-wrap">
      <table className="artifact-office__table">
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {row.map((cell, cellIndex) => (
                rowIndex === 0
                  ? <th key={cellIndex}>{cell}</th>
                  : <td key={cellIndex}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function PreviewUnavailable({
  fileType,
  fileName,
  contentUrl,
  error,
}: {
  fileType: string;
  fileName: string;
  contentUrl: string;
  error?: string | null;
}) {
  return (
    <div className="artifact-slideout__empty">
      <p>{error ? `Preview failed: ${error}` : `Preview not available for .${fileType} files`}</p>
      <a href={contentUrl} download={fileName} className="btn btn--outline btn--sm">
        <Download size={14} /> Download {fileName}
      </a>
    </div>
  );
}
