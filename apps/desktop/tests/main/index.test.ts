import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

type AppEventHandler = (...args: unknown[]) => void;

const hoisted = vi.hoisted(() => {
  const appHandlers = new Map<string, AppEventHandler>();
  const mainWindow = {
    loadFile: vi.fn(async () => {}),
    loadURL: vi.fn(async () => {}),
  };
  let openMainWindow: (() => Promise<void>) | null = null;

  return {
    BrowserWindow: {
      getAllWindows: vi.fn(() => []),
    },
    app: {
      exit: vi.fn(),
      on: vi.fn((eventName: string, handler: AppEventHandler) => {
        appHandlers.set(eventName, handler);
      }),
      quit: vi.fn(),
      isPackaged: false,
      whenReady: vi.fn(() => Promise.resolve()),
    },
    appHandlers,
    mainWindow,
    createMainWindow: vi.fn(() => mainWindow),
    dialog: {
      showErrorBox: vi.fn(),
    },
    ensureDesktopDaemonRuntime: vi.fn(async () => {}),
    lifecycleReopenMainWindowIfNeeded: vi.fn(async () => {}),
    lifecycleStart: vi.fn(async () => {}),
    registerDesktopRpcHandlers: vi.fn(),
    registerDesktopStreamHandlers: vi.fn(),
    stopManagedDaemonOnQuit: vi.fn(async () => {}),
    get openMainWindow() {
      return openMainWindow;
    },
    setOpenMainWindow(handler: (() => Promise<void>) | null) {
      openMainWindow = handler;
    },
  };
});

vi.mock("electron", () => ({
  BrowserWindow: hoisted.BrowserWindow,
  app: hoisted.app,
  dialog: hoisted.dialog,
}));

vi.mock("../../packages/main/src/daemon-process.js", () => ({
  ensureDesktopDaemonRuntime: () => hoisted.ensureDesktopDaemonRuntime(),
  stopManagedDaemonOnQuit: () => hoisted.stopManagedDaemonOnQuit(),
}));

vi.mock("../../packages/main/src/lifecycle.js", () => ({
  createDesktopLifecycle: (config: { openMainWindow: () => Promise<void> }) => {
    hoisted.setOpenMainWindow(config.openMainWindow);
    return {
      reopenMainWindowIfNeeded: hoisted.lifecycleReopenMainWindowIfNeeded,
      start: hoisted.lifecycleStart,
    };
  },
}));

vi.mock("../../packages/main/src/rpc.js", () => ({
  registerDesktopRpcHandlers: hoisted.registerDesktopRpcHandlers,
}));

vi.mock("../../packages/main/src/streams.js", () => ({
  registerDesktopStreamHandlers: hoisted.registerDesktopStreamHandlers,
}));

vi.mock("../../packages/main/src/windows.js", () => ({
  createMainWindow: hoisted.createMainWindow,
}));

describe("desktop main entry quit flow", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.appHandlers.clear();
    hoisted.BrowserWindow.getAllWindows.mockClear();
    hoisted.app.exit.mockClear();
    hoisted.app.on.mockClear();
    hoisted.app.quit.mockClear();
    hoisted.app.whenReady.mockClear();
    hoisted.app.isPackaged = false;
    hoisted.createMainWindow.mockClear();
    hoisted.createMainWindow.mockImplementation(() => hoisted.mainWindow);
    hoisted.dialog.showErrorBox.mockClear();
    hoisted.ensureDesktopDaemonRuntime.mockReset();
    hoisted.ensureDesktopDaemonRuntime.mockResolvedValue(undefined);
    hoisted.lifecycleReopenMainWindowIfNeeded.mockReset();
    hoisted.lifecycleReopenMainWindowIfNeeded.mockResolvedValue(undefined);
    hoisted.lifecycleStart.mockReset();
    hoisted.lifecycleStart.mockResolvedValue(undefined);
    hoisted.registerDesktopRpcHandlers.mockClear();
    hoisted.registerDesktopStreamHandlers.mockClear();
    hoisted.stopManagedDaemonOnQuit.mockReset();
    hoisted.stopManagedDaemonOnQuit.mockResolvedValue(undefined);
    hoisted.mainWindow.loadFile.mockReset();
    hoisted.mainWindow.loadFile.mockResolvedValue(undefined);
    hoisted.mainWindow.loadURL.mockReset();
    hoisted.mainWindow.loadURL.mockResolvedValue(undefined);
    hoisted.setOpenMainWindow(null);
    delete process.env.TAUGENTIC_DESKTOP_URL;
  });

  it("keeps before-quit blocked until daemon shutdown finishes", async () => {
    const stopDeferred = Promise.withResolvers<void>();
    hoisted.stopManagedDaemonOnQuit.mockImplementation(() => stopDeferred.promise);

    await import("../../packages/main/src/index.js");

    const beforeQuit = hoisted.appHandlers.get("before-quit");
    if (!beforeQuit) {
      throw new Error("expected before-quit handler to be registered");
    }

    const firstEvent = { preventDefault: vi.fn() };
    beforeQuit(firstEvent);
    expect(firstEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);

    const secondEvent = { preventDefault: vi.fn() };
    beforeQuit(secondEvent);
    expect(secondEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);

    stopDeferred.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(hoisted.app.quit).toHaveBeenCalledTimes(1);

    const finalEvent = { preventDefault: vi.fn() };
    beforeQuit(finalEvent);
    expect(finalEvent.preventDefault).not.toHaveBeenCalled();
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);
  });

  it("shows a fatal startup error, stops the daemon, and exits when bootstrap fails", async () => {
    const startupError = new Error("startup failed");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    hoisted.lifecycleStart.mockRejectedValueOnce(startupError);

    await import("../../packages/main/src/index.js");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(hoisted.dialog.showErrorBox).toHaveBeenCalledWith(
      "Taugentic failed to start",
      "startup failed",
    );
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);
    expect(hoisted.app.exit).toHaveBeenCalledWith(1);
    expect(consoleError).toHaveBeenCalledWith("desktop startup failed", startupError);
  });

  it("shows a fatal startup error when the packaged renderer file fails to load", async () => {
    const loadError = new Error("renderer load failed");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    hoisted.mainWindow.loadFile.mockRejectedValueOnce(loadError);
    hoisted.lifecycleStart.mockImplementationOnce(async () => {
      await hoisted.openMainWindow?.();
    });

    await import("../../packages/main/src/index.js");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(hoisted.dialog.showErrorBox).toHaveBeenCalledWith(
      "Taugentic failed to start",
      "renderer load failed",
    );
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);
    expect(hoisted.app.exit).toHaveBeenCalledWith(1);
    expect(consoleError).toHaveBeenCalledWith("failed to load desktop renderer", loadError);
    expect(consoleError).toHaveBeenCalledWith("desktop startup failed", loadError);
  });

  it("shows a fatal startup error when the allowed dev renderer url fails to load", async () => {
    const loadError = new Error("renderer url failed");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    process.env.TAUGENTIC_DESKTOP_URL = "http://127.0.0.1:1420";
    hoisted.mainWindow.loadURL.mockRejectedValueOnce(loadError);
    hoisted.lifecycleStart.mockImplementationOnce(async () => {
      await hoisted.openMainWindow?.();
    });

    await import("../../packages/main/src/index.js");
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(hoisted.dialog.showErrorBox).toHaveBeenCalledWith(
      "Taugentic failed to start",
      "renderer url failed",
    );
    expect(hoisted.stopManagedDaemonOnQuit).toHaveBeenCalledTimes(1);
    expect(hoisted.app.exit).toHaveBeenCalledWith(1);
    expect(consoleError).toHaveBeenCalledWith("failed to load desktop renderer", loadError);
    expect(consoleError).toHaveBeenCalledWith("desktop startup failed", loadError);
  });

  it("loads the local renderer file by default", async () => {
    await import("../../packages/main/src/index.js");

    await hoisted.openMainWindow?.();

    expect(hoisted.createMainWindow).toHaveBeenCalledTimes(1);
    expect(hoisted.mainWindow.loadFile).toHaveBeenCalledTimes(1);
    expect(hoisted.mainWindow.loadURL).not.toHaveBeenCalled();
  });

  it("allows only local dev renderer urls in unpackaged mode", async () => {
    process.env.TAUGENTIC_DESKTOP_URL = "http://127.0.0.1:1420";

    await import("../../packages/main/src/index.js");

    await hoisted.openMainWindow?.();

    expect(hoisted.mainWindow.loadURL).toHaveBeenCalledWith("http://127.0.0.1:1420/");
    expect(hoisted.mainWindow.loadFile).not.toHaveBeenCalled();
  });

  it("rejects remote renderer urls even in unpackaged mode", async () => {
    process.env.TAUGENTIC_DESKTOP_URL = "https://evil.example";
    vi.spyOn(console, "error").mockImplementation(() => {});

    await import("../../packages/main/src/index.js");

    await expect(hoisted.openMainWindow?.()).rejects.toThrow(
      "TAUGENTIC_DESKTOP_URL must target a local http renderer on 127.0.0.1 or localhost",
    );
    expect(hoisted.mainWindow.loadURL).not.toHaveBeenCalled();
  });

  it("rejects dev renderer urls in packaged mode", async () => {
    hoisted.app.isPackaged = true;
    process.env.TAUGENTIC_DESKTOP_URL = "http://127.0.0.1:1420";
    vi.spyOn(console, "error").mockImplementation(() => {});

    await import("../../packages/main/src/index.js");

    await expect(hoisted.openMainWindow?.()).rejects.toThrow(
      "TAUGENTIC_DESKTOP_URL is not allowed in packaged desktop builds",
    );
    expect(hoisted.mainWindow.loadURL).not.toHaveBeenCalled();
  });

  it("quits on window-all-closed only outside darwin", async () => {
    const originalPlatform = process.platform;
    await import("../../packages/main/src/index.js");

    const handler = hoisted.appHandlers.get("window-all-closed");
    if (!handler) {
      throw new Error("expected window-all-closed handler to be registered");
    }

    vi.stubGlobal("process", { ...process, platform: "darwin" });
    handler();
    expect(hoisted.app.quit).not.toHaveBeenCalled();

    vi.stubGlobal("process", { ...process, platform: "linux" });
    handler();
    expect(hoisted.app.quit).toHaveBeenCalledTimes(1);

    vi.stubGlobal("process", { ...process, platform: originalPlatform });
  });
});
