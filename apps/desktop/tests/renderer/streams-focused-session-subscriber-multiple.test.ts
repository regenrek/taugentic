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
  openRunCalls: number;
  openApprovalCalls: number;
  openArtifactCalls: number;
}

function makeFakeTransport(): FakeTransport {
  const runPort = makeFakeSubscription<RunStreamMessage>();
  const approvalPort = makeFakeSubscription<ApprovalStreamMessage>();
  const artifactPort = makeFakeSubscription<ArtifactStreamMessage>();

  const transport: FakeTransport = {
    runPort,
    approvalPort,
    artifactPort,
    openRunCalls: 0,
    openApprovalCalls: 0,
    openArtifactCalls: 0,
    async subscribeRunStream(sessionId, onMessage) {
      transport.openRunCalls += 1;
      return runPort.subscribe(sessionId, onMessage);
    },
    async subscribeApprovalStream(sessionId, onMessage) {
      transport.openApprovalCalls += 1;
      return approvalPort.subscribe(sessionId, onMessage);
    },
    async subscribeArtifactStream(sessionId, onMessage) {
      transport.openArtifactCalls += 1;
      return artifactPort.subscribe(sessionId, onMessage);
    },
  };
  return transport;
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

describe("focused-session subscriber: multiple concurrent subscribers", () => {
  it("fans every event to every active handler from a single underlying transport", async () => {
    const sessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({ sessionId, transport });

    const handlerA = vi.fn();
    const handlerB = vi.fn();
    const unsubA = subscriber.subscribe(handlerA);
    const unsubB = subscriber.subscribe(handlerB);
    await flushMicrotasks();

    transport.runPort.emit(runEnvelopeFor(sessionId, 1n));
    transport.approvalPort.emit(approvalEnvelopeFor(sessionId, 2n));
    transport.artifactPort.emit(artifactEnvelopeFor(sessionId, 3n));

    expect(handlerA).toHaveBeenCalledTimes(3);
    expect(handlerB).toHaveBeenCalledTimes(3);

    expect(transport.openRunCalls).toBe(1);
    expect(transport.openApprovalCalls).toBe(1);
    expect(transport.openArtifactCalls).toBe(1);

    unsubA();
    unsubB();
  });

  it("disposing one subscriber leaves the other receiving events; closes ports only when last leaves", async () => {
    const sessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({ sessionId, transport });

    const handlerA = vi.fn();
    const handlerB = vi.fn();
    const unsubA = subscriber.subscribe(handlerA);
    const unsubB = subscriber.subscribe(handlerB);
    await flushMicrotasks();

    unsubA();

    expect(transport.runPort.closed).toBe(false);
    expect(transport.approvalPort.closed).toBe(false);
    expect(transport.artifactPort.closed).toBe(false);

    transport.runPort.emit(runEnvelopeFor(sessionId, 10n));
    expect(handlerA).not.toHaveBeenCalled();
    expect(handlerB).toHaveBeenCalledTimes(1);
    expect((handlerB.mock.calls[0]![0] as StreamEvent).sequence).toBe(10n);

    unsubB();
    expect(transport.runPort.closed).toBe(true);
    expect(transport.approvalPort.closed).toBe(true);
    expect(transport.artifactPort.closed).toBe(true);
  });
});
