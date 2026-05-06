import { BrowserWindow, ipcMain } from "electron";
import { fileURLToPath } from "node:url";

import {
  createDesktopWindowState,
  DESKTOP_WINDOW_CHANNELS,
  resolveDesktopWindowPlatform,
  resolveWindowChromeOptions,
  type DesktopWindowState,
} from "@taugentic/desktop-shared";

const WINDOW_CHROME_BACKGROUND = "#14171b";
const WINDOW_CHROME_SYMBOL = "#ededed";

let desktopWindowHandlersRegistered = false;

function resolvePreloadPath(): string {
  return fileURLToPath(new URL("../../preload/dist/preload.cjs", import.meta.url));
}

function getDesktopWindowState(mainWindow: BrowserWindow): DesktopWindowState {
  const platform = resolveDesktopWindowPlatform(process.platform);
  return createDesktopWindowState(platform, {
    canClose: mainWindow.isClosable(),
    canMaximize: mainWindow.isMaximizable(),
    canMinimize: mainWindow.isMinimizable(),
    isFocused: mainWindow.isFocused(),
    isFullScreen: mainWindow.isFullScreen(),
    isMaximized: mainWindow.isMaximized(),
  });
}

function notifyDesktopWindowState(mainWindow: BrowserWindow): void {
  mainWindow.webContents.send(
    DESKTOP_WINDOW_CHANNELS.stateDidChange,
    getDesktopWindowState(mainWindow),
  );
}

function resolveSenderWindow(sender: Electron.WebContents): BrowserWindow {
  const mainWindow = BrowserWindow.fromWebContents(sender);
  if (mainWindow == null) {
    throw new Error("desktop window is not available for the current sender");
  }
  return mainWindow;
}

function ensureDesktopWindowHandlersRegistered(): void {
  if (desktopWindowHandlersRegistered) {
    return;
  }
  desktopWindowHandlersRegistered = true;

  ipcMain.handle(DESKTOP_WINDOW_CHANNELS.getState, (event) => {
    return getDesktopWindowState(resolveSenderWindow(event.sender));
  });

  ipcMain.handle(DESKTOP_WINDOW_CHANNELS.minimize, (event) => {
    const mainWindow = resolveSenderWindow(event.sender);
    if (mainWindow.isMinimizable()) {
      mainWindow.minimize();
    }
    return getDesktopWindowState(mainWindow);
  });

  ipcMain.handle(DESKTOP_WINDOW_CHANNELS.toggleMaximize, (event) => {
    const mainWindow = resolveSenderWindow(event.sender);
    if (mainWindow.isMaximizable()) {
      if (mainWindow.isMaximized()) {
        mainWindow.unmaximize();
      } else {
        mainWindow.maximize();
      }
    }
    return getDesktopWindowState(mainWindow);
  });

  ipcMain.handle(DESKTOP_WINDOW_CHANNELS.close, (event) => {
    resolveSenderWindow(event.sender).close();
  });
}

export function createMainWindow(): BrowserWindow {
  ensureDesktopWindowHandlersRegistered();

  const chromeOptions = resolveWindowChromeOptions(resolveDesktopWindowPlatform(process.platform), {
    background: WINDOW_CHROME_BACKGROUND,
    symbol: WINDOW_CHROME_SYMBOL,
  });

  const mainWindow = new BrowserWindow({
    width: 1440,
    height: 920,
    minWidth: 1100,
    minHeight: 720,
    backgroundColor: WINDOW_CHROME_BACKGROUND,
    frame: chromeOptions.frame,
    titleBarStyle: chromeOptions.titleBarStyle,
    titleBarOverlay: chromeOptions.titleBarOverlay,
    trafficLightPosition: chromeOptions.trafficLightPosition,
    webPreferences: {
      preload: resolvePreloadPath(),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  mainWindow.webContents.on("will-navigate", (event) => {
    event.preventDefault();
  });
  mainWindow.webContents.on("dom-ready", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  mainWindow.on("blur", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("enter-full-screen", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("focus", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("leave-full-screen", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("maximize", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("restore", () => {
    notifyDesktopWindowState(mainWindow);
  });
  mainWindow.on("unmaximize", () => {
    notifyDesktopWindowState(mainWindow);
  });

  return mainWindow;
}
