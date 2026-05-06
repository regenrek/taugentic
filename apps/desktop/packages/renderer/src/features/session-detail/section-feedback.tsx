export interface SectionFeedbackProps {
  errorMessage: string | null;
  hasLoaded: boolean;
  isEmpty: boolean;
  itemsLabel: string;
}

export function SectionFeedback({
  errorMessage,
  hasLoaded,
  isEmpty,
  itemsLabel,
}: SectionFeedbackProps) {
  // Initial error: first load failed, nothing to show.
  if (!hasLoaded && errorMessage !== null) {
    return (
      <div
        className="border border-[var(--status-failed)]/40 bg-[var(--bg-raised)] px-2 py-1.5 font-[var(--font-mono)] text-[12px] text-[var(--status-failed)]"
        data-state="error"
      >
        error: {errorMessage}
      </div>
    );
  }
  // First load still in flight: explicit loading row.
  if (!hasLoaded) {
    return (
      <div
        className="font-[var(--font-mono)] text-[12px] text-[var(--fg-dim)]"
        data-state="loading"
      >
        loading {itemsLabel}…
      </div>
    );
  }
  // Loaded with stale error: keep existing data visible above; surface the error inline.
  if (errorMessage !== null) {
    return (
      <div
        className="border border-[var(--status-failed)]/40 bg-[var(--bg-raised)] px-2 py-1.5 font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]"
        data-state="stale"
      >
        stale · {errorMessage}
      </div>
    );
  }
  // Loaded successfully, no items, no error.
  if (isEmpty) {
    return (
      <div className="font-[var(--font-mono)] text-[12px] text-[var(--fg-dim)]" data-state="empty">
        no {itemsLabel} yet
      </div>
    );
  }
  return null;
}
