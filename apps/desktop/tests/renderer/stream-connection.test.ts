import { describe, expect, it, vi } from "vite-plus/test";

import { connectHydratedStream } from "../../packages/renderer/src/features/streams/connection.js";

type TestState = {
  decodeErrors: number;
  events: number[];
  refreshErrors: string[];
};

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createFakePort() {
  let onDecodeError: (() => void) | undefined;
  let onMessage: ((message: number) => void) | undefined;
  const unsubscribe = vi.fn();
  return {
    emit(message: number) {
      onMessage?.(message);
    },
    failDecode() {
      onDecodeError?.();
    },
    subscribeStream: vi.fn(
      async (
        _afterCursor: number | null,
        nextOnMessage: (message: number) => void,
        nextOnDecodeError: () => void,
      ) => {
        onMessage = nextOnMessage;
        onDecodeError = nextOnDecodeError;
        return unsubscribe;
      },
    ),
    unsubscribe,
  };
}

async function flushMicrotasks(turns = 16): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

describe("shared hydrated stream connection", () => {
  it("closes late-opened ports after dispose", async () => {
    const snapshot = createDeferred<{ cursor: number | null; events: number[] }>();
    const openStream = createDeferred<() => void>();
    const fakePort = createFakePort();
    let state: TestState = {
      decodeErrors: 0,
      events: [],
      refreshErrors: [],
    };

    const dispose = connectHydratedStream<
      TestState,
      number,
      { cursor: number | null; events: number[] },
      number
    >(
      (updater) => {
        state = updater(state);
      },
      {
        getAfterCursor(hydratedSnapshot) {
          return hydratedSnapshot.cursor;
        },
        hydrateSnapshot(current, nextSnapshot) {
          return {
            ...current,
            events: nextSnapshot.events,
          };
        },
        loadSnapshot: () => snapshot.promise,
        onDecodeError(current) {
          return {
            ...current,
            decodeErrors: current.decodeErrors + 1,
          };
        },
        onSnapshotError(current, error) {
          return {
            ...current,
            refreshErrors: [...current.refreshErrors, String(error)],
          };
        },
        subscribeStream: () => openStream.promise,
        reduceMessage(current, message) {
          return {
            needsRefresh: false,
            state: {
              ...current,
              events: [...current.events, message],
            },
          };
        },
      },
    );

    snapshot.resolve({ cursor: 3, events: [1] });
    await flushMicrotasks();

    dispose();
    openStream.resolve(fakePort.unsubscribe);
    await flushMicrotasks();

    expect(fakePort.unsubscribe).toHaveBeenCalledTimes(1);
    expect(state).toEqual({
      decodeErrors: 0,
      events: [1],
      refreshErrors: [],
    });
  });

  it("ignores queued messages and decode errors after dispose", async () => {
    const fakePort = createFakePort();
    const loadSnapshot = vi.fn(async () => ({ cursor: 1, events: [1] }));
    let state: TestState = {
      decodeErrors: 0,
      events: [],
      refreshErrors: [],
    };

    const dispose = connectHydratedStream<
      TestState,
      number,
      { cursor: number | null; events: number[] },
      number
    >(
      (updater) => {
        state = updater(state);
      },
      {
        getAfterCursor(snapshot) {
          return snapshot.cursor;
        },
        hydrateSnapshot(current, nextSnapshot) {
          return {
            ...current,
            events: nextSnapshot.events,
          };
        },
        loadSnapshot,
        onDecodeError(current) {
          return {
            ...current,
            decodeErrors: current.decodeErrors + 1,
          };
        },
        onSnapshotError(current, error) {
          return {
            ...current,
            refreshErrors: [...current.refreshErrors, String(error)],
          };
        },
        subscribeStream: async (_afterCursor, onMessage, onDecodeError) => {
          return fakePort.subscribeStream(_afterCursor, onMessage, onDecodeError);
        },
        reduceMessage(current, message) {
          return {
            needsRefresh: true,
            state: {
              ...current,
              events: [...current.events, message],
            },
          };
        },
      },
    );
    await flushMicrotasks();

    dispose();
    fakePort.emit(99);
    fakePort.failDecode();
    await flushMicrotasks();

    expect(loadSnapshot).toHaveBeenCalledTimes(1);
    expect(state).toEqual({
      decodeErrors: 0,
      events: [1],
      refreshErrors: [],
    });
  });
});
