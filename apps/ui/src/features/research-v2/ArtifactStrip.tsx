// =============================================================================
// ArtifactStrip — live horizontal chip row (R14d).
//
// Renders a scrollable strip of artifact chips above the composer. Polling in
// useResearchSession keeps state.artifacts fresh; clicking a chip asks the
// parent to open ArtifactSlideOut for that artifact.
// =============================================================================

import { getArtifactIcon } from "../chat/artifact-utils";
import type { ResearchArtifactRef } from "./types";

interface ArtifactStripProps {
  artifacts: ResearchArtifactRef[];
  onOpen(artifact: ResearchArtifactRef): void;
}

export function ArtifactStrip({ artifacts, onOpen }: ArtifactStripProps) {
  if (artifacts.length === 0) return null;

  return (
    <ul className="research-artifacts" aria-label="Session artifacts">
      {artifacts.map((artifact) => (
        <li key={artifact.id} className="research-artifacts__item">
          <button
            type="button"
            className="research-artifact-chip"
            onClick={() => onOpen(artifact)}
            aria-label={`Open artifact ${artifact.fileName}`}
            title={artifact.fileName}
          >
            <span className="research-artifact-chip__icon" aria-hidden="true">
              {getArtifactIcon(artifact.fileType, 12)}
            </span>
            <span className="research-artifact-chip__name">{artifact.fileName}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
