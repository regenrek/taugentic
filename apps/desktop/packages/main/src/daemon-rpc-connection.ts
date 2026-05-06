import type { Socket } from "node:net";

import type { DaemonEventEnvelope, RunEventStreamItem } from "@taugentic/desktop-shared";
import { METHOD_DAEMON_EVENT, METHOD_DAEMON_RUN_EVENT } from "@taugentic/desktop-shared";
import {
  parseDaemonEventEnvelope,
  parseRunEventStreamItem,
} from "@taugentic/desktop-shared/validation";

const DAEMON_UNAVAILABLE_CODES = new Set(["ECONNREFUSED", "ECONNRESET", "ENOENT", "EPIPE"]);
export const DAEMON_REQUEST_TIMEOUT_MS = 5_000;
const DAEMON_INTERACTIVE_AUTH_REQUEST_TIMEOUT_MS = 30_000;

interface JsonRpcErrorMessage {
  jsonrpc: "2.0";
  id?: number | string | null;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
}

interface PendingRequest {
  resolveResult: (value: unknown) => void;
  reject: (error: unknown) => void;
}

export type DaemonRequestTimeoutPolicy = { kind: "disabled" } | { kind: "deadline"; ms: number };

export const DAEMON_REQUEST_TIMEOUT_DISABLED = {
  kind: "disabled",
} as const satisfies DaemonRequestTimeoutPolicy;

export const DAEMON_REQUEST_TIMEOUT_STANDARD = {
  kind: "deadline",
  ms: DAEMON_REQUEST_TIMEOUT_MS,
} as const satisfies DaemonRequestTimeoutPolicy;

export const DAEMON_REQUEST_TIMEOUT_AGENT_RUNTIME = {
  kind: "deadline",
  ms: 30_000,
} as const satisfies DaemonRequestTimeoutPolicy;

export const DAEMON_REQUEST_TIMEOUT_INTERACTIVE_AUTH = {
  kind: "deadline",
  ms: DAEMON_INTERACTIVE_AUTH_REQUEST_TIMEOUT_MS,
} as const satisfies DaemonRequestTimeoutPolicy;

export class DaemonRpcUnavailableError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "DaemonRpcUnavailableError";
  }
}

export function isDaemonRpcUnavailableError(error: unknown): error is DaemonRpcUnavailableError {
  return (
    error instanceof DaemonRpcUnavailableError ||
    (error instanceof Error && error.name === "DaemonRpcUnavailableError")
  );
}

export class DaemonProtocolError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "DaemonProtocolError";
  }
}

export class DaemonJsonRpcError extends DaemonProtocolError {
  constructor(
    readonly code: number,
    readonly rpcMessage: string,
    readonly data?: unknown,
  ) {
    super(`daemon JSON-RPC error ${code}: ${rpcMessage}`);
    this.name = "DaemonJsonRpcError";
  }
}

export class DaemonRequestTimeoutError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "DaemonRequestTimeoutError";
  }
}

export function isRetryablePersistentSessionError(error: unknown): boolean {
  return isDaemonRpcUnavailableError(error);
}

interface DaemonRpcConnectionOptions {
  initializeConnection: () => Promise<void>;
  onNotification?: (envelope: DaemonEventEnvelope) => void;
  onRunEventNotification?: (item: RunEventStreamItem) => void;
  openSocket: () => { socket: Socket; socketPath: string };
  onTransportTermination?: (error: unknown) => void;
  requestTimeout: DaemonRequestTimeoutPolicy;
}

const jsonRpcWire = {
  isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
  },

  asJsonRpcId(value: unknown): number | string | null | undefined {
    if (
      value === undefined ||
      value === null ||
      typeof value === "number" ||
      typeof value === "string"
    ) {
      return value;
    }

    throw new DaemonProtocolError("daemon returned an invalid JSON-RPC id");
  },

  isJsonRpcErrorObject(value: unknown): value is JsonRpcErrorMessage["error"] {
    return (
      this.isRecord(value) && typeof value.code === "number" && typeof value.message === "string"
    );
  },
};

export class DaemonRpcConnection {
  constructor(private readonly options: DaemonRpcConnectionOptions) {
    assertValidRequestTimeoutPolicy(options.requestTimeout);
  }

  private socket: Socket | null = null;
  private socketPath: string | null = null;
  private buffer = "";
  private connectPromise: Promise<void> | null = null;
  private operationQueue: Promise<void> = Promise.resolve();
  private initialized = false;
  private nextRequestId = 1;
  private pendingRequests = new Map<number, PendingRequest>();
  private timedOutRequestIds = new Set<number>();

  async ensureConnected(): Promise<void> {
    if (this.initialized && this.socket && !this.socket.destroyed) {
      return;
    }

    if (!this.connectPromise) {
      this.connectPromise = this.connectAndInitialize().finally(() => {
        this.connectPromise = null;
      });
    }

    return this.connectPromise;
  }

  enqueueOperation<Result>(operation: () => Promise<Result>): Promise<Result> {
    const queued = this.operationQueue.then(operation);
    this.operationQueue = queued.then(
      () => undefined,
      () => undefined,
    );
    return queued;
  }

  request<Result>(
    method: string,
    params: Record<string, unknown>,
    parseResult: (value: unknown) => Result,
  ): Promise<Result> {
    const socket = this.socket;
    if (!socket || socket.destroyed) {
      return Promise.reject(
        new DaemonRpcUnavailableError("persistent daemon session is not connected"),
      );
    }

    const requestId = this.nextRequestId++;
    const requestLine = `${JSON.stringify({
      jsonrpc: "2.0",
      id: requestId,
      method,
      params,
    })}\n`;

    return new Promise<Result>((resolve, reject) => {
      const pendingRequest = this.createPendingRequestEntry(
        requestId,
        method,
        parseResult,
        resolve,
        reject,
      );
      this.pendingRequests.set(requestId, pendingRequest);
      socket.write(requestLine, (error) => {
        if (!error) {
          return;
        }
        pendingRequest.reject(toDesktopDaemonError(error, this.socketPath ?? "<unknown-socket>"));
      });
    });
  }

  dispose(): void {
    this.initialized = false;
    this.buffer = "";
    this.socketPath = null;
    this.timedOutRequestIds.clear();

    const socket = this.socket;
    this.socket = null;
    if (socket && !socket.destroyed) {
      socket.destroy();
    }
  }

  private async connectAndInitialize(): Promise<void> {
    const connection = this.options.openSocket();
    const socket = connection.socket;
    this.socketPath = connection.socketPath;
    socket.setEncoding("utf8");
    await new Promise<void>((resolve, reject) => {
      const handleConnect = () => {
        socket.off("error", handleConnectError);
        resolve();
      };
      const handleConnectError = (error: Error) => {
        socket.off("connect", handleConnect);
        socket.destroy();
        reject(toDesktopDaemonError(error, connection.socketPath));
      };

      socket.once("connect", handleConnect);
      socket.once("error", handleConnectError);
    });

    this.socket = socket;
    this.buffer = "";
    socket.on("data", (chunk: string) => {
      this.handleSocketData(chunk);
    });
    socket.on("error", (error) => {
      this.handleTransportTermination(toDesktopDaemonError(error, connection.socketPath));
    });
    socket.on("close", () => {
      this.handleTransportTermination(
        new DaemonRpcUnavailableError(
          `daemon closed the persistent session on ${connection.socketPath}`,
        ),
      );
    });

    try {
      await this.options.initializeConnection();
      this.initialized = true;
    } catch (error) {
      this.handleTransportTermination(error);
      throw error;
    }
  }

  private handleSocketData(chunk: string): void {
    this.buffer += chunk;
    while (true) {
      const newlineIndex = this.buffer.indexOf("\n");
      if (newlineIndex === -1) {
        return;
      }

      const line = this.buffer.slice(0, newlineIndex).trim();
      this.buffer = this.buffer.slice(newlineIndex + 1);
      if (!line) {
        this.handleTransportTermination(
          new DaemonProtocolError("daemon returned an empty JSON-RPC response line"),
        );
        return;
      }

      try {
        this.handleJsonRpcLine(line);
      } catch (error) {
        this.handleTransportTermination(error);
        return;
      }
    }
  }

  private handleJsonRpcLine(line: string): void {
    const value: unknown = JSON.parse(line);
    if (!jsonRpcWire.isRecord(value) || value.jsonrpc !== "2.0") {
      throw new DaemonProtocolError("daemon returned a non-JSON-RPC 2.0 message");
    }

    if ("method" in value) {
      this.handleNotificationMessage(value);
      return;
    }

    this.handleResponseMessage(value);
  }

  private handleNotificationMessage(value: Record<string, unknown>): void {
    if (value.method === METHOD_DAEMON_RUN_EVENT) {
      if (!this.initialized) {
        throw new DaemonProtocolError(
          "daemon.run.event arrived before daemon.initialize completed",
        );
      }
      this.options.onRunEventNotification?.(parseRunEventStreamItem(value.params));
      return;
    }

    if (value.method !== METHOD_DAEMON_EVENT) {
      return;
    }

    if (!this.initialized) {
      throw new DaemonProtocolError("daemon.event arrived before daemon.initialize completed");
    }

    const envelope = parseDaemonEventEnvelope(value.params);
    this.options.onNotification?.(envelope);
  }

  private handleResponseMessage(value: Record<string, unknown>): void {
    const id = jsonRpcWire.asJsonRpcId(value.id);
    if (typeof id !== "number") {
      throw new DaemonProtocolError("daemon returned a response with an invalid JSON-RPC id");
    }

    const pending = this.pendingRequests.get(id);
    if (!pending) {
      if (this.timedOutRequestIds.delete(id)) {
        return;
      }
      throw new DaemonProtocolError(`daemon returned an unexpected response id ${id}`);
    }
    this.pendingRequests.delete(id);

    if ("error" in value) {
      if (!jsonRpcWire.isJsonRpcErrorObject(value.error)) {
        throw new DaemonProtocolError("daemon returned an invalid JSON-RPC error object");
      }

      pending.reject(
        new DaemonJsonRpcError(value.error.code, value.error.message, value.error.data),
      );
      return;
    }

    if (!("result" in value)) {
      throw new DaemonProtocolError("daemon returned a JSON-RPC response without a result");
    }

    try {
      pending.resolveResult(value.result);
    } catch (error) {
      pending.reject(error);
    }
  }

  private handleTransportTermination(error: unknown): void {
    this.initialized = false;
    this.buffer = "";
    this.socketPath = null;
    this.timedOutRequestIds.clear();

    const socket = this.socket;
    this.socket = null;
    if (socket && !socket.destroyed) {
      socket.destroy();
    }

    const pendingRequests = [...this.pendingRequests.values()];
    this.pendingRequests.clear();
    for (const pending of pendingRequests) {
      pending.reject(error);
    }

    this.options.onTransportTermination?.(error);
  }

  private createPendingRequestEntry<Result>(
    requestId: number,
    method: string,
    parseResult: (value: unknown) => Result,
    resolve: (value: Result) => void,
    reject: (error: unknown) => void,
  ): PendingRequest {
    let settled = false;
    let timeoutHandle: ReturnType<typeof setTimeout> | null = null;

    const clearPendingRequest = () => {
      this.pendingRequests.delete(requestId);
      if (timeoutHandle !== null) {
        clearTimeout(timeoutHandle);
        timeoutHandle = null;
      }
    };

    const rejectPendingRequest = (error: unknown) => {
      if (settled) {
        return;
      }
      settled = true;
      clearPendingRequest();
      reject(error);
    };

    const resolvePendingRequest = (rawResult: unknown) => {
      if (settled) {
        return;
      }

      let parsedResult: Result;
      try {
        parsedResult = parseResult(rawResult);
      } catch (error) {
        rejectPendingRequest(error);
        return;
      }

      settled = true;
      clearPendingRequest();
      resolve(parsedResult);
    };

    if (this.options.requestTimeout.kind === "deadline") {
      const timeoutMs = this.options.requestTimeout.ms;
      timeoutHandle = setTimeout(() => {
        this.timedOutRequestIds.add(requestId);
        rejectPendingRequest(
          new DaemonRequestTimeoutError(`daemon request ${method} timed out after ${timeoutMs}ms`),
        );
      }, timeoutMs);
    }

    return {
      resolveResult: resolvePendingRequest,
      reject: rejectPendingRequest,
    };
  }
}

function toDesktopDaemonError(error: unknown, socketPath: string): Error {
  if (
    error instanceof DaemonRpcUnavailableError ||
    error instanceof DaemonProtocolError ||
    error instanceof DaemonRequestTimeoutError
  ) {
    return error;
  }

  if (error instanceof Error && hasDaemonUnavailableCode(error)) {
    return new DaemonRpcUnavailableError(
      `failed to reach daemon on ${socketPath}: ${error.message}`,
      {
        cause: error,
      },
    );
  }

  if (error instanceof Error) {
    return new DaemonProtocolError(error.message, { cause: error });
  }

  return new DaemonProtocolError(`unexpected daemon error: ${String(error)}`);
}

function hasDaemonUnavailableCode(error: Error): boolean {
  return (
    "code" in error && typeof error.code === "string" && DAEMON_UNAVAILABLE_CODES.has(error.code)
  );
}

function assertValidRequestTimeoutPolicy(policy: DaemonRequestTimeoutPolicy): void {
  if (policy.kind === "disabled") {
    return;
  }

  if (!Number.isInteger(policy.ms) || policy.ms <= 0) {
    throw new TypeError("daemon request timeout deadline must be a positive integer");
  }
}
