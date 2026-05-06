import type {
  AgentStreamViewModel,
  LiveAgentMessage,
  LiveAgentToolCall,
} from "@/features/agent-stream";
import { assistantLogicalKey, toolLogicalKey } from "@/features/agent-stream";
import type { CortexEngine } from "@/features/cortex-canvas";

import type { SessionId } from "@taugentic/desktop-shared";

import type { LaneEffect } from "./lane-effects";

const COLUMN_STRIDE_PX = 100;

interface MessageProgress {
  completed: boolean;
  textLength: number;
}

interface ToolProgress {
  outcome: LiveAgentToolCall["outcome"];
  outputLength: number;
}

export interface AgentStreamVisualizationDriver {
  dispose(): void;
  resize(): void;
  sync(view: AgentStreamViewModel): void;
}

export interface CreateAgentStreamVisualizationDriverOptions {
  engine: CortexEngine;
  host: HTMLElement;
  sessionId: SessionId;
  onLaneEffect?(this: void, args: { laneId: string; effect: LaneEffect }): void;
}

export function createAgentStreamVisualizationDriver(
  opts: CreateAgentStreamVisualizationDriverOptions,
): AgentStreamVisualizationDriver {
  const engine = opts.engine;
  const host = opts.host;
  const onLaneEffect = opts.onLaneEffect;
  const sessionId = opts.sessionId;
  let disposed = false;
  let mainColumnIndex = 0;
  let widthPx = 1;
  let heightPx = 1;
  let pendingState: string | null = null;
  let activeToolKeys: string[] = [];
  const messageProgress = new Map<string, MessageProgress>();
  const toolProgress = new Map<string, ToolProgress>();
  const branchLaneIds = new Set<string>();
  const branchAnchorIds = new Set<string>();

  function resize(): void {
    if (disposed) {
      return;
    }
    const dpr = (globalThis as { devicePixelRatio?: number }).devicePixelRatio ?? 1;
    widthPx = Math.max(1, Math.floor(host.clientWidth * dpr));
    heightPx = Math.max(1, Math.floor(host.clientHeight * dpr));
    mainColumnIndex = Math.max(0, Math.round((widthPx * 0.5 - 50) / COLUMN_STRIDE_PX));
    engine.registerLane({ laneId: sessionId, columnIndex: mainColumnIndex });
    engine.registerAnchor({
      anchorId: sessionId,
      x: widthPx * 0.5,
      y: heightPx * 0.5,
    });
    syncBranchGeometry(activeToolKeys);
  }

  function sync(view: AgentStreamViewModel): void {
    if (disposed) {
      return;
    }
    syncAssistantMessages(view.liveMessages);
    syncToolCalls(view.liveToolCalls);
    syncPendingState(view);
    syncStreamStatus(view);
    syncBreath(view);
  }

  function syncAssistantMessages(messages: LiveAgentMessage[]): void {
    const nextKeys = new Set<string>();
    const activeMessage = selectActiveMessage(messages);

    for (const message of messages) {
      const key = assistantLogicalKey(message.runId, message.turnId);
      nextKeys.add(key);
      const previous = messageProgress.get(key);
      const next: MessageProgress = {
        completed: message.completed,
        textLength: message.text.length,
      };

      if (previous === undefined) {
        emitAssistantStarted();
      }

      const previousLength = previous?.textLength ?? 0;
      const deltaChars = Math.max(0, next.textLength - previousLength);
      if (deltaChars > 0) {
        spawnMessageDelta(deltaChars);
      }

      if ((previous?.completed ?? false) === false && message.completed) {
        engine.triggerAssemblyBloom({ laneId: sessionId, glyph: "ok" });
        onLaneEffect?.({ laneId: sessionId, effect: "sweep" });
      }

      messageProgress.set(key, next);
    }

    const previousMessageKeys = Array.from(messageProgress.keys());
    for (const key of previousMessageKeys) {
      if (!nextKeys.has(key)) {
        messageProgress.delete(key);
      }
    }

    if (activeMessage !== null && activeMessage.text.length === 0) {
      engine.spawnParticle({ laneId: sessionId, intensity: 0.42 });
    }
  }

  function syncToolCalls(toolCalls: LiveAgentToolCall[]): void {
    const nextKeys = toolCalls
      .map((toolCall) => toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId))
      .sort();
    activeToolKeys = nextKeys;
    syncBranchGeometry(nextKeys);
    const nextKeySet = new Set(nextKeys);

    for (const toolCall of toolCalls) {
      const key = toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId);
      const laneId = toToolLaneId(sessionId, key);
      const anchorId = toToolAnchorId(sessionId, key);
      const previous = toolProgress.get(key);
      const next: ToolProgress = {
        outcome: toolCall.outcome,
        outputLength: toolCall.output.length,
      };

      if (previous === undefined) {
        engine.triggerPulseRing({ anchorId, tone: "attention" });
        engine.triggerAssemblyBloom({ laneId, glyph: "tool" });
        engine.spawnParticle({ laneId, intensity: 0.66 });
      }

      const previousLength = previous?.outputLength ?? 0;
      const deltaChars = Math.max(0, next.outputLength - previousLength);
      if (deltaChars > 0) {
        spawnToolProgress(laneId, deltaChars);
      }

      if (previous?.outcome == null && next.outcome != null) {
        if (next.outcome === "failed" || next.outcome === "cancelled") {
          engine.triggerPulseRing({ anchorId, tone: "failed" });
        } else {
          engine.triggerAssemblyBloom({ laneId, glyph: "tool" });
        }
        onLaneEffect?.({ laneId: sessionId, effect: "sweep" });
      }

      toolProgress.set(key, next);
    }

    const previousToolKeys = Array.from(toolProgress.keys());
    for (const key of previousToolKeys) {
      if (!nextKeySet.has(key)) {
        toolProgress.delete(key);
      }
    }
  }

  function syncPendingState(view: AgentStreamViewModel): void {
    const nextPendingState = selectLatestPendingState(view);
    if (nextPendingState === pendingState) {
      return;
    }
    pendingState = nextPendingState;
    if (nextPendingState === "waitingForApproval" || nextPendingState === "waitingForInput") {
      engine.triggerPulseRing({ anchorId: sessionId, tone: "attention" });
    }
  }

  function syncStreamStatus(view: AgentStreamViewModel): void {
    if (view.streamStatus === "error") {
      engine.triggerPulseRing({ anchorId: sessionId, tone: "failed" });
    }
    if (
      view.streamStatus === "connecting" ||
      view.streamStatus === "recoveringFromGap" ||
      view.streamStatus === "reopeningLiveStream"
    ) {
      engine.spawnParticle({ laneId: sessionId, intensity: 0.35 });
    }
  }

  function syncBreath(view: AgentStreamViewModel): void {
    const activeMessage = selectActiveMessage(view.liveMessages);
    const activeToolCount = view.liveToolCalls.filter(
      (toolCall) => toolCall.outcome == null,
    ).length;

    if (activeMessage !== null) {
      const textWeight = Math.min(0.55, activeMessage.text.length / 240);
      engine.setBreath({ hz: 0.95 + textWeight, amplitude: 0.66 + textWeight * 0.25 });
      return;
    }

    if (activeToolCount > 0) {
      const toolWeight = Math.min(0.35, activeToolCount * 0.1);
      engine.setBreath({ hz: 0.9 + toolWeight, amplitude: 0.58 + toolWeight });
      return;
    }

    if (pendingState === "waitingForApproval" || pendingState === "waitingForInput") {
      engine.setBreath({ hz: 0.74, amplitude: 0.48 });
      return;
    }

    if (view.streamStatus === "connecting" || view.streamStatus === "recoveringFromGap") {
      engine.setBreath({ hz: 0.7, amplitude: 0.42 });
      return;
    }

    engine.setBreath({ hz: 0.6, amplitude: 0.35 });
  }

  function emitAssistantStarted(): void {
    engine.triggerPulseRing({ anchorId: sessionId, tone: "attention" });
    engine.spawnParticle({ laneId: sessionId, intensity: 0.72 });
  }

  function spawnMessageDelta(deltaChars: number): void {
    const count = Math.min(6, Math.max(1, Math.ceil(deltaChars / 24)));
    const intensity = Math.min(0.92, 0.42 + deltaChars / 160);
    for (let i = 0; i < count; i += 1) {
      engine.spawnParticle({ laneId: sessionId, intensity });
    }
  }

  function spawnToolProgress(laneId: string, deltaChars: number): void {
    const count = Math.min(5, Math.max(1, Math.ceil(deltaChars / 32)));
    const intensity = Math.min(0.84, 0.38 + deltaChars / 220);
    for (let i = 0; i < count; i += 1) {
      engine.spawnParticle({ laneId, intensity });
    }
  }

  function syncBranchGeometry(toolKeys: string[]): void {
    const nextBranchLaneIds = new Set<string>();
    const nextBranchAnchorIds = new Set<string>();

    for (const [index, key] of toolKeys.entries()) {
      const laneId = toToolLaneId(sessionId, key);
      const anchorId = toToolAnchorId(sessionId, key);
      const slot = branchSlot(index);
      const columnIndex = Math.max(0, mainColumnIndex + slot);
      const x = widthPx * 0.5 + slot * COLUMN_STRIDE_PX;
      const y = heightPx * (0.32 + Math.min(0.18, Math.abs(slot) * 0.04));

      engine.registerLane({ laneId, columnIndex });
      engine.registerAnchor({ anchorId, x, y });
      nextBranchLaneIds.add(laneId);
      nextBranchAnchorIds.add(anchorId);
    }

    for (const laneId of branchLaneIds) {
      if (!nextBranchLaneIds.has(laneId)) {
        engine.unregisterLane({ laneId });
      }
    }

    for (const anchorId of branchAnchorIds) {
      if (!nextBranchAnchorIds.has(anchorId)) {
        engine.unregisterAnchor({ anchorId });
      }
    }

    branchLaneIds.clear();
    branchAnchorIds.clear();
    for (const laneId of nextBranchLaneIds) {
      branchLaneIds.add(laneId);
    }
    for (const anchorId of nextBranchAnchorIds) {
      branchAnchorIds.add(anchorId);
    }
  }

  function dispose(): void {
    if (disposed) {
      return;
    }
    disposed = true;
    for (const laneId of branchLaneIds) {
      engine.unregisterLane({ laneId });
    }
    for (const anchorId of branchAnchorIds) {
      engine.unregisterAnchor({ anchorId });
    }
    engine.unregisterLane({ laneId: sessionId });
    engine.unregisterAnchor({ anchorId: sessionId });
    branchLaneIds.clear();
    branchAnchorIds.clear();
    activeToolKeys = [];
    messageProgress.clear();
    toolProgress.clear();
  }

  resize();

  return {
    dispose,
    resize,
    sync,
  };
}

function selectActiveMessage(messages: LiveAgentMessage[]): LiveAgentMessage | null {
  let activeMessage: LiveAgentMessage | null = null;
  for (const message of messages) {
    if (message.completed) {
      continue;
    }
    if (activeMessage === null || message.lastSequence > activeMessage.lastSequence) {
      activeMessage = message;
    }
  }
  return activeMessage;
}

function selectLatestPendingState(view: AgentStreamViewModel): string | null {
  let pendingState: string | null = null;
  let latestAt = -1n;
  for (const row of view.committedRows) {
    if (row.kind !== "pendingState") {
      continue;
    }
    if (row.occurredAtMs >= latestAt) {
      latestAt = row.occurredAtMs;
      pendingState = row.state;
    }
  }
  return pendingState;
}

function branchSlot(index: number): number {
  const distance = Math.floor(index / 2) + 1;
  return index % 2 === 0 ? -distance : distance;
}

function toToolLaneId(sessionId: SessionId, key: string): string {
  return `${sessionId}:tool:${key}`;
}

function toToolAnchorId(sessionId: SessionId, key: string): string {
  return `${sessionId}:tool-anchor:${key}`;
}
