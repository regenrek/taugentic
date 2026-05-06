import type { SessionId } from "@taugentic/desktop-shared";

import { useAgentStream } from "@/features/agent-stream";
import { cn } from "@/lib/ui/cn";

import {
  formatAgentTurnLine,
  formatLiveAgentTurnLines,
  sortActivityLinesAscending,
  type ActivityLine,
} from "./formatters";
import { SectionFeedback } from "./section-feedback";
import { SectionHeader } from "./section-header";

export interface AgentTurnsSectionProps {
  sessionId: SessionId;
  className?: string;
  variant?: "default" | "mission-control";
}

export function AgentTurnsSection({
  className,
  sessionId,
  variant = "default",
}: AgentTurnsSectionProps) {
  return (
    <AgentTurnsSectionView
      key={sessionId}
      className={className}
      sessionId={sessionId}
      variant={variant}
    />
  );
}

function AgentTurnsSectionView({ className, sessionId, variant }: AgentTurnsSectionProps) {
  const agentTurns = useAgentStream(sessionId);
  const items = agentTurns.committedRows;
  const hasLoaded = agentTurns.hasHydratedCommitted;
  const errorMessage = agentTurns.errorMessage ?? "";

  const durableLines: ActivityLine[] = items.map(formatAgentTurnLine);
  const liveLines = formatLiveAgentTurnLines(
    agentTurns.liveMessages,
    agentTurns.liveToolCalls,
    items,
  );
  const lines = sortActivityLinesAscending([...durableLines, ...liveLines]);
  const missionControl = variant === "mission-control";

  return (
    <section
      className={cn(
        "flex flex-col gap-2 px-3 py-3",
        missionControl
          ? "min-h-0 flex-1 gap-0 border-b border-[var(--border)] bg-[var(--bg-sunken)] px-0 py-0"
          : undefined,
        className,
      )}
      data-agent-turns-variant={variant}
      data-section="agent-turns"
    >
      <div className={cn(missionControl ? "px-3 py-3" : undefined)}>
        <SectionHeader
          count={lines.length}
          errorMessage={errorMessage.length > 0 ? errorMessage : null}
          hasLoaded={hasLoaded}
          label="agent turns"
          pending={
            !hasLoaded ||
            agentTurns.streamStatus === "connecting" ||
            agentTurns.streamStatus === "rehydratingCommitted" ||
            agentTurns.streamStatus === "reopeningLiveStream"
          }
        />
      </div>
      <div className={cn(missionControl ? "px-3" : undefined)}>
        <SectionFeedback
          errorMessage={errorMessage.length > 0 ? errorMessage : null}
          hasLoaded={hasLoaded}
          isEmpty={lines.length === 0}
          itemsLabel="agent turns"
        />
      </div>
      {lines.length > 0 ? (
        <div
          aria-label="Agent activity stream"
          className={cn(
            "flex flex-col gap-0 font-[var(--font-mono)] text-[12px] leading-5 text-[var(--fg)]",
            missionControl
              ? "min-h-0 flex-1 overflow-y-auto px-3 pb-3 text-[11px] leading-5 text-[var(--fg-dim)]"
              : undefined,
          )}
          role="log"
        >
          {lines.map((line) => (
            <div
              key={line.key}
              className={cn(
                "whitespace-pre-wrap",
                missionControl
                  ? "border-b border-[var(--border)]/35 py-1 last:border-b-0"
                  : undefined,
              )}
              data-line-kind={line.kind}
            >
              {line.text}
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
