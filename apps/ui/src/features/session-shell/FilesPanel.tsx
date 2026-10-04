import { useEffect, useState } from "react";
import { getArtifactIcon, formatFileSize } from "../chat/artifact-utils";
import { ArtifactSlideOut } from "../chat/ArtifactSlideOut";
import { InspectorNotice } from "./InspectorNotice";
import { useSessionArtifacts } from "./useSessionArtifacts";

/**
 * Server-backed Files inspector: the session's artifact manifest, opened
 * through the existing ID-based content endpoint (confinement enforced
 * server-side). Reuses the established artifact viewer for content.
 */
export function FilesPanel({ sessionId, active }: {sessionId?: string; active: boolean}) {
  const {artifacts, loading, error, retry} = useSessionArtifacts(sessionId, active);
  const [openId, setOpenId] = useState<string | null>(null);
  // Never keep another conversation's artifact open under a new selection.
  useEffect(() => { setOpenId(null); }, [sessionId]);
  const open = artifacts?.find(artifact => artifact.id === openId) ?? null;

  if (!sessionId) return <p className="session-shell__hint">Select or start a conversation to inspect its files.</p>;
  return <section className="session-files" aria-label="Session files">
    <InspectorNotice loading={loading} hasData={Boolean(artifacts)} error={error}
      loadingLabel="Loading files…" unavailable="Files are unavailable. Your conversation has been kept." retry={retry} />
    {artifacts && artifacts.length === 0 && <p className="session-shell__hint">No files recorded for this conversation yet.</p>}
    <ul className="session-files__list" aria-busy={loading}>
      {artifacts?.map(artifact => <li key={artifact.id} className="session-files__item">
        <button type="button" className="session-files__open" onClick={() => setOpenId(artifact.id)}
          aria-label={`Open artifact ${artifact.fileName}`}>
          <span className="session-files__icon" aria-hidden="true">{getArtifactIcon(artifact.fileType, 14)}</span>
          <span className="session-files__name">{artifact.fileName}</span>
          {artifact.fileSize !== undefined && <span className="session-files__size">{formatFileSize(artifact.fileSize)}</span>}
        </button>
      </li>)}
    </ul>
    {open && <ArtifactSlideOut artifact={open} onClose={() => setOpenId(null)} />}
  </section>;
}
