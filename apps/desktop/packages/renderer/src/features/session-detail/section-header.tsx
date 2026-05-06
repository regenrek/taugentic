import type { ReactNode } from "react";

export interface SectionHeaderProps {
  count?: number;
  errorMessage: string | null;
  hasLoaded: boolean;
  label: string;
  pending: boolean;
  /** Optional UI rendered to the right of the section label. */
  trailing?: ReactNode;
}

export function SectionHeader({
  count,
  errorMessage,
  hasLoaded,
  label,
  pending,
  trailing,
}: SectionHeaderProps) {
  const hint = deriveHint({ count, errorMessage, hasLoaded, pending });
  return (
    <div
      className="flex items-center gap-2 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]"
      data-section-header={label}
    >
      <span>{label}</span>
      {trailing !== undefined ? (
        <span className="ml-2 inline-flex items-center" data-section-trailing="">
          {trailing}
        </span>
      ) : null}
      {hint !== null ? (
        <span
          className="ml-auto"
          data-section-hint={hint.kind}
          style={hint.kind === "error" ? { color: "var(--status-failed)" } : undefined}
        >
          {hint.text}
        </span>
      ) : null}
    </div>
  );
}

type SectionHint = { kind: "loading" | "error" | "count"; text: string } | null;

function deriveHint({
  count,
  errorMessage,
  hasLoaded,
  pending,
}: {
  count?: number;
  errorMessage: string | null;
  hasLoaded: boolean;
  pending: boolean;
}): SectionHint {
  if (errorMessage !== null) {
    return { kind: "error", text: `error: ${errorMessage}` };
  }
  if (!hasLoaded && pending) {
    return { kind: "loading", text: "loading…" };
  }
  if (typeof count === "number") {
    return { kind: "count", text: String(count) };
  }
  return null;
}
