import { useState } from "react";

import type {
  ArtifactId,
  ArtifactSummary,
  SaveArtifactAsResult,
  SessionId,
} from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";

import { useSessionArtifactsQuery } from "@/lib/queries/session-queries";

import { ArtifactViewer } from "./ArtifactViewer";
import { describeArtifactMissingReason } from "./formatters";
import { SectionFeedback } from "./section-feedback";
import { SectionHeader } from "./section-header";
import { useArtifactContentQuery, useSaveArtifactAsMutation } from "./useArtifactContent";

export interface ArtifactsSectionProps {
  sessionId: SessionId;
}

export function ArtifactsSection({ sessionId }: ArtifactsSectionProps) {
  const query = useSessionArtifactsQuery(sessionId);
  const items = query.data ?? [];
  const hasLoaded = query.data !== undefined;
  const errorMessage = query.error ? toErrorMessage(query.error) : null;

  const [expandedArtifactId, setExpandedArtifactId] = useState<ArtifactId | null>(null);
  const [saveErrorMessage, setSaveErrorMessage] = useState<string | null>(null);

  const expandedArtifact = expandedArtifactId
    ? (items.find((artifact) => artifact.id === expandedArtifactId) ?? null)
    : null;

  const saveMutation = useSaveArtifactAsMutation(sessionId);
  const contentQuery = useArtifactContentQuery(sessionId, expandedArtifact);
  const contentError = contentQuery.error ? toErrorMessage(contentQuery.error) : null;

  const combinedError = errorMessage ?? saveErrorMessage;

  async function handleSave(artifact: ArtifactSummary): Promise<void> {
    setSaveErrorMessage(null);
    let result: SaveArtifactAsResult;
    try {
      result = await saveMutation.mutateAsync({ artifact });
    } catch (error) {
      setSaveErrorMessage(toErrorMessage(error));
      return;
    }
    // Invalidation of the artifact list + overview on `missing` is owned by
    // `useSaveArtifactAsMutation`; the component only drives the UX effect
    // (error copy).
    const effect = classifySaveArtifactResult(result);
    setSaveErrorMessage(effect.errorMessage);
  }

  function handleToggleExpand(artifact: ArtifactSummary): void {
    setExpandedArtifactId((current) => (current === artifact.id ? null : artifact.id));
  }

  return (
    <section className="flex flex-col gap-2 px-3 py-3" data-section="artifacts">
      <SectionHeader
        count={items.length}
        errorMessage={combinedError}
        hasLoaded={hasLoaded}
        label="artifacts"
        pending={query.isFetching}
      />
      <SectionFeedback
        errorMessage={combinedError}
        hasLoaded={hasLoaded}
        isEmpty={items.length === 0}
        itemsLabel="artifacts"
      />
      {items.length > 0 ? (
        <div className="flex flex-col">
          {items.map((artifact) => {
            const isExpanded = expandedArtifactId === artifact.id;
            const isSaving =
              saveMutation.isPending && saveMutation.variables?.artifact.id === artifact.id;
            return (
              <div className="flex flex-col" data-artifact-id={artifact.id} key={artifact.id}>
                <ArtifactRow
                  artifact={artifact}
                  expanded={isExpanded}
                  onSave={() => void handleSave(artifact)}
                  onToggle={() => handleToggleExpand(artifact)}
                  saving={isSaving}
                />
                {isExpanded ? (
                  <ArtifactViewer
                    artifact={artifact}
                    content={contentQuery.data}
                    errorMessage={contentError}
                    isLoading={contentQuery.isPending}
                  />
                ) : null}
              </div>
            );
          })}
        </div>
      ) : null}
    </section>
  );
}

function ArtifactRow({
  artifact,
  expanded,
  onSave,
  onToggle,
  saving,
}: {
  artifact: ArtifactSummary;
  expanded: boolean;
  onSave: () => void;
  onToggle: () => void;
  saving: boolean;
}) {
  return (
    <div
      className="flex items-center gap-2 py-1 font-[var(--font-mono)] text-[12px] text-[var(--fg)]"
      data-artifact-row=""
      data-artifact-expanded={expanded ? "true" : undefined}
    >
      <button
        aria-expanded={expanded}
        aria-label={
          expanded ? `collapse ${artifact.kind} artifact` : `expand ${artifact.kind} artifact`
        }
        className="flex min-w-0 flex-1 items-center gap-2 bg-transparent p-0 text-left outline-none hover:text-[var(--fg)] focus-visible:underline"
        data-artifact-toggle=""
        onClick={onToggle}
        type="button"
      >
        <span
          aria-hidden="true"
          className="shrink-0 text-[10px] text-[var(--fg-dim)]"
          data-artifact-chevron={expanded ? "down" : "right"}
        >
          {expanded ? "▾" : "▸"}
        </span>
        <span
          className="shrink-0 rounded border border-[var(--border)] px-1.5 py-[1px] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]"
          data-artifact-kind-badge=""
        >
          {artifact.kind}
        </span>
        <span className="min-w-0 flex-1 truncate" title={artifact.storagePath}>
          {artifact.storagePath}
        </span>
        <span className="shrink-0 text-[11px] text-[var(--fg-mute)]">run {artifact.runId}</span>
      </button>
      <Button
        aria-label={`save ${artifact.kind} artifact as…`}
        data-artifact-save-button=""
        disabled={saving}
        onClick={onSave}
        size="sm"
        type="button"
        variant="secondary"
      >
        {saving ? "saving…" : "save as…"}
      </Button>
    </div>
  );
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export interface SaveArtifactUxEffect {
  readonly errorMessage: string | null;
  readonly invalidateArtifactList: boolean;
}

/**
 * Maps a {@link SaveArtifactAsResult} to the corresponding UX effect so
 * every branch of the discriminated union is handled explicitly:
 *
 * - `saved` clears any prior error
 * - `cancelled` stays silent (no error, no refresh)
 * - `missing` surfaces the shared missing-reason copy and triggers an
 *   artifact-list invalidation so the stale list refreshes
 *
 * Exported for unit tests.
 */
export function classifySaveArtifactResult(result: SaveArtifactAsResult): SaveArtifactUxEffect {
  switch (result.status) {
    case "saved":
      return { errorMessage: null, invalidateArtifactList: false };
    case "cancelled":
      return { errorMessage: null, invalidateArtifactList: false };
    case "missing":
      return {
        errorMessage: describeArtifactMissingReason(result.reason),
        invalidateArtifactList: true,
      };
  }
}
