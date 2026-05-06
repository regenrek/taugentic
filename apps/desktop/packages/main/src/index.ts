import { app, BrowserWindow, dialog } from "electron";
import { fileURLToPath } from "node:url";

import { ensureDesktopDaemonRuntime, stopManagedDaemonOnQuit } from "./daemon-process.js";
import { createDesktopLifecycle } from "./lifecycle.js";
import { registerDesktopRpcHandlers } from "./rpc.js";
import { registerDesktopStreamHandlers } from "./streams.js";
import { createMainWindow } from "./windows.js";

function resolveRendererIndexPath(): string {
  return fileURLToPath(new URL("../../renderer/dist/index.html", import.meta.url));
}

async function openMainWindow(): Promise<void> {
  const mainWindow = createMainWindow();
  const rendererUrl = resolveDevRendererUrl();

  try {
    if (rendererUrl != null) {
      await mainWindow.loadURL(rendererUrl);
      return;
    }

    await mainWindow.loadFile(resolveRendererIndexPath());
  } catch (error) {
    console.error("failed to load desktop renderer", error);
    throw error;
  }
}

function resolveDevRendererUrl(): string | null {
  const rawRendererUrl = process.env.TAUGENTIC_DESKTOP_URL?.trim();
  if (rawRendererUrl == null || rawRendererUrl.length === 0) {
    return null;
  }
  if (app.isPackaged) {
    throw new Error("TAUGENTIC_DESKTOP_URL is not allowed in packaged desktop builds");
  }

  const rendererUrl = new URL(rawRendererUrl);
  const allowedLocalHost =
    rendererUrl.hostname === "127.0.0.1" || rendererUrl.hostname === "localhost";
  if (rendererUrl.protocol !== "http:" || !allowedLocalHost) {
    throw new Error(
      "TAUGENTIC_DESKTOP_URL must target a local http renderer on 127.0.0.1 or localhost",
    );
  }

  return rendererUrl.toString();
}

const desktopLifecycle = createDesktopLifecycle({
  ensureDaemonRuntime: ensureDesktopDaemonRuntime,
  registerDesktopIpc() {
    registerDesktopRpcHandlers();
    registerDesktopStreamHandlers();
  },
  openMainWindow,
});

async function bootstrapDesktopApp(): Promise<void> {
  await app.whenReady();
  await desktopLifecycle.start();

  app.on("activate", () => {
    void desktopLifecycle.reopenMainWindowIfNeeded(BrowserWindow.getAllWindows().length);
  });
}

void bootstrapDesktopApp().catch((error: unknown) => {
  void handleStartupFailure(error);
});

type QuitPhase = "idle" | "stoppingForQuit" | "readyToQuit";

let quitPhase: QuitPhase = "idle";

app.on("before-quit", (event) => {
  if (quitPhase !== "readyToQuit") {
    // Hold Electron in before-quit until the Rust-owned stop path has finished.
    // app.quit() below will emit before-quit again; only the readyToQuit phase
    // should fall through without preventDefault.
    event.preventDefault();
  }

  if (quitPhase !== "idle") {
    return;
  }

  quitPhase = "stoppingForQuit";
  void stopManagedDaemonOnQuit()
    .catch((error: unknown) => {
      console.error("failed to stop managed daemon during shutdown", error);
    })
    .finally(() => {
      quitPhase = "readyToQuit";
      app.quit();
    });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

async function handleStartupFailure(error: unknown): Promise<void> {
  console.error("desktop startup failed", error);

  try {
    dialog.showErrorBox("Taugentic failed to start", describeStartupFailure(error));
  } catch (dialogError) {
    console.error("failed to show startup error dialog", dialogError);
  }

  try {
    await stopManagedDaemonOnQuit();
  } catch (shutdownError) {
    console.error("failed to stop daemon after startup failure", shutdownError);
  }

  app.exit(1);
}

function describeStartupFailure(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
