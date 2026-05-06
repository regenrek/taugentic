import { DaemonRpcUnavailableError } from "./daemon-rpc-connection.js";

export interface DesktopLifecycle {
  reopenMainWindowIfNeeded(windowCount: number): Promise<void>;
  start(): Promise<void>;
}

export interface DesktopLifecycleHooks {
  ensureDaemonRuntime(this: void): Promise<void>;
  openMainWindow(this: void): Promise<void>;
  registerDesktopIpc(this: void): void;
}

export function createDesktopLifecycle({
  ensureDaemonRuntime,
  openMainWindow,
  registerDesktopIpc,
}: DesktopLifecycleHooks): DesktopLifecycle {
  let desktopIpcRegistered = false;

  function ensureDesktopIpcRegistered(): void {
    if (desktopIpcRegistered) {
      return;
    }

    registerDesktopIpc();
    desktopIpcRegistered = true;
  }

  return {
    async start() {
      ensureDesktopIpcRegistered();
      try {
        await ensureDaemonRuntime();
      } catch (error) {
        if (!(error instanceof DaemonRpcUnavailableError)) {
          throw error;
        }
        console.error("daemon unavailable during desktop startup; opening degraded shell", error);
      }
      await openMainWindow();
    },
    async reopenMainWindowIfNeeded(windowCount) {
      if (windowCount === 0) {
        await openMainWindow();
      }
    },
  };
}
