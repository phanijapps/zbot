interface InspectorNoticeProps {
  loading: boolean;
  hasData: boolean;
  error: boolean;
  loadingLabel: string;
  unavailable: string;
  retry(): void;
}

/**
 * Shared loading/unavailable/retry notice for the session inspector tabs.
 * One copy so Activity, Sources and Files cannot drift; the error keeps
 * previously loaded rows visible (each panel renders them separately).
 */
export function InspectorNotice({loading, hasData, error, loadingLabel, unavailable, retry}: InspectorNoticeProps) {
  return <div role="status" aria-atomic="true">
    {loading && !hasData && <p className="session-shell__hint">{loadingLabel}</p>}
    {error && <div className="session-inspector__notice">
      <p>{unavailable}</p>
      <button type="button" className="btn btn--outline btn--sm" onClick={retry}>Retry</button>
    </div>}
  </div>;
}
