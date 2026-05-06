import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type {
  RunEventDelta,
  RunEventStreamItem,
  SubscribeRunEventsResult,
} from "../../packages/shared/src/contracts.js";
import { METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS } from "../../packages/shared/src/contracts.js";

const hoisted = vi.hoisted(() => {
  const instances: MockDaemonSessionConnection[] = [];
  let requestResult: SubscribeRunEventsResult = { events: [], latestEventSeq: null };

  class MockDaemonSessionConnection {
    readonly dispose = vi.fn();
    readonly ensureConnected = vi.fn(async () => {});
    readonly request = vi.fn(async () => requestResult);

    constructor(
      readonly _sessionId: string,
      readonly options: {
        hooks?: {
          onRunEventNotification?: (item: RunEventStreamItem) => void;
          onTransportTermination?: (error: unknown) => void;
        };
      },
    ) {
      instances.push(this);
    }
  }

  return {
    instances,
    MockDaemonSessionConnection,
    setRequestResult(result: SubscribeRunEventsResult) {
      requestResult = result;
    },
  };
});

vi.mock("../../packages/main/src/daemon-session-connection.js", () => ({
  DaemonSessionConnection: hoisted.MockDaemonSessionConnection,
}));

function port() {
  const closeHandlers: Array<() => void> = [];
  return {
    close: vi.fn(() => {
      for (const handler of closeHandlers) {
        handler();
      }
    }),
    once: vi.fn((event: "close", handler: () => void) => {
      if (event === "close") {
        closeHandlers.push(handler);
      }
    }),
    postMessage: vi.fn(),
  };
}

function delta(seq: bigint): RunEventDelta {
  return {
    event: {
      run: {
        detail: `event ${seq.toString()}`,
        runId: "run-1",
        status: "running",
      },
    },
    seq,
  };
}

describe("RunEventStreamConnection", () => {
  beforeEach(() => {
    hoisted.instances.length = 0;
    hoisted.setRequestResult({ events: [], latestEventSeq: null });
  });

  it("opens a per-run subscription and posts replay before live items", async () => {
    hoisted.setRequestResult({ events: [delta(2n)], latestEventSeq: 2n });
    const onClosed = vi.fn();
    const targetPort = port();
    const { RunEventStreamConnection } =
      await import("../../packages/main/src/run-event-stream-connection.js");

    const stream = new RunEventStreamConnection(
      "session-1",
      "run-1",
      targetPort as never,
      1n,
      onClosed,
    );
    await stream.open();

    const connection = hoisted.instances[0]!;
    expect(connection.ensureConnected).toHaveBeenCalledTimes(1);
    expect(connection.request).toHaveBeenCalledWith(
      METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
      { sessionId: "session-1", runId: "run-1", afterSeq: "1" },
      expect.any(Function),
    );
    expect(targetPort.postMessage).toHaveBeenNthCalledWith(1, {
      latestEventSeq: 2n,
      stream: "runEvents",
      status: "ready",
    });
    expect(targetPort.postMessage).toHaveBeenNthCalledWith(2, {
      runId: "run-1",
      payload: { kind: "delta", delta: delta(2n) },
    });

    connection.options.hooks?.onRunEventNotification?.({
      runId: "run-1",
      payload: { kind: "delta", delta: delta(3n) },
    });
    expect(targetPort.postMessage).toHaveBeenNthCalledWith(3, {
      runId: "run-1",
      payload: { kind: "delta", delta: delta(3n) },
    });
  });

  it("disposes the daemon connection after terminal run stream errors", async () => {
    const onClosed = vi.fn();
    const targetPort = port();
    const { RunEventStreamConnection } =
      await import("../../packages/main/src/run-event-stream-connection.js");
    const stream = new RunEventStreamConnection(
      "session-1",
      "run-1",
      targetPort as never,
      null,
      onClosed,
    );
    await stream.open();
    const connection = hoisted.instances[0]!;

    connection.options.hooks?.onRunEventNotification?.({
      runId: "run-1",
      payload: { kind: "error", error: "lagged" },
    });

    expect(targetPort.postMessage).toHaveBeenLastCalledWith({
      runId: "run-1",
      payload: { kind: "error", error: "lagged" },
    });
    expect(connection.dispose).toHaveBeenCalledTimes(1);
    expect(onClosed).toHaveBeenCalledTimes(1);
  });
});
