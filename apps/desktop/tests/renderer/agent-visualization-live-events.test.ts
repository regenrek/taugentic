import { describe, expect, it, vi } from "vite-plus/test";

import type { AgentStreamViewModel } from "../../packages/renderer/src/features/agent-stream/index.js";
import { createAgentStreamVisualizationDriver } from "../../packages/renderer/src/features/agent-visualization/agent-stream-visualization.js";
import type { SessionId } from "../../packages/shared/generated/index.js";
import { makeCortexEngineSpy } from "./support/cortex-engine-spy.js";

function makeView(
  sessionId: SessionId,
  overrides: Partial<AgentStreamViewModel> = {},
): AgentStreamViewModel {
  return {
    committedRows: [],
    errorMessage: null,
    hasHydratedCommitted: true,
    liveMessages: [],
    liveToolCalls: [],
    streamStatus: "ready",
    ...overrides,
  };
}

describe("agent stream visualization driver", () => {
  it("reacts immediately to assistantTurnStarted without waiting for a delta", () => {
    const sessionId = "session-visual" as SessionId;
    const engine = makeCortexEngineSpy();

    const driver = createAgentStreamVisualizationDriver({
      engine,
      host: { clientWidth: 320, clientHeight: 160 } as HTMLElement,
      sessionId,
    });

    driver.sync(
      makeView(sessionId, {
        liveMessages: [
          {
            completed: false,
            firstSequence: 1n,
            lastSequence: 1n,
            occurredAtMs: 100n,
            runId: "run-1",
            startedAtMs: 100n,
            text: "",
            turnId: "turn-1",
          },
        ],
      }),
    );

    expect(engine.triggerPulseRing).toHaveBeenCalledWith({
      anchorId: sessionId,
      tone: "attention",
    });
    expect(engine.spawnParticle).toHaveBeenCalledWith({
      laneId: sessionId,
      intensity: 0.72,
    });
    expect(engine.setBreath).toHaveBeenCalledWith({
      amplitude: expect.any(Number),
      hz: expect.any(Number),
    });

    driver.dispose();
  });

  it("turns assistantMessageDelta growth into forward motion on the main lane", () => {
    const sessionId = "session-visual" as SessionId;
    const engine = makeCortexEngineSpy();

    const driver = createAgentStreamVisualizationDriver({
      engine,
      host: { clientWidth: 320, clientHeight: 160 } as HTMLElement,
      sessionId,
    });

    driver.sync(
      makeView(sessionId, {
        liveMessages: [
          {
            completed: false,
            firstSequence: 1n,
            lastSequence: 1n,
            occurredAtMs: 100n,
            runId: "run-1",
            startedAtMs: 100n,
            text: "",
            turnId: "turn-1",
          },
        ],
      }),
    );

    engine.spawnParticle.mockClear();

    driver.sync(
      makeView(sessionId, {
        liveMessages: [
          {
            completed: false,
            firstSequence: 1n,
            lastSequence: 2n,
            occurredAtMs: 120n,
            runId: "run-1",
            startedAtMs: 100n,
            text: "streaming is moving now",
            turnId: "turn-1",
          },
        ],
      }),
    );

    expect(engine.spawnParticle.mock.calls.length).toBeGreaterThan(0);
    expect(engine.spawnParticle.mock.calls.every(([args]) => args.laneId === sessionId)).toBe(true);

    driver.dispose();
  });

  it("maps tool-call progress onto a distinct branch lane and settles it on completion", () => {
    const sessionId = "session-visual" as SessionId;
    const engine = makeCortexEngineSpy();
    const onLaneEffect = vi.fn();

    const driver = createAgentStreamVisualizationDriver({
      engine,
      host: { clientWidth: 320, clientHeight: 160 } as HTMLElement,
      onLaneEffect,
      sessionId,
    });

    driver.sync(
      makeView(sessionId, {
        liveToolCalls: [
          {
            firstSequence: 10n,
            itemId: "item-1",
            lastSequence: 10n,
            occurredAtMs: 200n,
            outcome: null,
            output: "",
            runId: "run-1",
            startedAtMs: 200n,
            toolName: "shell",
            turnId: "turn-1",
          },
        ],
      }),
    );

    const branchLaneId = engine.registerLane.mock.calls
      .map(([args]) => args.laneId)
      .find((laneId) => laneId !== sessionId);
    const branchAnchorId = engine.registerAnchor.mock.calls
      .map(([args]) => args.anchorId)
      .find((anchorId) => anchorId !== sessionId);

    expect(branchLaneId).toBeTruthy();
    expect(engine.triggerPulseRing).toHaveBeenCalledWith({
      anchorId: branchAnchorId,
      tone: "attention",
    });
    expect(engine.triggerAssemblyBloom).toHaveBeenCalledWith({
      glyph: "tool",
      laneId: branchLaneId,
    });

    engine.spawnParticle.mockClear();
    engine.triggerAssemblyBloom.mockClear();

    driver.sync(
      makeView(sessionId, {
        liveToolCalls: [
          {
            firstSequence: 10n,
            itemId: "item-1",
            lastSequence: 11n,
            occurredAtMs: 220n,
            outcome: null,
            output: "ls -la\n./src\n./tests\n",
            runId: "run-1",
            startedAtMs: 200n,
            toolName: "shell",
            turnId: "turn-1",
          },
        ],
      }),
    );

    expect(engine.spawnParticle.mock.calls.some(([args]) => args.laneId === branchLaneId)).toBe(
      true,
    );

    engine.triggerAssemblyBloom.mockClear();

    driver.sync(
      makeView(sessionId, {
        liveToolCalls: [
          {
            firstSequence: 10n,
            itemId: "item-1",
            lastSequence: 12n,
            occurredAtMs: 240n,
            outcome: "completed",
            output: "ls -la\n./src\n./tests\n",
            runId: "run-1",
            startedAtMs: 200n,
            toolName: "shell",
            turnId: "turn-1",
          },
        ],
      }),
    );

    expect(engine.triggerAssemblyBloom).toHaveBeenCalledWith({
      glyph: "tool",
      laneId: branchLaneId,
    });
    expect(onLaneEffect).toHaveBeenCalledWith({
      effect: "sweep",
      laneId: sessionId,
    });

    driver.dispose();
  });

  it("settles the main lane when assistantTurnCompleted lands", () => {
    const sessionId = "session-visual" as SessionId;
    const engine = makeCortexEngineSpy();
    const onLaneEffect = vi.fn();

    const driver = createAgentStreamVisualizationDriver({
      engine,
      host: { clientWidth: 320, clientHeight: 160 } as HTMLElement,
      onLaneEffect,
      sessionId,
    });

    driver.sync(
      makeView(sessionId, {
        liveMessages: [
          {
            completed: false,
            firstSequence: 1n,
            lastSequence: 2n,
            occurredAtMs: 120n,
            runId: "run-1",
            startedAtMs: 100n,
            text: "partial answer",
            turnId: "turn-1",
          },
        ],
      }),
    );

    engine.triggerAssemblyBloom.mockClear();
    onLaneEffect.mockClear();

    driver.sync(
      makeView(sessionId, {
        liveMessages: [
          {
            completed: true,
            firstSequence: 1n,
            lastSequence: 3n,
            occurredAtMs: 140n,
            runId: "run-1",
            startedAtMs: 100n,
            text: "partial answer",
            turnId: "turn-1",
          },
        ],
      }),
    );

    expect(engine.triggerAssemblyBloom).toHaveBeenCalledWith({
      glyph: "ok",
      laneId: sessionId,
    });
    expect(onLaneEffect).toHaveBeenCalledWith({
      effect: "sweep",
      laneId: sessionId,
    });

    driver.dispose();
  });
});
