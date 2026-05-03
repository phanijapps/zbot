import { useEffect, useState } from "react";
import { ConflictBanner } from "./ConflictBanner";

type Props = {
  path: string;
};

type Conflict = { currentContent: string; currentVersion: string };

export function FileEditor({ path }: Props) {
  const [diskContent, setDiskContent] = useState("");
  const [editorContent, setEditorContent] = useState("");
  const [version, setVersion] = useState<string>("");
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch(`/api/customization/file?path=${encodeURIComponent(path)}`);
        const body = await res.json();
        if (!cancelled && body.success) {
          setDiskContent(body.content ?? "");
          setEditorContent(body.content ?? "");
          setVersion(body.version ?? "");
          setError(null);
          setConflict(null);
        } else if (!cancelled) {
          setError(body.error ?? "Failed to load file");
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [path]);

  const isDirty = editorContent !== diskContent;

  const onSave = async () => {
    setIsSaving(true);
    try {
      const res = await fetch("/api/customization/file", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          path,
          content: editorContent,
          expectedVersion: version,
        }),
      });
      const body = await res.json();
      if (res.status === 409) {
        setConflict({
          currentContent: body.currentContent ?? "",
          currentVersion: body.currentVersion ?? "",
        });
      } else if (body.success) {
        setDiskContent(editorContent);
        setVersion(body.version ?? version);
      } else {
        setError(body.error ?? "Save failed");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  };

  const onDiscard = () => {
    setEditorContent(diskContent);
  };

  const onAcceptDisk = () => {
    if (!conflict) return;
    setDiskContent(conflict.currentContent);
    setEditorContent(conflict.currentContent);
    setVersion(conflict.currentVersion);
    setConflict(null);
  };

  const onKeepEditing = () => {
    if (!conflict) return;
    setVersion(conflict.currentVersion);
    setConflict(null);
  };

  return (
    <div className="customization-editor">
      <header className="row" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
        <h4 style={{ margin: 0 }}>
          <code>{path}</code>
        </h4>
        {isDirty && <span className="muted small">• Modified</span>}
      </header>
      {conflict && (
        <ConflictBanner onAcceptDisk={onAcceptDisk} onKeepEditing={onKeepEditing} />
      )}
      {error && (
        <div className="warning" role="status">
          {error}
        </div>
      )}
      <textarea
        value={editorContent}
        onChange={(e) => setEditorContent(e.target.value)}
        style={{
          width: "100%",
          minHeight: 400,
          fontFamily: "monospace",
          fontSize: "var(--font-size-sm)",
          padding: "var(--spacing-3)",
        }}
        spellCheck={false}
      />
      <div className="row" style={{ display: "flex", gap: "var(--spacing-2)", marginTop: "var(--spacing-2)" }}>
        <button
          type="button"
          className="btn btn--outline btn--sm"
          onClick={onDiscard}
          disabled={!isDirty || isSaving}
        >
          Discard
        </button>
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={onSave}
          disabled={!isDirty || isSaving}
        >
          {isSaving ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}
