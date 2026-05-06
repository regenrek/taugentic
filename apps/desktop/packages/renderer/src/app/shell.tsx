import type { CSSProperties } from "react";

import { AlertTriangle, Moon, Sun } from "lucide-react";

import type { SessionId } from "@taugentic/desktop-shared";

import { ScrollArea } from "@/components/ui/scroll-area";
import { deriveDaemonShellSummary, type DaemonControlModel } from "@/features/daemon/model";
import { AgentRuntimePanel } from "@/features/agent-runtime";
import { AgentVisualizationPanel } from "@/features/agent-visualization";
import { SessionRail } from "@/features/overview";
import { GlobalActivityLog } from "@/features/activity-log";
import { AttentionStrip } from "@/features/attention-strip";
import { AttentionCard, ProviderHealthCard } from "@/features/inspector-cards";
import { MissionControlPanel } from "@/features/mission-control";
import { DesktopWindowChrome } from "@/features/window/chrome";
import { useThemeMode } from "@/lib/theme/use-theme-mode";
import { cn } from "@/lib/ui/cn";

const WINDOW_NO_DRAG_STYLE = {
  WebkitAppRegion: "no-drag",
} as CSSProperties;

export interface AppShellProps {
  currentSessionId: SessionId | null;
  daemon: DaemonControlModel;
  /** Forwarded to inline surfaces that trigger session-scoped runs (run composer). */
  onRunStarted: () => void;
  onSessionChange: (sessionId: SessionId | null) => void;
}

/**
 * Operator workspace shell.
 *
 * Layout strategy (shadcn + Tailwind v4, 2026):
 * - A single outer grid with explicit row tracks ensures the three workspace
 *   rails never lose their height when the window resizes.
 * - The workspace row is a CSS grid with `minmax()` column tracks so each rail
 *   has a guaranteed readable minimum width and the center grows.
 * - Viewport max-breakpoints collapse the right rail (medium windows) and then
 *   the left rail (narrow windows) into the bottom chrome so the shell stays
 *   usable at every desktop size.
 * - `h-dvh` + `min-h-0` chain on every scroll container guarantees the inner
 *   `<ScrollArea>`s actually scroll instead of pushing the page.
 */
export function AppShell({
  currentSessionId,
  daemon,
  onRunStarted,
  onSessionChange,
}: AppShellProps) {
  const daemonShell = deriveDaemonShellSummary(daemon);
  const sessionLabel = currentSessionId ?? "—";
  const isDegraded = daemonShell.isDegraded;

  return (
    <main
      className={cn(
        "grid h-dvh w-dvw overflow-hidden bg-[var(--bg)] text-[var(--fg)] font-[var(--font-mono)]",
        isDegraded
          ? "grid-rows-[auto_auto_minmax(0,1fr)_auto]"
          : "grid-rows-[auto_minmax(0,1fr)_auto]",
      )}
    >
      <DesktopWindowChrome>
        <TopBarContent
          daemon={daemon}
          isDegraded={isDegraded}
          sessionLabel={sessionLabel}
          statusLabel={daemonShell.statusLabel}
        />
      </DesktopWindowChrome>

      {isDegraded ? (
        <DaemonDegradedRow message={daemon.errorMessage ?? "daemon unavailable"} />
      ) : null}

      <WorkspaceGrid
        currentSessionId={currentSessionId}
        onRunStarted={onRunStarted}
        onSessionChange={onSessionChange}
      />

      <div className="border-t border-[var(--border)] bg-[var(--bg-raised)]">
        <AttentionStrip daemon={daemon} />
      </div>
    </main>
  );
}

function WorkspaceGrid({
  currentSessionId,
  onRunStarted,
  onSessionChange,
}: {
  currentSessionId: SessionId | null;
  onRunStarted: () => void;
  onSessionChange: (sessionId: SessionId | null) => void;
}) {
  return (
    <div className="workspace-grid" data-workspace-grid>
      <WorkspacePanel label="Sessions" slot="sessions">
        <SessionRail onSelect={onSessionChange} selectedSessionId={currentSessionId} />
      </WorkspacePanel>

      <WorkspacePanel label="Session detail" slot="detail">
        <AgentVisualizationPanel onRunStarted={onRunStarted} sessionId={currentSessionId} />
      </WorkspacePanel>

      <WorkspacePanel label="Activity log" slot="activity">
        <div className="flex h-full min-h-0 flex-col">
          <MissionControlPanel />
          <AgentRuntimePanel />
          <ProviderHealthCard />
          <AttentionCard />
          <div className="min-h-0 flex-1">
            <GlobalActivityLog />
          </div>
        </div>
      </WorkspacePanel>
    </div>
  );
}

function WorkspacePanel({
  children,
  label,
  slot,
}: {
  children: React.ReactNode;
  label: string;
  slot: "sessions" | "detail" | "activity";
}) {
  return (
    <section
      aria-label={label}
      className="flex min-h-0 min-w-0 flex-col overflow-hidden bg-[var(--bg)]"
      data-workspace-slot={slot}
    >
      <ScrollArea className="h-full w-full" viewportClassName="size-full">
        {children}
      </ScrollArea>
    </section>
  );
}

function TopBarContent({
  daemon,
  isDegraded,
  sessionLabel,
  statusLabel,
}: {
  daemon: DaemonControlModel;
  isDegraded: boolean;
  sessionLabel: string;
  statusLabel: string;
}) {
  const actualMode = daemon.state?.actualMode ?? "unknown";
  const transitionStatus = daemon.state?.transitionStatus ?? "—";
  const daemonVersion = daemon.state?.daemonVersion ?? null;

  return (
    <div className="flex min-w-0 items-center gap-3 text-[12px] @container">
      <span className="shrink-0 text-[10px] uppercase tracking-[0.24em] text-[var(--fg-dim)]">
        Taugentic
      </span>
      <span className="hidden h-4 w-px shrink-0 bg-[var(--border)] @[520px]:block" aria-hidden />
      <BarField
        className="hidden @[520px]:flex"
        label="mode"
        tone={isDegraded ? "danger" : "default"}
        value={actualMode}
      />
      <BarField className="hidden @[640px]:flex" label="status" value={transitionStatus} />
      <BarField
        className="hidden @[760px]:flex"
        label="shell"
        tone={isDegraded ? "danger" : "default"}
        value={statusLabel}
      />
      {daemonVersion !== null ? (
        <BarField className="hidden @[900px]:flex" label="ver" value={daemonVersion} />
      ) : null}
      <BarField
        className="min-w-0 flex-1"
        label="session"
        mono
        tone={sessionLabel === "—" ? "mute" : "default"}
        value={sessionLabel}
      />
      <div className="shrink-0">
        <ThemeModeButton />
      </div>
    </div>
  );
}

function BarField({
  className,
  label,
  mono = false,
  tone = "default",
  value,
}: {
  className?: string;
  label: string;
  mono?: boolean;
  tone?: "default" | "danger" | "mute";
  value: string;
}) {
  const valueTone =
    tone === "danger"
      ? "text-[var(--status-failed)]"
      : tone === "mute"
        ? "text-[var(--fg-mute)]"
        : "text-[var(--fg)]";

  return (
    <div className={cn("flex min-w-0 items-center gap-2", className)}>
      <span className="shrink-0 uppercase tracking-[0.18em] text-[var(--fg-dim)] text-[10px]">
        {label}
      </span>
      <span
        className={cn(
          "truncate",
          mono ? "font-[var(--font-mono)]" : "font-[var(--font-mono)]",
          valueTone,
        )}
        title={value}
      >
        {value}
      </span>
    </div>
  );
}

function ThemeModeButton() {
  const { mode, toggle } = useThemeMode();
  const nextLabel = mode === "dark" ? "Switch to light theme" : "Switch to dark theme";
  const Icon = mode === "dark" ? Sun : Moon;
  return (
    <button
      aria-label={nextLabel}
      className="inline-flex size-7 items-center justify-center border border-[var(--border)] bg-[var(--bg)] text-[var(--fg-dim)] transition-colors hover:border-[var(--border-strong)] hover:bg-[var(--bg-sunken)] hover:text-[var(--fg)]"
      onClick={() => toggle()}
      style={WINDOW_NO_DRAG_STYLE}
      title={nextLabel}
      type="button"
    >
      <Icon className="size-3.5" />
    </button>
  );
}

function DaemonDegradedRow({ message }: { message: string }) {
  return (
    <div
      aria-live="polite"
      className="flex items-center gap-2 border-b border-[var(--status-failed)]/40 bg-[var(--bg-raised)] px-3 py-1.5 font-[var(--font-mono)] text-[12px] text-[var(--status-failed)]"
    >
      <AlertTriangle aria-hidden className="size-3.5" />
      <span className="uppercase tracking-[0.18em]">daemon degraded</span>
      <span className="truncate">{message}</span>
    </div>
  );
}
