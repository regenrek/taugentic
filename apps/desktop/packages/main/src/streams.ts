import {
  createDesktopStreamOpenErrorResponse,
  createDesktopStreamOpenSuccessResponse,
  DESKTOP_IPC_SCHEMA,
  DESKTOP_STREAM_METHODS,
  getDesktopStreamResponseChannel,
  parseDesktopStreamOpenRequest,
} from "@taugentic/desktop-shared";
import {
  parseNullableActivityCursor,
  parseNullableDaemonEventCursor,
  parseRunId,
  parseSessionId,
} from "@taugentic/desktop-shared/validation";

import { desktopSessionStreamHandlers } from "./daemon-session.js";
import { MessageChannelMain, ipcMain } from "./electron.js";

let desktopStreamHandlersRegistered = false;

function postStreamOpenError(
  event: Electron.IpcMainEvent,
  responseChannel: string,
  error: unknown,
): void {
  event.sender.postMessage(
    responseChannel,
    createDesktopStreamOpenErrorResponse(
      error instanceof Error ? error.message : `failed to attach stream: ${String(error)}`,
    ),
  );
}

export function registerDesktopStreamHandlers(): void {
  if (desktopStreamHandlersRegistered) {
    return;
  }
  desktopStreamHandlersRegistered = true;

  for (const method of DESKTOP_STREAM_METHODS) {
    const spec = DESKTOP_IPC_SCHEMA[method];
    ipcMain.on(spec.requestChannel, (event, ...args: unknown[]) => {
      if (args.length !== 1) {
        throw new Error(
          `desktop stream request ${method} expected 1 payload arg, got ${args.length}`,
        );
      }
      const request = parseDesktopStreamOpenRequest(method, args[0]);
      const [rawSessionId] = request.args;
      const sessionId = parseSessionId(rawSessionId);
      const responseChannel = getDesktopStreamResponseChannel(spec, request.requestId);
      const { port1, port2 } = new MessageChannelMain();
      const attachPort = (() => {
        if (method === "openRunStream") {
          return desktopSessionStreamHandlers.openRunStream(
            sessionId,
            port1,
            parseNullableActivityCursor(
              request.args[1],
              "desktop stream request openRunStream afterCursor",
            ),
          );
        }
        if (method === "openRunEventStream") {
          return desktopSessionStreamHandlers.openRunEventStream(
            sessionId,
            parseRunId(request.args[1]),
            port1,
            parseNullableRunEventSeq(request.args[2]),
          );
        }
        if (method === "openApprovalStream") {
          return desktopSessionStreamHandlers.openApprovalStream(
            sessionId,
            port1,
            parseNullableDaemonEventCursor(
              request.args[1],
              "desktop stream request openApprovalStream afterCursor",
            ),
          );
        }
        if (method === "openArtifactStream") {
          return desktopSessionStreamHandlers.openArtifactStream(
            sessionId,
            port1,
            parseNullableDaemonEventCursor(
              request.args[1],
              "desktop stream request openArtifactStream afterCursor",
            ),
          );
        }
        return desktopSessionStreamHandlers.openAgentStream(
          sessionId,
          port1,
          parseNullableDaemonEventCursor(
            request.args[1],
            "desktop stream request openAgentStream afterCursor",
          ),
        );
      })();
      void attachPort
        .then(() => {
          event.sender.postMessage(responseChannel, createDesktopStreamOpenSuccessResponse(), [
            port2,
          ]);
        })
        .catch((error: unknown) => {
          port1.close();
          port2.close?.();
          postStreamOpenError(event, responseChannel, error);
          console.error(`failed to attach ${method} to daemon session`, error);
        });
    });
  }
}

function parseNullableRunEventSeq(value: unknown): bigint | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value !== "bigint" || value < 0n) {
    throw new Error("desktop stream request openRunEventStream afterSeq must be a uint64 bigint");
  }
  return value;
}
