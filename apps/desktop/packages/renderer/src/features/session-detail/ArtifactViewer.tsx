import { useMemo, useState } from "react";

import type { ArtifactSummary, ReadArtifactContentResult } from "@taugentic/desktop-shared";

import { Input } from "@/components/ui/input";

import { describeArtifactMissingReason } from "./formatters";

export interface ArtifactViewerProps {
  artifact: ArtifactSummary;
  content: ReadArtifactContentResult | undefined;
  errorMessage: string | null;
  isLoading: boolean;
}

/**
 * Renders artifact content for a selected `ArtifactSummary`.
 *
 * Kind-specific rendering:
 * - `Patch` → unified diff viewer with minimal `+`/`-` coloring
 * - `CommandLog` / `Transcript` / `FileSnapshot` → searchable text viewer
 *
 * Size/missing fallbacks:
 * - `tooLarge` → explicit prompt to Save the artifact for offline inspection
 * - `missing` → terse not-found message
 */
export function ArtifactViewer({
  artifact,
  content,
  errorMessage,
  isLoading,
}: ArtifactViewerProps) {
  if (errorMessage !== null) {
    return (
      <div
        className="border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]"
        data-artifact-viewer="error"
      >
        error: {errorMessage}
      </div>
    );
  }
  if (isLoading || content === undefined) {
    return (
      <div
        className="border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]"
        data-artifact-viewer="loading"
      >
        loading artifact…
      </div>
    );
  }

  if (content.status === "missing") {
    return (
      <div
        className="border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]"
        data-artifact-viewer="missing"
        data-artifact-missing-reason={content.reason}
      >
        {describeArtifactMissingReason(content.reason)}
      </div>
    );
  }

  if (content.status === "tooLarge") {
    return (
      <div
        className="flex flex-col gap-1 border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]"
        data-artifact-viewer="too-large"
      >
        <div>
          too large to preview ({formatBytes(content.totalBytes)} &gt;{" "}
          {formatBytes(content.limitBytes)} cap)
        </div>
        <div className="text-[var(--fg-dim)]">use “save as…” to inspect offline</div>
      </div>
    );
  }

  if (artifact.kind === "Patch") {
    return <PatchViewer truncated={content.truncated} content={content.content} />;
  }
  return (
    <LogViewer
      truncated={content.truncated}
      content={content.content}
      totalBytes={content.totalBytes}
      readBytes={content.readBytes}
    />
  );
}

function PatchViewer({ content, truncated }: { content: string; truncated: boolean }) {
  const lines = useMemo(() => content.split("\n"), [content]);
  return (
    <div
      className="flex flex-col gap-1 border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2"
      data-artifact-viewer="patch"
      data-artifact-truncated={truncated ? "true" : undefined}
    >
      <pre
        className="max-h-[260px] overflow-auto font-[var(--font-mono)] text-[11px] leading-5"
        data-artifact-patch-body=""
      >
        {lines.map((line, index) => (
          <div
            className="whitespace-pre-wrap"
            data-patch-line-kind={patchLineKind(line)}
            key={`${index}-${line.length}`}
            style={patchLineStyle(line)}
          >
            {line === "" ? "\u200b" : line}
          </div>
        ))}
      </pre>
      {truncated ? (
        <div className="text-[10px] text-[var(--fg-mute)]">
          content truncated to preview cap; save as… for the full file
        </div>
      ) : null}
    </div>
  );
}

function LogViewer({
  content,
  readBytes,
  totalBytes,
  truncated,
}: {
  content: string;
  readBytes: number;
  totalBytes: number;
  truncated: boolean;
}) {
  const [query, setQuery] = useState<string>("");
  const trimmedQuery = query.trim();
  const lines = useMemo(() => content.split("\n"), [content]);
  const visibleLines = useMemo(() => {
    if (trimmedQuery.length === 0) {
      return lines;
    }
    const lowered = trimmedQuery.toLowerCase();
    return lines.filter((line) => line.toLowerCase().includes(lowered));
  }, [lines, trimmedQuery]);

  return (
    <div
      className="flex flex-col gap-1 border-t border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2"
      data-artifact-viewer="log"
      data-artifact-truncated={truncated ? "true" : undefined}
    >
      <div className="flex items-center gap-2">
        <Input
          aria-label="filter log"
          className="h-7 max-w-[240px] font-[var(--font-mono)] text-[11px]"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="filter…"
          type="search"
          value={query}
        />
        <span className="text-[10px] text-[var(--fg-mute)]">
          {visibleLines.length}/{lines.length} lines · {formatBytes(readBytes)} of{" "}
          {formatBytes(totalBytes)}
        </span>
      </div>
      <pre
        className="max-h-[260px] overflow-auto font-[var(--font-mono)] text-[11px] leading-5 text-[var(--fg)]"
        data-artifact-log-body=""
      >
        {visibleLines.map((line, index) => (
          <div className="whitespace-pre-wrap" key={`${index}-${line.length}`}>
            {line === "" ? "\u200b" : line}
          </div>
        ))}
      </pre>
      {truncated ? (
        <div className="text-[10px] text-[var(--fg-mute)]">
          log truncated to preview cap; save as… for the full file
        </div>
      ) : null}
    </div>
  );
}

/** Exported for unit tests. */
export function patchLineKind(
  line: string,
): "addition" | "deletion" | "hunk" | "header" | "context" {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("diff ")) {
    return "header";
  }
  if (line.startsWith("@@")) {
    return "hunk";
  }
  if (line.startsWith("+")) {
    return "addition";
  }
  if (line.startsWith("-")) {
    return "deletion";
  }
  return "context";
}

function patchLineStyle(line: string): { color?: string } {
  switch (patchLineKind(line)) {
    case "addition":
      return { color: "var(--status-ready, #4ade80)" };
    case "deletion":
      return { color: "var(--status-failed, #f87171)" };
    case "hunk":
      return { color: "var(--fg-dim)" };
    case "header":
      return { color: "var(--fg-dim)" };
    case "context":
      return {};
  }
}

/** Exported for unit tests. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export const ARTIFACT_VIEWER_PATCH_KINDS = [
  "addition",
  "deletion",
  "hunk",
  "header",
  "context",
] as const;
