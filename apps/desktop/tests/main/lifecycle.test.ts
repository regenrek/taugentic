import { describe, expect, it, vi } from "vite-plus/test";

import { DaemonRpcUnavailableError } from "../../packages/main/src/daemon-rpc-connection.js";
import { createDesktopLifecycle } from "../../packages/main/src/lifecycle.js";

describe("createDesktopLifecycle", () => {
  it("ensures the daemon runtime before opening windows and only reopens when needed", async () => {
    const ensureDaemonRuntime = vi.fn(async () => {});
    const registerDesktopIpc = vi.fn();
    const openMainWindow = vi.fn(async () => {});
    const lifecycle = createDesktopLifecycle({
      ensureDaemonRuntime,
      registerDesktopIpc,
      openMainWindow,
    });

    await lifecycle.start();
    await lifecycle.reopenMainWindowIfNeeded(1);
    await lifecycle.reopenMainWindowIfNeeded(0);

    expect(ensureDaemonRuntime).toHaveBeenCalledTimes(1);
    expect(registerDesktopIpc).toHaveBeenCalledTimes(1);
    expect(openMainWindow).toHaveBeenCalledTimes(2);
    expect(ensureDaemonRuntime.mock.invocationCallOrder[0]).toBeLessThan(
      openMainWindow.mock.invocationCallOrder[0],
    );
  });

  it("opens the desktop shell in degraded mode when the daemon is unavailable at startup", async () => {
    const startupError = new DaemonRpcUnavailableError("daemon unavailable");
    const ensureDaemonRuntime = vi.fn(async () => {
      throw startupError;
    });
    const registerDesktopIpc = vi.fn();
    const openMainWindow = vi.fn(async () => {});
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const lifecycle = createDesktopLifecycle({
      ensureDaemonRuntime,
      registerDesktopIpc,
      openMainWindow,
    });

    await expect(lifecycle.start()).resolves.toBeUndefined();

    expect(ensureDaemonRuntime).toHaveBeenCalledTimes(1);
    expect(registerDesktopIpc).toHaveBeenCalledTimes(1);
    expect(openMainWindow).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledWith(
      "daemon unavailable during desktop startup; opening degraded shell",
      startupError,
    );
  });
});
