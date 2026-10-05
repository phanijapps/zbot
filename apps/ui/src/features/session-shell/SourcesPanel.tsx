import { useSessionDetails } from "./useSessionDetails";
import { InspectorNotice } from "./InspectorNotice";
import type { SessionDetails } from "@/services/transport/types";

const evidence: Record<SessionDetails["sources"][number]["evidence"], string> = {
  answer_citation: "Cited in answer",
  structured_source_use: "Reported source",
};

const safeDestination = (url: string) => /^https?:\/\//i.test(url) ? url : undefined;

/**
 * Server-backed Sources inspector: only records the details contract resolved
 * as cited by the answer or emitted as a structured source use. Unsafe
 * destinations are never rendered as links (defense in depth; the server
 * already rejects them).
 */
export function SourcesPanel({ sessionId, active }: {sessionId?: string; active: boolean}) {
  const {details, loading, error, retry} = useSessionDetails(sessionId, active);

  if (!sessionId) return <p className="session-shell__hint">Select or start a conversation to inspect its sources.</p>;
  return <section className="session-sources" aria-label="Recorded sources">
    <InspectorNotice loading={loading} hasData={Boolean(details)} error={error} loadingLabel="Loading sources…"
      unavailable="Sources are unavailable. Your conversation and previously loaded sources have been kept." retry={retry} />
    {details?.sourcesTruncated && <p className="session-inspector__notice">First 100 sources in citation/use order shown. Later sources are not included.</p>}
    {details && details.sources.length === 0 && <p className="session-shell__hint">No cited or used sources for this conversation yet.</p>}
    <ul className="session-sources__list" aria-busy={loading}>
      {details?.sources.map(source => {
        const destination = safeDestination(source.url);
        return <li key={source.id} className="session-sources__item">
          {destination
            ? <a className="session-sources__link" href={destination} target="_blank" rel="noreferrer noopener">{source.title}</a>
            : <span className="session-sources__link">{source.title}</span>}
          <span className="session-sources__evidence">{evidence[source.evidence]}</span>
        </li>;
      })}
    </ul>
  </section>;
}
