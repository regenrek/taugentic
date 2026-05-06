import { describe, expect, it, vi } from "vite-plus/test";

import type { SessionId } from "../../packages/shared/generated/index.js";
import type {
  ApprovalStreamMessage,
  ArtifactStreamMessage,
  RunStreamMessage,
} from "../../packages/shared/src/ipc.js";

import { createFocusedSessionSubscriber } from "../../packages/renderer/src/features/streams/index.js";
import type {
  FocusedSessionTransport,
  StreamEvent,
} from "../../packages/renderer/src/features/streams/index.js";

interface FakeSubscription<TMessage> {
  emit(message: TMessage): void;
  closed: boolean;
}

function makeFakeSubscription<TMessage>() {
  let onMessage: ((message: TMessage) => void) | undefined;
  const fake = {
    closed: false,
    emit(message: TMessage) {
      onMessage?.(message);
    },
    subscribe: vi.fn(async (_sessionId: SessionId, nextOnMessage: (message: TMessage) => void) => {
      onMessage = nextOnMessage;
      return () => {
        fake.closed = true;
      };
    }),
  };
  return fake;
}

interface FakeTransport extends FocusedSessionTransport {
  runPort: FakeSubscription<RunStreamMessage>;
  approvalPort: FakeSubscription<ApprovalStreamMessage>;
  artifactPort: FakeSubscription<ArtifactStreamMessage>;
  openCalls: { domain: "run" | "approval" | "artifact"; sessionId: SessionId }[];
}

function makeFakeTransport(): FakeTransport {
  const runPort = makeFakeSubscription<RunStreamMessage>();
  const approvalPort = makeFakeSubscription<ApprovalStreamMessage>();
  const artifactPort = makeFakeSubscription<ArtifactStreamMessage>();
  const openCalls: FakeTransport["openCalls"] = [];

  return {
    runPort,
    approvalPort,
    artifactPort,
    openCalls,
    async subscribeRunStream(sessionId, onMessage) {
      openCalls.push({ domain: "run", sessionId });
      return runPort.subscribe(sessionId, onMessage);
    },
    async subscribeApprovalStream(sessionId, onMessage) {
      openCalls.push({ domain: "approval", sessionId });
      return approvalPort.subscribe(sessionId, onMessage);
    },
    async subscribeArtifactStream(sessionId, onMessage) {
      openCalls.push({ domain: "artifact", sessionId });
      return artifactPort.subscribe(sessionId, onMessage);
    },
  };
}

function runEnvelopeFor(sessionId: SessionId, sequence: bigint): RunStreamMessage {
  return {
    daemonInstanceId: "daemon-fixture",
    sessionId,
    sequence,
    occurredAtMs: 0n,
    event: {
      run: {
        runId: `run-${sequence.toString()}`,
        status: "running",
        detail: "",
      },
    },
  };
}

function approvalEnvelopeFor(sessionId: SessionId, sequence: bigint): ApprovalStreamMessage {
  return {
    daemonInstanceId: "daemon-fixture",
    sessionId,
    sequence,
    occurredAtMs: 0n,
    event: {
      approval: {
        phase: "requested",
        request: {
          expiresAtMs: 60_000n,
          id: `approval-${sequence.toString()}`,
          requestedAtMs: 0n,
          runId: `run-${sequence.toString()}`,
          scope: "processExec",
          target: { kind: "processExec", command: "echo ok" },
          reason: "allow",
        },
      },
    },
  };
}

function artifactEnvelopeFor(sessionId: SessionId, sequence: bigint): ArtifactStreamMessage {
  return {
    daemonInstanceId: "daemon-fixture",
    sessionId,
    sequence,
    occurredAtMs: 0n,
    event: {
      artifact: {
        artifact: {
          id: `artifact-${sequence.toString()}`,
          runId: `run-${sequence.toString()}`,
          kind: "Transcript",
          storagePath: "/tmp/transcript",
        },
      },
    },
  };
}

async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    await Promise.resolve();
  }
}

describe("focused-session subscriber", () => {
  it("forwards real daemon envelopes from every active port", async () => {
    const targetSessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({
      sessionId: targetSessionId,
      transport,
    });

    const handler = vi.fn();
    const unsubscribe = subscriber.subscribe(handler);
    await flushMicrotasks();

    transport.runPort.emit(runEnvelopeFor(targetSessionId, 1n));
    transport.approvalPort.emit(approvalEnvelopeFor(targetSessionId, 2n));
    transport.artifactPort.emit(artifactEnvelopeFor(targetSessionId, 3n));

    const events = handler.mock.calls.map((args) => (args[0] as StreamEvent).event);
    expect("run" in events[0]).toBe(true);
    expect("approval" in events[1]).toBe(true);
    expect("artifact" in events[2]).toBe(true);

    expect(transport.openCalls).toHaveLength(3);
    expect(transport.openCalls.map((call) => call.sessionId)).toEqual([
      targetSessionId,
      targetSessionId,
      targetSessionId,
    ]);

    unsubscribe();
  });

  it("ignores transport status frames", async () => {
    const targetSessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({
      sessionId: targetSessionId,
      transport,
    });

    const handler = vi.fn();
    const unsubscribe = subscriber.subscribe(handler);
    await flushMicrotasks();

    transport.runPort.emit({ stream: "runs", status: "ready" });
    transport.runPort.emit({ stream: "runs", status: "historyGap" });

    expect(handler).not.toHaveBeenCalled();

    transport.runPort.emit(runEnvelopeFor(targetSessionId, 7n));
    expect(handler).toHaveBeenCalledTimes(1);

    unsubscribe();
  });
});
