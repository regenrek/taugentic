import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { DESKTOP_WINDOW_CHANNELS } from "../../packages/shared/src/ipc.js";

type IpcHandle = (event: { sender: unknown }) => unknown;
type WindowHandler = () => void;
type WebContentsHandler = (...args: unknown[]) => void;

const hoisted = vi.hoisted(() => {
  const ipcHandles = new Map<string, IpcHandle>();
  const webContentsHandlers = new Map<string, WebContentsHandler>();
  const windowHandlers = new Map<string, WindowHandler>();
  const webContents = {
    on: vi.fn((eventName: string, handler: WebContentsHandler) => {
      webContentsHandlers.set(eventName, handler);
    }),
    send: vi.fn(),
    setWindowOpenHandler: vi.fn(),
  };
  const browserWindowInstance = {
    close: vi.fn(),
    isClosable: vi.fn(() => true),
    isFocused: vi.fn(() => true),
    isFullScreen: vi.fn(() => false),
    isMaximizable: vi.fn(() => true),
    isMaximized: vi.fn(() => false),
    isMinimizable: vi.fn(() => true),
    maximize: vi.fn(),
    minimize: vi.fn(),
    on: vi.fn((eventName: string, handler: WindowHandler) => {
      windowHandlers.set(eventName, handler);
    }),
    unmaximize: vi.fn(),
    webContents,
  };
  const BrowserWindow = Object.assign(
    vi.fn(function BrowserWindowMock() {
      return browserWindowInstance;
    }),
    {
      fromWebContents: vi.fn(() => browserWindowInstance),
    },
  );

  return {
    browserWindowInstance,
    BrowserWindow,
    ipcHandles,
    ipcMain: {
      handle: vi.fn((channel: string, handler: IpcHandle) => {
        ipcHandles.set(channel, handler);
      }),
    },
    webContents,
    webContentsHandlers,
    windowHandlers,
  };
});

vi.mock("electron", () => ({
  BrowserWindow: hoisted.BrowserWindow,
  ipcMain: hoisted.ipcMain,
}));

describe("createMainWindow", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.ipcHandles.clear();
    hoisted.webContentsHandlers.clear();
    hoisted.windowHandlers.clear();
    hoisted.BrowserWindow.mockClear();
    hoisted.BrowserWindow.fromWebContents.mockClear();
    hoisted.ipcMain.handle.mockClear();
    hoisted.browserWindowInstance.close.mockClear();
    hoisted.browserWindowInstance.isClosable.mockReset();
    hoisted.browserWindowInstance.isClosable.mockReturnValue(true);
    hoisted.browserWindowInstance.isFocused.mockReset();
    hoisted.browserWindowInstance.isFocused.mockReturnValue(true);
    hoisted.browserWindowInstance.isFullScreen.mockReset();
    hoisted.browserWindowInstance.isFullScreen.mockReturnValue(false);
    hoisted.browserWindowInstance.isMaximizable.mockReset();
    hoisted.browserWindowInstance.isMaximizable.mockReturnValue(true);
    hoisted.browserWindowInstance.isMaximized.mockReset();
    hoisted.browserWindowInstance.isMaximized.mockReturnValue(false);
    hoisted.browserWindowInstance.isMinimizable.mockReset();
    hoisted.browserWindowInstance.isMinimizable.mockReturnValue(true);
    hoisted.browserWindowInstance.maximize.mockClear();
    hoisted.browserWindowInstance.minimize.mockClear();
    hoisted.browserWindowInstance.on.mockClear();
    hoisted.browserWindowInstance.unmaximize.mockClear();
    hoisted.webContents.on.mockClear();
    hoisted.webContents.send.mockClear();
    hoisted.webContents.setWindowOpenHandler.mockClear();
  });

  it("creates the main window with platform-correct chrome and denies renderer navigation and popup creation", async () => {
    const { createMainWindow } = await import("../../packages/main/src/windows.js");
    const { resolveWindowChromeOptions, resolveDesktopWindowPlatform } =
      await import("../../packages/shared/src/ipc.js");

    const mainWindow = createMainWindow();
    const expectedChrome = resolveWindowChromeOptions(
      resolveDesktopWindowPlatform(process.platform),
      { background: "#14171b", symbol: "#ededed" },
    );

    expect(mainWindow).toBe(hoisted.browserWindowInstance);
    expect(hoisted.BrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({
        frame: expectedChrome.frame,
        titleBarStyle: expectedChrome.titleBarStyle,
        minHeight: 720,
        minWidth: 1100,
      }),
    );
    expect(hoisted.webContents.on).toHaveBeenCalledWith("will-navigate", expect.any(Function));
    expect(hoisted.webContents.on).toHaveBeenCalledWith("dom-ready", expect.any(Function));
    expect(hoisted.webContents.setWindowOpenHandler).toHaveBeenCalledWith(expect.any(Function));

    const willNavigateHandler = hoisted.webContentsHandlers.get("will-navigate");
    const event = { preventDefault: vi.fn() };
    willNavigateHandler?.(event);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);

    const windowOpenHandler = hoisted.webContents.setWindowOpenHandler.mock.calls[0]?.[0];
    expect(windowOpenHandler()).toEqual({ action: "deny" });
  });

  it("registers window-control handlers once and routes actions to the sender window", async () => {
    const { createMainWindow } = await import("../../packages/main/src/windows.js");

    createMainWindow();
    createMainWindow();

    expect(hoisted.ipcMain.handle).toHaveBeenCalledTimes(4);

    const senderEvent = { sender: { id: "sender" } };
    const getState = hoisted.ipcHandles.get(DESKTOP_WINDOW_CHANNELS.getState);
    const minimize = hoisted.ipcHandles.get(DESKTOP_WINDOW_CHANNELS.minimize);
    const toggleMaximize = hoisted.ipcHandles.get(DESKTOP_WINDOW_CHANNELS.toggleMaximize);
    const close = hoisted.ipcHandles.get(DESKTOP_WINDOW_CHANNELS.close);

    expect(getState?.(senderEvent)).toEqual(
      expect.objectContaining({
        canClose: true,
        canMaximize: true,
        canMinimize: true,
        isMaximized: false,
      }),
    );

    await minimize?.(senderEvent);
    expect(hoisted.BrowserWindow.fromWebContents).toHaveBeenCalledWith(senderEvent.sender);
    expect(hoisted.browserWindowInstance.minimize).toHaveBeenCalledTimes(1);

    hoisted.browserWindowInstance.isMaximized.mockReturnValueOnce(false);
    await toggleMaximize?.(senderEvent);
    expect(hoisted.browserWindowInstance.maximize).toHaveBeenCalledTimes(1);

    hoisted.browserWindowInstance.isMaximized.mockReturnValueOnce(true);
    await toggleMaximize?.(senderEvent);
    expect(hoisted.browserWindowInstance.unmaximize).toHaveBeenCalledTimes(1);

    await close?.(senderEvent);
    expect(hoisted.browserWindowInstance.close).toHaveBeenCalledTimes(1);
  });

  it("broadcasts native window state changes into the renderer", async () => {
    const { createMainWindow } = await import("../../packages/main/src/windows.js");

    createMainWindow();

    hoisted.webContentsHandlers.get("dom-ready")?.();
    expect(hoisted.webContents.send).toHaveBeenCalledWith(
      DESKTOP_WINDOW_CHANNELS.stateDidChange,
      expect.objectContaining({
        canClose: true,
        isFocused: true,
        isMaximized: false,
      }),
    );

    hoisted.browserWindowInstance.isMaximized.mockReturnValue(true);
    hoisted.windowHandlers.get("maximize")?.();

    expect(hoisted.webContents.send).toHaveBeenLastCalledWith(
      DESKTOP_WINDOW_CHANNELS.stateDidChange,
      expect.objectContaining({
        isMaximized: true,
      }),
    );
  });
});
