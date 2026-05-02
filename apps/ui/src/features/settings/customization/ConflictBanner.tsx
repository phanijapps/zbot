type Props = {
  onAcceptDisk: () => void;
  onKeepEditing: () => void;
};

export function ConflictBanner({ onAcceptDisk, onKeepEditing }: Props) {
  return (
    <div className="warning" role="status" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
      <span>This file changed on disk while you were editing.</span>
      <button type="button" className="btn btn--outline btn--sm" onClick={onAcceptDisk}>
        View disk version
      </button>
      <button type="button" className="btn btn--outline btn--sm" onClick={onKeepEditing}>
        Keep editing
      </button>
    </div>
  );
}
