import { describe, expect, it, vi } from "vite-plus/test";

import type { SessionId } from "../../packages/shared/generated/index.js";
import type {
  ApprovalStreamMessage,
  ArtifactStreamMessage,
  RunStreamMessage,
} from "../../packages/shared/src/ipc.js";

import { createFocusedSessionSubscriber } from "../../packages/renderer/src/features/streams/index.js";
import type { FocusedSessionTransport } from "../../packages/renderer/src/features/streams/index.js";

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
}

function makeFakeTransport(): FakeTransport {
  const runPort = makeFakeSubscription<RunStreamMessage>();
  const approvalPort = makeFakeSubscription<ApprovalStreamMessage>();
  const artifactPort = makeFakeSubscription<ArtifactStreamMessage>();

  return {
    runPort,
    approvalPort,
    artifactPort,
    async subscribeRunStream(sessionId, onMessage) {
      return runPort.subscribe(sessionId, onMessage);
    },
    async subscribeApprovalStream(sessionId, onMessage) {
      return approvalPort.subscribe(sessionId, onMessage);
    },
    async subscribeArtifactStream(sessionId, onMessage) {
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

describe("focused-session subscriber: dispose semantics", () => {
  it("never invokes the handler after unsubscribe", async () => {
    const sessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({ sessionId, transport });

    const handler = vi.fn();
    const unsubscribe = subscriber.subscribe(handler);
    await flushMicrotasks();

    transport.runPort.emit(runEnvelopeFor(sessionId, 1n));
    expect(handler).toHaveBeenCalledTimes(1);

    unsubscribe();

    transport.runPort.emit(runEnvelopeFor(sessionId, 2n));
    transport.approvalPort.emit(approvalEnvelopeFor(sessionId, 3n));
    transport.artifactPort.emit(artifactEnvelopeFor(sessionId, 4n));

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("unsubscribe is idempotent", async () => {
    const sessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({ sessionId, transport });

    const handler = vi.fn();
    const unsubscribe = subscriber.subscribe(handler);
    await flushMicrotasks();

    expect(() => {
      unsubscribe();
      unsubscribe();
      unsubscribe();
    }).not.toThrow();

    expect(transport.runPort.closed).toBe(true);
    expect(transport.approvalPort.closed).toBe(true);
    expect(transport.artifactPort.closed).toBe(true);

    transport.runPort.emit(runEnvelopeFor(sessionId, 99n));
    expect(handler).not.toHaveBeenCalled();
  });

  it("subscribe after full disposal yields a noop unsubscribe and never reopens transport", async () => {
    const sessionId = "session-S1" as SessionId;
    const transport = makeFakeTransport();
    const subscriber = createFocusedSessionSubscriber({ sessionId, transport });

    const handlerA = vi.fn();
    const unsubA = subscriber.subscribe(handlerA);
    await flushMicrotasks();
    unsubA();

    const handlerB = vi.fn();
    const unsubB = subscriber.subscribe(handlerB);

    transport.runPort.emit(runEnvelopeFor(sessionId, 5n));
    expect(handlerB).not.toHaveBeenCalled();

    expect(() => unsubB()).not.toThrow();
  });
});
