import { createConnection } from "node:net";

import type { DaemonControlStatusResult } from "@taugentic/desktop-shared";

import {
  disableDaemonBackgroundModeViaBootstrap,
  enableDaemonBackgroundModeViaBootstrap,
  reconcileDaemonViaBootstrap,
  snapshotDaemonViaBootstrap,
  stopDaemonViaBootstrap,
} from "./daemon-bootstrap.js";
import { createDesktopDaemonLocatorConfig } from "./desktop-locator-config.js";

function openDaemonRpcSocket() {
  const connection = createDesktopDaemonLocatorConfig();
  return {
    socket: createConnection(connection.socketPath),
    socketPath: connection.socketPath,
  };
}

export { openDaemonRpcSocket };

export async function readDaemonControlStateFromDaemon(): Promise<DaemonControlStatusResult> {
  return await snapshotDaemonViaBootstrap();
}

export async function disableDaemonBackgroundModeFromDaemon(): Promise<DaemonControlStatusResult> {
  return await disableDaemonBackgroundModeViaBootstrap();
}

export async function enableDaemonBackgroundModeFromDaemon(): Promise<DaemonControlStatusResult> {
  return await enableDaemonBackgroundModeViaBootstrap();
}

export async function reconcileDaemonControlFromDaemon(): Promise<DaemonControlStatusResult> {
  return await reconcileDaemonViaBootstrap();
}

export async function stopDaemonControlFromDaemon(): Promise<DaemonControlStatusResult> {
  return await stopDaemonViaBootstrap();
}
