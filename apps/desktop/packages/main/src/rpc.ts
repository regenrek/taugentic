import { ipcMain } from "electron";

import {
  DESKTOP_INVOKE_METHODS,
  DESKTOP_IPC_SCHEMA,
  type DesktopInvokeHandlers,
  type DesktopInvokeMethod,
} from "@taugentic/desktop-shared";

import { startDaemonViaBootstrap } from "./daemon-bootstrap.js";
import {
  disableDaemonBackgroundModeFromDaemon,
  enableDaemonBackgroundModeFromDaemon,
  readDaemonControlStateFromDaemon,
  reconcileDaemonControlFromDaemon,
  stopDaemonControlFromDaemon,
} from "./daemon-rpc-client.js";
import { desktopAgentRuntimeInvokeHandlers } from "./daemon-agent-runtime.js";
import {
  desktopSessionInvokeHandlers,
  listDaemonNativeRuns,
  listDaemonRuns,
  listDaemonSessions,
} from "./daemon-session.js";

let desktopRpcHandlersRegistered = false;

const desktopInvokeHandlers: DesktopInvokeHandlers = {
  getDaemonStatus: readDaemonControlStateFromDaemon,
  startDaemon: startDaemonViaBootstrap,
  stopDaemon: stopDaemonControlFromDaemon,
  enableBackgroundService: enableDaemonBackgroundModeFromDaemon,
  disableBackgroundService: disableDaemonBackgroundModeFromDaemon,
  reconcileDaemon: reconcileDaemonControlFromDaemon,
  ...desktopAgentRuntimeInvokeHandlers,
  listSessions: listDaemonSessions,
  listRuns: listDaemonRuns,
  listNativeRuns: listDaemonNativeRuns,
  ...desktopSessionInvokeHandlers,
};

function registerDesktopInvokeHandler<Method extends DesktopInvokeMethod>(
  method: Method,
  handler: DesktopInvokeHandlers[Method],
): void {
  const spec = DESKTOP_IPC_SCHEMA[method];
  ipcMain.handle(spec.channel, (_event, ...args) => {
    if (args.length !== spec.argCount) {
      throw new Error(
        `desktop IPC method ${method} expected ${spec.argCount} arg(s), got ${args.length}`,
      );
    }
    return Reflect.apply(handler, undefined, args);
  });
}

export function registerDesktopRpcHandlers(): void {
  if (desktopRpcHandlersRegistered) {
    return;
  }
  desktopRpcHandlersRegistered = true;

  for (const method of DESKTOP_INVOKE_METHODS) {
    registerDesktopInvokeHandler(method, desktopInvokeHandlers[method]);
  }
}
