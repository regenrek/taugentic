import type { DaemonControlSnapshot, DaemonRuntimeMode } from "@taugentic/desktop-shared";

import { startDaemonViaBootstrap } from "./daemon-bootstrap.js";
import { isDaemonRpcUnavailableError } from "./daemon-rpc-connection.js";
import {
  readDaemonControlStateFromDaemon,
  reconcileDaemonControlFromDaemon,
  stopDaemonControlFromDaemon,
} from "./daemon-rpc-client.js";

export async function ensureDesktopDaemonRuntime(): Promise<void> {
  const state = await readDaemonControlStateFromDaemon().catch(async (error: unknown) => {
    if (isDaemonRpcUnavailableError(error)) {
      await startDaemonViaBootstrap();
      return null;
    }
    throw error;
  });
  if (state === null) {
    return;
  }
  if (hasAllowedAction(state, "reconcile")) {
    await reconcileDaemonControlFromDaemon();
    return;
  }
  if (hasAllowedAction(state, "start")) {
    await startDaemonViaBootstrap();
  }
}

export async function stopManagedDaemonOnQuit(): Promise<void> {
  const state = await readDaemonControlStateFromDaemon();
  if (!shouldTerminateManagedDaemonOnQuit(state.desiredMode)) {
    return;
  }
  if (hasAllowedAction(state, "stop")) {
    await stopDaemonControlFromDaemon();
  }
}

export function shouldTerminateManagedDaemonOnQuit(runtimeMode: DaemonRuntimeMode): boolean {
  return runtimeMode === "local";
}

function hasAllowedAction(
  state: DaemonControlSnapshot,
  action: "start" | "stop" | "enableBackground" | "disableBackground" | "reconcile",
): boolean {
  return state.allowedActions.includes(action);
}
