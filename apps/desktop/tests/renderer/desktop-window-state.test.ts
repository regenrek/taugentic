import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { createDesktopWindowState } from "../../packages/shared/src/ipc.js";
import {
  getDesktopWindowSnapshot,
  resetDesktopWindowSnapshotCacheForTests,
  subscribeDesktopWindow,
} from "../../packages/renderer/src/features/window/state.js";

describe("desktop window snapshot cache", () => {
  afterEach(() => {
    resetDesktopWindowSnapshotCacheForTests();
    Reflect.deleteProperty(globalThis, "window");
  });

  it("caches bridged snapshot objects between reads and refreshes them only from subscriptions", () => {
    const subscriptionListener: { current: (() => void) | null } = { current: null };
    const getSnapshot = vi
      .fn<() => ReturnType<typeof createDesktopWindowState>>()
      .mockImplementation(() => createDesktopWindowState("macos"));

    const desktopWindow = {
      close: vi.fn(async () => undefined),
      getSnapshot,
      minimize: vi.fn(async () => createDesktopWindowState("macos")),
      subscribe: vi.fn((listener: () => void) => {
        subscriptionListener.current = listener;
        return () => {
          subscriptionListener.current = null;
        };
      }),
      toggleMaximize: vi.fn(async () => createDesktopWindowState("macos")),
    } satisfies Window["desktopWindow"];

    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: {
        desktopWindow,
      } satisfies Pick<Window, "desktopWindow">,
    });

    const firstSnapshot = getDesktopWindowSnapshot();
    const secondSnapshot = getDesktopWindowSnapshot();

    expect(firstSnapshot).toBe(secondSnapshot);
    expect(getSnapshot).toHaveBeenCalledTimes(1);

    const listener = vi.fn();
    const unsubscribe = subscribeDesktopWindow(listener);

    getSnapshot.mockImplementation(() =>
      createDesktopWindowState("macos", {
        isMaximized: true,
      }),
    );
    const notifySubscription = subscriptionListener.current;
    if (notifySubscription === null) {
      throw new Error("expected desktop window subscription listener");
    }
    notifySubscription();

    const refreshedSnapshot = getDesktopWindowSnapshot();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(refreshedSnapshot).toMatchObject({
      isMaximized: true,
    });
    expect(refreshedSnapshot).not.toBe(firstSnapshot);
    expect(getSnapshot).toHaveBeenCalledTimes(2);

    unsubscribe();
  });
});
