/*
 * AgentVisualizationPanel.
 *
 * Mounted by `app/shell.tsx` in the WorkspacePanel slot=detail. Composes:
 *
 *   <RunHeader/>           -- session/run/status meta + manual cortex pause
 *   <CortexField/>         -- hero canvas (engine instance per session)
 *   <FocusedRunTabs/>      -- 5 tabs wrapping existing SessionDetail children
 *
 * When `sessionId === null`, only `<NoSessionPlaceholder/>` renders --
 * CortexField is NOT mounted and no agent-stream visualization driver is
 * created. This keeps the empty case cheap and avoids running the engine
 * when there is nothing to visualize.
 *
 * When `sessionId` changes, the focused subtree is keyed by the id so
 * React fully remounts the canvas + driver pair. The semantic
 * agent-stream driver is created once after the engine ref is populated
 * and disposed on unmount, so engine + visualization lifecycles always
 * pair cleanly.
 */

import { useLayoutEffect, useRef } from "react";

import { useSelector } from "@xstate/store/react";

import type { SessionId } from "@taugentic/desktop-shared";

import { CortexField, type CortexFieldHandle } from "@/features/cortex-canvas";
import { useAgentStream, type AgentStreamViewModel } from "@/features/agent-stream";
import { useMountEffect } from "@/lib/react/use-mount-effect";

import { createAgentStreamVisualizationDriver } from "./agent-stream-visualization";
import { FocusedRunTabs } from "./FocusedRunTabs";
import { NoSessionPlaceholder } from "./empty/NoSessionPlaceholder";
import { applyLaneEffect } from "./lane-effects";
import { RunHeader } from "./RunHeader";
import { motionStore, selectMotionPaused } from "./state/motion.store";

export interface AgentVisualizationPanelProps {
  sessionId: SessionId | null;
  onRunStarted: () => void;
}

export function AgentVisualizationPanel({ onRunStarted, sessionId }: AgentVisualizationPanelProps) {
  if (sessionId === null) {
    return <NoSessionPlaceholder />;
  }
  return (
    <FocusedAgentVisualization key={sessionId} onRunStarted={onRunStarted} sessionId={sessionId} />
  );
}

function FocusedAgentVisualization({
  onRunStarted,
  sessionId,
}: {
  onRunStarted: () => void;
  sessionId: SessionId;
}) {
  const fieldRef = useRef<CortexFieldHandle | null>(null);
  const cortexHostRef = useRef<HTMLDivElement | null>(null);
  const paused = useSelector(motionStore, selectMotionPaused);
  const agentStream = useAgentStream(sessionId);

  return (
    <div
      aria-label="Agent visualization"
      className="flex h-full min-h-0 w-full flex-col"
      data-agent-visualization="focused"
      data-session-id={sessionId}
      role="region"
    >
      <RunHeader sessionId={sessionId} />
      <div
        className="relative w-full shrink-0 border-b border-[var(--border)]"
        data-agent-visualization-cortex
        ref={cortexHostRef}
        style={{
          height: "160px",
          background: "var(--mc-field-bg, var(--bg-sunken))",
        }}
      >
        <CortexField paused={paused} ref={fieldRef} />
      </div>
      <AgentStreamVisualizationBridge
        agentStream={agentStream}
        fieldRef={fieldRef}
        hostRef={cortexHostRef}
        sessionId={sessionId}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        <FocusedRunTabs onRunStarted={onRunStarted} sessionId={sessionId} />
      </div>
    </div>
  );
}

/**
 * Constructs the semantic agent-stream visualization driver exactly once
 * per focused session (the parent is keyed by sessionId, so this
 * component remounts on session change), and disposes it on unmount.
 */
function AgentStreamVisualizationBridge({
  agentStream,
  fieldRef,
  hostRef,
  sessionId,
}: {
  agentStream: AgentStreamViewModel;
  fieldRef: React.RefObject<CortexFieldHandle | null>;
  hostRef: React.RefObject<HTMLDivElement | null>;
  sessionId: SessionId;
}) {
  const driverRef = useRef<ReturnType<typeof createAgentStreamVisualizationDriver> | null>(null);

  useMountEffect(() => {
    const engine = fieldRef.current?.engine ?? null;
    const host = hostRef.current;
    if (engine === null || host === null) {
      return undefined;
    }
    const driver = createAgentStreamVisualizationDriver({
      engine,
      host,
      sessionId,
      onLaneEffect: ({ effect, laneId }) => {
        applyLaneEffect({ effect, laneId });
      },
    });
    driver.sync(agentStream);
    driverRef.current = driver;
    let observer: ResizeObserver | null = null;
    const RO = (globalThis as { ResizeObserver?: typeof ResizeObserver }).ResizeObserver;
    if (typeof RO === "function") {
      observer = new RO(() => {
        driver.resize();
      });
      observer.observe(host);
    }
    return () => {
      observer?.disconnect();
      driver.dispose();
      driverRef.current = null;
    };
  });

  useLayoutEffect(() => {
    driverRef.current?.sync(agentStream);
  }, [agentStream]);

  return null;
}

export type { FocusedRunTabValue } from "./FocusedRunTabs";
