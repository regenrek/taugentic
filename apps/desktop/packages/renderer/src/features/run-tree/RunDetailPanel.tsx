import { createContext, useContext, useMemo, useState, type JSX, type ReactNode } from "react";

import type {
  CapsuleResult,
  ConflictSummary,
  ContextReceipt,
  OutputContractKind,
  RunDetail,
  RunEventDelta,
  RunId,
  RunListEntry,
  RunTimeline,
  SessionId,
  ValidationError,
  WorktreeInfo,
} from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Tabs } from "@/components/ui/tabs";
import {
  useRunConflictWarningsQuery,
  useRunDetailQuery,
  useRunEventTimelineQuery,
  useRunTimelineQuery,
  type RunConflictWarningItem,
} from "@/lib/queries/session-queries";
import { cn } from "@/lib/ui/cn";

import { RunTreeStatusBadge } from "./RunTreeNodeView";
import { RunEventTimelineList } from "./RunEventTimelineList";
import { EmptyDetailState, RunTimelineTab } from "./RunTimelineTab";
import { RunErrorSummary } from "./RunErrorSummary";
import { RunLogsTab } from "./RunLogsTab";
import { RunReplayControl } from "./RunReplayControl";

type RunDetailTab =
  | "result"
  | "membrane"
  | "logs"
  | "timeline"
  | "workspace"
  | "violation"
  | "quarantine"
  | "raw";

interface RunDetailPanelContextValue {
  detail?: RunDetail | null;
  run: RunListEntry | null;
  selectedRunId: RunId | null;
  sessionId: SessionId | null;
}

export interface RunDetailPanelProviderProps extends RunDetailPanelContextValue {
  children: ReactNode;
}

export interface RunDetailPanelViewProps {
  activeTab?: RunDetailTab;
  className?: string;
  conflictWarnings?: RunConflictWarningItem[];
  detail?: RunDetail | null;
  isConflictWarningsFetching?: boolean;
  isDetailFetching?: boolean;
  isTimelineFetching?: boolean;
  isRunTimelineFetching?: boolean;
  onTabChange?: (tab: RunDetailTab) => void;
  run: RunListEntry | null;
  sessionId?: SessionId | null;
  timelineEvents?: RunEventDelta[];
  runTimeline?: RunTimeline | null;
}

const RunDetailPanelContext = createContext<RunDetailPanelContextValue | null>(null);

export function RunDetailPanelProvider({
  children,
  detail,
  run,
  selectedRunId,
  sessionId,
}: RunDetailPanelProviderProps): JSX.Element {
  const value = useMemo(
    () => ({
      detail: detail ?? null,
      run,
      selectedRunId,
      sessionId,
    }),
    [detail, run, selectedRunId, sessionId],
  );

  return <RunDetailPanelContext.Provider value={value}>{children}</RunDetailPanelContext.Provider>;
}

export function RunDetailPanel(): JSX.Element | null {
  const context = useContext(RunDetailPanelContext);
  const selectedRunId = context?.selectedRunId ?? null;
  const selectedRun = context?.run ?? null;

  if (selectedRunId === null || selectedRun === null) {
    return null;
  }

  return (
    <RunDetailPanelBound
      detail={context?.detail ?? null}
      run={selectedRun}
      runId={selectedRunId}
      sessionId={context?.sessionId ?? null}
    />
  );
}

function RunDetailPanelBound({
  detail,
  run,
  runId,
  sessionId,
}: {
  detail: RunDetail | null;
  run: RunListEntry;
  runId: RunId;
  sessionId: SessionId | null;
}): JSX.Element {
  const [activeTab, setActiveTab] = useState<RunDetailTab>("result");
  const detailQuery = useRunDetailQuery(sessionId, runId);
  const conflictWarningsQuery = useRunConflictWarningsQuery(sessionId, runId);
  const timelineQuery = useRunEventTimelineQuery(sessionId, runId);
  const runTimelineQuery = useRunTimelineQuery(sessionId, runId);

  return (
    <RunDetailPanelView
      activeTab={activeTab}
      conflictWarnings={conflictWarningsQuery.data ?? []}
      detail={detail ?? detailQuery.data ?? null}
      isConflictWarningsFetching={conflictWarningsQuery.isFetching}
      isDetailFetching={detailQuery.isFetching}
      isTimelineFetching={timelineQuery.isFetching}
      isRunTimelineFetching={runTimelineQuery.isFetching}
      onTabChange={setActiveTab}
      run={run}
      sessionId={sessionId}
      timelineEvents={timelineQuery.data ?? []}
      runTimeline={runTimelineQuery.data ?? null}
    />
  );
}

export function RunDetailPanelView({
  activeTab,
  className,
  conflictWarnings = [],
  detail,
  isConflictWarningsFetching = false,
  isDetailFetching = false,
  isTimelineFetching = false,
  isRunTimelineFetching = false,
  onTabChange,
  run,
  sessionId = null,
  timelineEvents = [],
  runTimeline = null,
}: RunDetailPanelViewProps): JSX.Element | null {
  if (run === null) {
    return null;
  }

  const result = detail?.result ?? null;
  const validationError = detail?.contractViolation ?? null;
  const quarantineReceipt = detail?.quarantineReceipt ?? null;
  const workspaceInfo = detail?.workspaceInfo ?? run.workspaceInfo ?? null;
  const claimedFiles = detail?.claimedFiles ?? run.claimedFiles ?? [];
  const conflictSummary = detail?.conflictSummary ?? run.conflictSummary ?? null;
  const visibleTabs = createVisibleTabs({
    claimedFiles,
    conflictSummary,
    conflictWarnings,
    quarantineReceipt,
    validationError,
    workspaceInfo,
  });
  const selectedTab = activeTab && visibleTabs.includes(activeTab) ? activeTab : "result";

  function handleTabChange(next: unknown): void {
    if (typeof next === "string" && isRunDetailTab(next)) {
      onTabChange?.(next);
    }
  }

  return (
    <aside
      aria-label="Run detail"
      className={cn(
        "flex min-h-[220px] min-w-0 flex-col rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-raised)] xl:w-[25rem] xl:flex-none",
        className,
      )}
      data-run-detail-panel=""
      data-run-id={run.id}
    >
      <RunDetailHeader run={run} sessionId={sessionId} />
      <Tabs.Root
        className="min-h-0 flex-1 gap-0"
        onValueChange={handleTabChange}
        value={selectedTab}
      >
        <Tabs.List aria-label="Run detail tabs" className="px-2" variant="line">
          <Tabs.Trigger value="result" variant="line">
            Result
          </Tabs.Trigger>
          <Tabs.Trigger value="membrane" variant="line">
            Membrane
          </Tabs.Trigger>
          <Tabs.Trigger value="logs" variant="line">
            Logs
          </Tabs.Trigger>
          <Tabs.Trigger value="timeline" variant="line">
            Timeline
          </Tabs.Trigger>
          {visibleTabs.includes("workspace") ? (
            <Tabs.Trigger value="workspace" variant="line">
              Workspace
            </Tabs.Trigger>
          ) : null}
          {validationError !== null ? (
            <Tabs.Trigger value="violation" variant="line">
              Violation
            </Tabs.Trigger>
          ) : null}
          {quarantineReceipt !== null ? (
            <Tabs.Trigger value="quarantine" variant="line">
              Quarantine
            </Tabs.Trigger>
          ) : null}
          <Tabs.Trigger value="raw" variant="line">
            Raw
          </Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="result">
          <RunErrorSummary
            run={run}
            timelineEvents={timelineEvents}
            validationError={validationError}
          />
          <ResultTab
            isDetailFetching={isDetailFetching}
            outputContract={detail?.outputContract ?? run.outputContract ?? null}
            result={result}
            validationError={validationError}
          />
        </Tabs.Content>
        <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="membrane">
          <MembraneTab
            claimedFiles={claimedFiles}
            conflictSummary={conflictSummary}
            conflictWarnings={conflictWarnings}
            detail={detail ?? null}
            isTimelineFetching={isTimelineFetching}
            run={run}
            timelineEvents={timelineEvents}
            workspaceInfo={workspaceInfo}
          />
        </Tabs.Content>
        <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="logs">
          <RunLogsTab events={timelineEvents} isFetching={isTimelineFetching} />
        </Tabs.Content>
        <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="timeline">
          <RunTimelineTab isFetching={isRunTimelineFetching} timeline={runTimeline} />
        </Tabs.Content>
        {visibleTabs.includes("workspace") ? (
          <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="workspace">
            <WorkspaceTab
              claimedFiles={claimedFiles}
              conflictSummary={conflictSummary}
              conflictWarnings={conflictWarnings}
              isConflictWarningsFetching={isConflictWarningsFetching}
              workspaceInfo={workspaceInfo}
            />
          </Tabs.Content>
        ) : null}
        {validationError !== null ? (
          <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="violation">
            <ViolationTab validationError={validationError} />
          </Tabs.Content>
        ) : null}
        {quarantineReceipt !== null ? (
          <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="quarantine">
            <QuarantineTab receipt={quarantineReceipt} />
          </Tabs.Content>
        ) : null}
        <Tabs.Content className="min-h-0 flex-1 overflow-auto px-3 py-3" value="raw">
          <RawRunTab detail={detail ?? null} isDetailFetching={isDetailFetching} />
        </Tabs.Content>
      </Tabs.Root>
    </aside>
  );
}

function RunDetailHeader({
  run,
  sessionId,
}: {
  run: RunListEntry;
  sessionId: SessionId | null;
}): JSX.Element {
  return (
    <header className="flex flex-col gap-2 border-b border-[var(--border)] px-3 py-3">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className="min-w-0 flex-1 truncate font-[var(--font-mono)] text-[12px] font-medium text-[var(--fg)]"
          title={run.id}
        >
          {shortRunId(run.id)}
        </span>
        {run.recipeId ? (
          <Badge
            className="max-w-[10rem] truncate px-1.5 py-0 text-[9px] normal-case tracking-normal"
            title={run.recipeId}
            variant="outline"
          >
            {run.recipeId}
          </Badge>
        ) : null}
        <RunTreeStatusBadge status={run.status} />
      </div>
      <div className="flex flex-wrap items-center gap-2 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.14em] text-[var(--fg-dim)]">
        <span>{run.harness}</span>
        {run.outputContract ? <span>contract: {run.outputContract}</span> : null}
        {formatTiming(run) ? <span>{formatTiming(run)}</span> : null}
      </div>
      <div className="flex justify-end">
        <RunReplayControl run={run} sessionId={sessionId} />
      </div>
    </header>
  );
}

function ResultTab({
  isDetailFetching,
  outputContract,
  result,
  validationError,
}: {
  isDetailFetching: boolean;
  outputContract: OutputContractKind | null;
  result: CapsuleResult | null;
  validationError: ValidationError | null;
}): JSX.Element {
  if (result !== null && validationError === null) {
    return <JsonBlock data={result} label="CapsuleResult" />;
  }

  return (
    <EmptyDetailState
      message={
        isDetailFetching
          ? "loading run result"
          : outputContract === null
            ? "no output contract set for this run"
            : "no valid result available for this run"
      }
    />
  );
}

function ViolationTab({ validationError }: { validationError: ValidationError }): JSX.Element {
  return (
    <div className="flex flex-col gap-2 font-[var(--font-mono)] text-[11px]">
      <Field label="kind" value={validationError.kind} />
      <JsonBlock data={validationError.value} label="value" />
    </div>
  );
}

function QuarantineTab({ receipt }: { receipt: ContextReceipt }): JSX.Element {
  return (
    <div className="flex flex-col gap-2 font-[var(--font-mono)] text-[11px]">
      <Field label="id" value={receipt.id} />
      <Field label="kind" value={receipt.kind} />
      <Field label="state" value={receipt.state} />
      <Field label="reason" value={receipt.summary ?? "quarantined result"} />
      <JsonBlock data={receipt.provenance} label="provenance" />
    </div>
  );
}

function WorkspaceTab({
  claimedFiles,
  conflictSummary,
  conflictWarnings,
  isConflictWarningsFetching,
  workspaceInfo,
}: {
  claimedFiles: string[];
  conflictSummary: ConflictSummary | null;
  conflictWarnings: RunConflictWarningItem[];
  isConflictWarningsFetching: boolean;
  workspaceInfo: WorktreeInfo | null;
}): JSX.Element {
  return (
    <div className="flex flex-col gap-4 font-[var(--font-mono)] text-[11px]">
      <section className="flex flex-col gap-2">
        <SectionLabel>Worktree</SectionLabel>
        {workspaceInfo !== null ? (
          <div className="flex flex-col gap-2">
            <Field label="path" value={workspaceInfo.path} />
            <Field label="branch" value={workspaceInfo.branch} />
            <Field label="cleanup" value={workspaceInfo.cleanupPolicy} />
          </div>
        ) : (
          <EmptyDetailState message="no worktree assigned to this run" />
        )}
      </section>
      <section className="flex flex-col gap-2">
        <SectionLabel>Claimed Files</SectionLabel>
        <StringList emptyMessage="no declared file claims" values={claimedFiles} />
      </section>
      <section className="flex flex-col gap-2">
        <SectionLabel>Conflict Warnings</SectionLabel>
        <ConflictWarningList
          conflictSummary={conflictSummary}
          conflictWarnings={conflictWarnings}
          isFetching={isConflictWarningsFetching}
        />
      </section>
    </div>
  );
}

function MembraneTab({
  claimedFiles,
  conflictSummary,
  conflictWarnings,
  detail,
  isTimelineFetching,
  run,
  timelineEvents,
  workspaceInfo,
}: {
  claimedFiles: string[];
  conflictSummary: ConflictSummary | null;
  conflictWarnings: RunConflictWarningItem[];
  detail: RunDetail | null;
  isTimelineFetching: boolean;
  run: RunListEntry;
  timelineEvents: RunEventDelta[];
  workspaceInfo: WorktreeInfo | null;
}): JSX.Element {
  const outputContract = detail?.outputContract ?? run.outputContract ?? null;
  const tokenUsage = detail?.tokenUsage ?? null;
  return (
    <div className="flex flex-col gap-4 font-[var(--font-mono)] text-[11px]">
      <section className="flex flex-col gap-2">
        <SectionLabel>Input Boundary</SectionLabel>
        <Field label="run" value={shortRunId(run.id)} />
        <Field
          label="objective"
          value={detail?.summary.objective ?? run.objectivePreview ?? run.id}
        />
        {run.parentRunId ? <Field label="parent" value={shortRunId(run.parentRunId)} /> : null}
        {run.recipeId ? <Field label="recipe" value={run.recipeId} /> : null}
        {outputContract ? <Field label="contract" value={outputContract} /> : null}
      </section>
      <section className="flex flex-col gap-2">
        <SectionLabel>Workspace Boundary</SectionLabel>
        {workspaceInfo !== null ? (
          <>
            <Field label="worktree" value={workspaceInfo.path} />
            <Field label="branch" value={workspaceInfo.branch} />
          </>
        ) : (
          <EmptyDetailState message="no isolated worktree recorded" />
        )}
        <StringList emptyMessage="no declared file claims" values={claimedFiles} />
      </section>
      <section className="flex flex-col gap-2">
        <SectionLabel>Outputs</SectionLabel>
        <Field label="status" value={run.status} />
        {detail?.result ? <Field label="result" value={detail.result.kind} /> : null}
        {tokenUsage !== null ? (
          <>
            <Field label="tokens" value={formatTokenTotal(tokenUsage)} />
            <Field label="prompt" value={tokenUsage.promptTokens.toString()} />
            <Field label="completion" value={tokenUsage.completionTokens.toString()} />
            <Field label="cached" value={tokenUsage.cachedTokens.toString()} />
            <Field label="reasoning" value={tokenUsage.reasoningTokens.toString()} />
          </>
        ) : null}
        {detail?.quarantineReceipt ? (
          <Field label="receipt" value={detail.quarantineReceipt.id} />
        ) : null}
        {conflictSummary !== null && conflictSummary.warningCount > 0 ? (
          <Field label="conflicts" value={String(conflictSummary.warningCount)} />
        ) : null}
        {conflictWarnings.length > 0 ? (
          <Field label="warnings" value={String(conflictWarnings.length)} />
        ) : null}
      </section>
      <section className="flex flex-col gap-2">
        <SectionLabel>Timeline</SectionLabel>
        <RunEventTimelineList events={timelineEvents} isFetching={isTimelineFetching} />
      </section>
    </div>
  );
}

function formatTokenTotal(tokenUsage: NonNullable<RunDetail["tokenUsage"]>): string {
  return (tokenUsage.promptTokens + tokenUsage.completionTokens).toString();
}

function ConflictWarningList({
  conflictSummary,
  conflictWarnings,
  isFetching,
}: {
  conflictSummary: ConflictSummary | null;
  conflictWarnings: RunConflictWarningItem[];
  isFetching: boolean;
}): JSX.Element {
  if (conflictWarnings.length > 0) {
    return (
      <div className="flex flex-col gap-2">
        {conflictWarnings.map((item, index) => (
          <div
            className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-2"
            key={`${item.runId}-${item.occurredAtMs.toString()}-${index}`}
          >
            <div className="mb-1 flex flex-wrap items-center gap-2 text-[10px] uppercase tracking-[0.14em] text-[var(--fg-dim)]">
              <span>{formatDateTime(item.occurredAtMs)}</span>
              <span>{item.warning.severity}</span>
            </div>
            <Field label="request" value={shortRunId(item.warning.requestingCapsule)} />
            <div className="mt-2 flex flex-col gap-1">
              {item.warning.conflicts.map((conflict) => (
                <div
                  className="grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2"
                  key={`${conflict.holdingCapsule}:${conflict.file}`}
                >
                  <span className="uppercase tracking-[0.14em] text-[var(--fg-dim)]">
                    {conflict.holdingKind}
                  </span>
                  <span className="min-w-0 break-words text-[var(--fg)]">
                    {conflict.file} held by {shortRunId(conflict.holdingCapsule)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (conflictSummary !== null && conflictSummary.warningCount > 0) {
    return (
      <div className="flex flex-col gap-2">
        <Field label="warnings" value={String(conflictSummary.warningCount)} />
        <StringList emptyMessage="no conflict files listed" values={conflictSummary.files} />
        {isFetching ? <EmptyDetailState message="loading conflict event timestamps" /> : null}
      </div>
    );
  }

  return <EmptyDetailState message={isFetching ? "loading conflicts" : "no conflict warnings"} />;
}

function RawRunTab({
  detail,
  isDetailFetching,
}: {
  detail: RunDetail | null;
  isDetailFetching: boolean;
}): JSX.Element {
  if (detail === null) {
    return (
      <EmptyDetailState message={isDetailFetching ? "loading raw run data" : "no raw run data"} />
    );
  }

  return <JsonBlock data={detail} label="raw run data" />;
}

function Field({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-2">
      <span className="uppercase tracking-[0.14em] text-[var(--fg-dim)]">{label}</span>
      <span className="min-w-0 break-words text-[var(--fg)]">{value}</span>
    </div>
  );
}

function SectionLabel({ children }: { children: string }): JSX.Element {
  return (
    <span className="font-[var(--font-mono)] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]">
      {children}
    </span>
  );
}

function StringList({
  emptyMessage,
  values,
}: {
  emptyMessage: string;
  values: string[];
}): JSX.Element {
  if (values.length === 0) {
    return <EmptyDetailState message={emptyMessage} />;
  }

  return (
    <ul className="flex flex-col gap-1">
      {values.map((value) => (
        <li className="min-w-0 break-words text-[var(--fg)]" key={value}>
          {value}
        </li>
      ))}
    </ul>
  );
}

function JsonBlock({ data, label }: { data: unknown; label: string }): JSX.Element {
  return (
    <div className="flex min-h-0 flex-col gap-1">
      <span className="font-[var(--font-mono)] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]">
        {label}
      </span>
      <pre className="max-h-[360px] overflow-auto rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-2 font-[var(--font-mono)] text-[11px] leading-5 text-[var(--fg)]">
        {serializeJson(data)}
      </pre>
    </div>
  );
}

function createVisibleTabs({
  claimedFiles,
  conflictSummary,
  conflictWarnings,
  quarantineReceipt,
  validationError,
  workspaceInfo,
}: {
  claimedFiles: string[];
  conflictSummary: ConflictSummary | null;
  conflictWarnings: RunConflictWarningItem[];
  quarantineReceipt: ContextReceipt | null;
  validationError: ValidationError | null;
  workspaceInfo: WorktreeInfo | null;
}): RunDetailTab[] {
  const tabs: RunDetailTab[] = ["result", "membrane", "logs", "timeline"];
  if (
    workspaceInfo !== null ||
    claimedFiles.length > 0 ||
    conflictWarnings.length > 0 ||
    (conflictSummary !== null && conflictSummary.warningCount > 0)
  ) {
    tabs.push("workspace");
  }
  if (validationError !== null) {
    tabs.push("violation");
  }
  if (quarantineReceipt !== null) {
    tabs.push("quarantine");
  }
  tabs.push("raw");
  return tabs;
}

function isRunDetailTab(value: string): value is RunDetailTab {
  return (
    value === "result" ||
    value === "membrane" ||
    value === "logs" ||
    value === "timeline" ||
    value === "workspace" ||
    value === "violation" ||
    value === "quarantine" ||
    value === "raw"
  );
}

function formatTiming(run: RunListEntry): string | null {
  const started = toNumber(run.startedAtMs);
  if (started === null) {
    return null;
  }

  const ended = toNumber(run.endedAtMs);
  if (ended === null) {
    return `started ${formatClock(started)}`;
  }

  return `${formatClock(started)} - ${formatClock(ended)}`;
}

function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDateTime(ms: bigint | number): string {
  const timestamp = typeof ms === "bigint" ? Number(ms) : ms;
  return new Date(timestamp).toLocaleString([], {
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    day: "2-digit",
  });
}

function shortRunId(runId: string): string {
  if (runId.length <= 14) {
    return runId;
  }
  return `${runId.slice(0, 8)}...${runId.slice(-4)}`;
}

function toNumber(value: bigint | number | null | undefined): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  return typeof value === "bigint" ? Number(value) : value;
}

function serializeJson(data: unknown): string {
  return JSON.stringify(data, bigintReplacer, 2);
}

function bigintReplacer(_key: string, value: unknown): unknown {
  if (typeof value === "bigint") {
    return value.toString();
  }
  return value;
}
