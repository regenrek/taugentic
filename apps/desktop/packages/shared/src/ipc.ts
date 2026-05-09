import type {
  AgentRuntimeSnapshot,
  AgentTurnsPageQuery,
  AgentTurnsPageResult,
  ActivityCursor,
  ActivityPageQuery,
  ActivityPageResult,
  ApprovalDecision,
  ApprovalId,
  ApprovalSnapshotResult,
  ArtifactId,
  ArtifactKind,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonDiagnostics,
  DaemonWorkspaceOpenParams,
  DaemonWorkspaceOpenResult,
  SessionOverviewQuery,
  SessionOverviewResult,
  ArtifactSnapshotResult,
  DaemonControlStatusResult,
  DaemonEventEnvelope,
  DaemonEventCursor,
  ForkRunRequest,
  ForkRunResult,
  ListApprovalsQuery,
  ListArtifactsQuery,
  ListNativeRunsRequest,
  ListNativeRunsResult,
  RecipeListResponse,
  RunDetail,
  RunEventStreamItem,
  RunId,
  RunTimeline,
  SessionId,
  RunSummary,
  SessionSummary,
  WorkspaceSelector,
  StartRunCommand,
  SubscribeRunEventsResult,
  WorkItemDismissParams,
  WorkItemDismissResult,
  WorkItemListResult,
  WorkItemRefreshParams,
  WorkItemTriggerParams,
  WorkItemTriggerResult,
  WorkflowLoadParams,
  WorkflowStatusResult,
  WorkflowValidateParams,
  WorkflowValidationReport,
} from "./contracts.js";

/**
 * Default inline-read cap for artifact content (2 MiB).
 *
 * Anything larger must flow through `saveArtifactAs` (download-only) so the
 * renderer never tries to inline-render a huge log.
 */
export const DEFAULT_READ_ARTIFACT_MAX_BYTES = 2 * 1024 * 1024;

/** Upper bound that main-side handlers enforce to keep IPC payloads bounded. */
export const MAX_READ_ARTIFACT_MAX_BYTES = 16 * 1024 * 1024;

export interface ReadArtifactContentQuery {
  readonly sessionId: SessionId;
  readonly artifactId: ArtifactId;
  /** If omitted, defaults to {@link DEFAULT_READ_ARTIFACT_MAX_BYTES}. */
  readonly maxBytes?: number;
}

export type ReadArtifactContentResult =
  | {
      readonly status: "inline";
      readonly kind: ArtifactKind;
      readonly storagePath: string;
      readonly totalBytes: number;
      readonly readBytes: number;
      readonly truncated: boolean;
      readonly encoding: "utf-8";
      readonly content: string;
    }
  | {
      readonly status: "tooLarge";
      readonly kind: ArtifactKind;
      readonly storagePath: string;
      readonly totalBytes: number;
      readonly limitBytes: number;
    }
  | {
      readonly status: "missing";
      readonly reason: "artifactNotFound" | "fileNotFound";
    };

export interface SaveArtifactAsQuery {
  readonly sessionId: SessionId;
  readonly artifactId: ArtifactId;
  /** Filename suggested in the native Save dialog (no directory components). */
  readonly suggestedFilename?: string;
}

export type SaveArtifactAsResult =
  | {
      readonly status: "saved";
      readonly savedPath: string;
      readonly bytesCopied: number;
    }
  | {
      readonly status: "cancelled";
    }
  | {
      readonly status: "missing";
      readonly reason: "artifactNotFound" | "fileNotFound";
    };

export type DaemonControlSnapshot = DaemonControlStatusResult;

export type RunEventStreamStatus = {
  latestEventSeq?: bigint | null;
  stream: "runEvents";
  status: "ready" | "historyGap" | "terminalError";
};

export type RunEventStreamMessage = RunEventStreamStatus | RunEventStreamItem;

interface DesktopInvokeChannel<Args extends unknown[] = [], Result = unknown> {
  readonly kind: "invoke";
  readonly channel: string;
  readonly argCount: number;
  readonly __args?: Args;
  readonly __result?: Result;
}

interface DesktopStreamChannel<Args extends unknown[] = [], Message = unknown> {
  readonly kind: "stream";
  readonly requestChannel: string;
  readonly responseChannelPrefix: string;
  readonly argCount: number;
  readonly __args?: Args;
  readonly __message?: Message;
}

export interface DesktopStreamPort<Message = unknown> {
  onmessage: ((event: MessageEvent<Message>) => void) | null;
  onmessageerror: ((event: MessageEvent<Message>) => void) | null;
  start?: () => void;
  close?: () => void;
}

export type DesktopStreamUnsubscribe = () => void;
export type DesktopStreamListener<Message> = (message: Message) => void;
export type DesktopStreamErrorListener = (error: Error) => void;

type DesktopIpcChannel = DesktopInvokeChannel<any, any> | DesktopStreamChannel<any>;
type DesktopIpcSchema = Record<string, DesktopIpcChannel>;

type DesktopApiFromSchema<Schema extends DesktopIpcSchema> = {
  [Method in keyof Schema]: Schema[Method] extends DesktopInvokeChannel<infer Args, infer Result>
    ? (...args: Args) => Promise<Result>
    : Schema[Method] extends DesktopStreamChannel<infer Args, infer Message>
      ? (...args: Args) => Promise<DesktopStreamPort<Message>>
      : never;
};

type KeysMatching<Schema, Condition> = {
  [Key in keyof Schema]: Schema[Key] extends Condition ? Key : never;
}[keyof Schema];

export interface DesktopStreamOpenRequest<Args extends unknown[] = unknown[]> {
  readonly args: Args;
  readonly requestId: string;
}

export type DesktopStreamOpenResponse =
  | {
      readonly status: "ok";
    }
  | {
      readonly message: string;
      readonly status: "error";
    };

function invokeChannel<Args extends unknown[], Result>(
  channel: string,
  argCount: number,
): DesktopInvokeChannel<Args, Result> {
  return {
    kind: "invoke",
    channel,
    argCount,
  };
}

function streamChannel<Args extends unknown[], Message>(
  requestChannel: string,
  responseChannelPrefix: string,
  argCount: number,
): DesktopStreamChannel<Args, Message> {
  return {
    kind: "stream",
    requestChannel,
    responseChannelPrefix,
    argCount,
  };
}

export const DESKTOP_IPC_SCHEMA = {
  getDaemonStatus: invokeChannel<[], DaemonControlSnapshot>("desktop:get-daemon-status", 0),
  startDaemon: invokeChannel<[], DaemonControlSnapshot>("desktop:start-daemon", 0),
  stopDaemon: invokeChannel<[], DaemonControlSnapshot>("desktop:stop-daemon", 0),
  enableBackgroundService: invokeChannel<[], DaemonControlSnapshot>(
    "desktop:enable-background-service",
    0,
  ),
  disableBackgroundService: invokeChannel<[], DaemonControlSnapshot>(
    "desktop:disable-background-service",
    0,
  ),
  reconcileDaemon: invokeChannel<[], DaemonControlSnapshot>("desktop:reconcile-daemon", 0),
  getDaemonDiagnostics: invokeChannel<[], DaemonDiagnostics>("desktop:get-daemon-diagnostics", 0),
  getAgentRuntime: invokeChannel<[], AgentRuntimeSnapshot>("desktop:get-agent-runtime", 0),
  selectAgentRuntimeProfile: invokeChannel<
    [params: DaemonAgentRuntimeSelectProfileParams],
    AgentRuntimeSnapshot
  >("desktop:select-agent-runtime-profile", 1),
  patchAgentRuntimeProfile: invokeChannel<
    [params: DaemonAgentRuntimePatchProfileParams],
    AgentRuntimeSnapshot
  >("desktop:patch-agent-runtime-profile", 1),
  loginAgentRuntimeAuthProfile: invokeChannel<
    [params: DaemonAgentRuntimeAuthLoginParams],
    AuthProfileLoginResult
  >("desktop:login-agent-runtime-auth-profile", 1),
  logoutAgentRuntimeAuthProfile: invokeChannel<
    [params: DaemonAgentRuntimeAuthLogoutParams],
    AuthProfileLogoutResult
  >("desktop:logout-agent-runtime-auth-profile", 1),
  setAgentRuntimeExtensionEnabled: invokeChannel<
    [params: DaemonAgentRuntimeSetExtensionEnabledParams],
    AgentRuntimeSnapshot
  >("desktop:set-agent-runtime-extension-enabled", 1),
  pickWorkspaceFolder: invokeChannel<[], string | null>("desktop:pick-workspace-folder", 0),
  openWorkspace: invokeChannel<[params: DaemonWorkspaceOpenParams], DaemonWorkspaceOpenResult>(
    "desktop:open-workspace",
    1,
  ),
  listSessions: invokeChannel<[], SessionSummary[]>("desktop:list-sessions", 0),
  getSessionOverview: invokeChannel<[query: SessionOverviewQuery], SessionOverviewResult>(
    "desktop:get-session-overview",
    1,
  ),
  openSession: invokeChannel<[title: string, workspace: WorkspaceSelector], SessionSummary>(
    "desktop:open-session",
    2,
  ),
  listRecipes: invokeChannel<[], RecipeListResponse>("desktop:list-recipes", 0),
  listWorkItems: invokeChannel<[], WorkItemListResult>("desktop:list-work-items", 0),
  loadWorkflow: invokeChannel<[params: WorkflowLoadParams], WorkflowStatusResult>(
    "desktop:load-workflow",
    1,
  ),
  getWorkflowStatus: invokeChannel<[], WorkflowStatusResult>("desktop:get-workflow-status", 0),
  reloadWorkflow: invokeChannel<[], WorkflowStatusResult>("desktop:reload-workflow", 0),
  validateWorkflow: invokeChannel<[params: WorkflowValidateParams], WorkflowValidationReport>(
    "desktop:validate-workflow",
    1,
  ),
  refreshWorkItems: invokeChannel<[params: WorkItemRefreshParams], WorkItemListResult>(
    "desktop:refresh-work-items",
    1,
  ),
  dismissWorkItem: invokeChannel<[params: WorkItemDismissParams], WorkItemDismissResult>(
    "desktop:dismiss-work-item",
    1,
  ),
  triggerWorkItem: invokeChannel<
    [sessionId: SessionId, params: WorkItemTriggerParams],
    WorkItemTriggerResult
  >("desktop:trigger-work-item", 2),
  startRun: invokeChannel<[sessionId: SessionId, command: StartRunCommand], RunSummary>(
    "desktop:start-run",
    2,
  ),
  forkRun: invokeChannel<[sessionId: SessionId, request: ForkRunRequest], ForkRunResult>(
    "desktop:fork-run",
    2,
  ),
  decideApproval: invokeChannel<
    [sessionId: SessionId, approvalId: ApprovalId, decision: ApprovalDecision],
    RunSummary
  >("desktop:decide-approval", 3),
  getSession: invokeChannel<[sessionId: SessionId], SessionSummary | null>(
    "desktop:get-session",
    1,
  ),
  getActivityPage: invokeChannel<
    [sessionId: SessionId, query: ActivityPageQuery],
    ActivityPageResult
  >("desktop:get-activity-page", 2),
  getAgentTurnsPage: invokeChannel<
    [sessionId: SessionId, query: AgentTurnsPageQuery],
    AgentTurnsPageResult
  >("desktop:get-agent-turns-page", 2),
  listApprovals: invokeChannel<
    [sessionId: SessionId, query: ListApprovalsQuery],
    ApprovalSnapshotResult
  >("desktop:list-approvals", 2),
  listArtifacts: invokeChannel<
    [sessionId: SessionId, query: ListArtifactsQuery],
    ArtifactSnapshotResult
  >("desktop:list-artifacts", 2),
  readArtifactContent: invokeChannel<[query: ReadArtifactContentQuery], ReadArtifactContentResult>(
    "desktop:read-artifact-content",
    1,
  ),
  saveArtifactAs: invokeChannel<[query: SaveArtifactAsQuery], SaveArtifactAsResult>(
    "desktop:save-artifact-as",
    1,
  ),
  listRuns: invokeChannel<[sessionId: SessionId], RunSummary[]>("desktop:list-runs", 1),
  getRunDetail: invokeChannel<[sessionId: SessionId, runId: RunId], RunDetail | null>(
    "desktop:get-run-detail",
    2,
  ),
  listNativeRuns: invokeChannel<
    [sessionId: SessionId, query: ListNativeRunsRequest],
    ListNativeRunsResult
  >("desktop:list-native-runs", 2),
  getRunTimeline: invokeChannel<[sessionId: SessionId, rootRunId: RunId], RunTimeline>(
    "desktop:get-run-timeline",
    2,
  ),
  replayRunEvents: invokeChannel<
    [sessionId: SessionId, runId: RunId, afterSeq: bigint | null],
    SubscribeRunEventsResult
  >("desktop:replay-run-events", 3),
  openRunStream: streamChannel<
    [sessionId: SessionId, afterCursor: ActivityCursor | null],
    RunStreamMessage
  >("desktop:open-run-stream", "desktop:run-stream-port", 2),
  openRunEventStream: streamChannel<
    [sessionId: SessionId, runId: RunId, afterSeq: bigint | null],
    RunEventStreamMessage
  >("desktop:open-run-event-stream", "desktop:run-event-stream-port", 3),
  openApprovalStream: streamChannel<
    [sessionId: SessionId, afterCursor: DaemonEventCursor | null],
    ApprovalStreamMessage
  >("desktop:open-approval-stream", "desktop:approval-stream-port", 2),
  openArtifactStream: streamChannel<
    [sessionId: SessionId, afterCursor: DaemonEventCursor | null],
    ArtifactStreamMessage
  >("desktop:open-artifact-stream", "desktop:artifact-stream-port", 2),
  openAgentStream: streamChannel<
    [sessionId: SessionId, afterCursor: DaemonEventCursor | null],
    AgentStreamMessage
  >("desktop:open-agent-stream", "desktop:agent-stream-port", 2),
} as const satisfies DesktopIpcSchema;

type DesktopIpcMethod = keyof typeof DESKTOP_IPC_SCHEMA;

export type DesktopInvokeMethod = KeysMatching<
  typeof DESKTOP_IPC_SCHEMA,
  DesktopInvokeChannel<any, any>
>;
export type DesktopStreamMethod = KeysMatching<
  typeof DESKTOP_IPC_SCHEMA,
  DesktopStreamChannel<any>
>;

const DESKTOP_IPC_METHODS = Object.keys(DESKTOP_IPC_SCHEMA) as DesktopIpcMethod[];

export const DESKTOP_INVOKE_METHODS = DESKTOP_IPC_METHODS.filter(
  (method): method is DesktopInvokeMethod => DESKTOP_IPC_SCHEMA[method].kind === "invoke",
);

export const DESKTOP_STREAM_METHODS = DESKTOP_IPC_METHODS.filter(
  (method): method is DesktopStreamMethod => DESKTOP_IPC_SCHEMA[method].kind === "stream",
);

type DesktopInvokeSchema = Pick<typeof DESKTOP_IPC_SCHEMA, DesktopInvokeMethod>;

export type DesktopApi = DesktopApiFromSchema<DesktopInvokeSchema>;
export type DesktopInvokeHandlers = DesktopApi;
export type DesktopStreamSpec = (typeof DESKTOP_IPC_SCHEMA)[DesktopStreamMethod];

export const DESKTOP_WINDOW_CHANNELS = {
  close: "desktop:window-close",
  getState: "desktop:get-window-state",
  minimize: "desktop:window-minimize",
  stateDidChange: "desktop:window-state-did-change",
  toggleMaximize: "desktop:window-toggle-maximize",
} as const;

export type DesktopWindowPlatform = "linux" | "macos" | "windows";
export type DesktopWindowControlsAlignment = "leading" | "trailing";

export interface DesktopWindowState {
  readonly canClose: boolean;
  readonly canMaximize: boolean;
  readonly canMinimize: boolean;
  readonly controlsAlignment: DesktopWindowControlsAlignment;
  readonly isFocused: boolean;
  readonly isFullScreen: boolean;
  readonly isMaximized: boolean;
  readonly platform: DesktopWindowPlatform;
}

export interface DesktopWindowApi {
  close(): Promise<void>;
  getSnapshot(): DesktopWindowState;
  minimize(): Promise<DesktopWindowState>;
  subscribe(listener: () => void): () => void;
  toggleMaximize(): Promise<DesktopWindowState>;
}

/**
 * Canonical 2026 cross-OS window chrome:
 *
 * - macOS: native traffic lights inset from the edge, everything else custom.
 *   `titleBarStyle: "hiddenInset"` keeps the native close/min/zoom buttons
 *   rendered by macOS — the renderer must NOT draw its own traffic lights.
 * - Windows: Window Controls Overlay (WCO). `titleBarStyle: "hidden"` +
 *   `titleBarOverlay` → Windows paints native min/max/close at the trailing
 *   edge; the renderer reserves space via padding.
 * - Linux: frameless. No reliable native chrome across DE/distros, so the
 *   renderer draws its own min/max/close controls.
 */
export type DesktopWindowTitleBarStyle = "default" | "hidden" | "hiddenInset";

export interface DesktopWindowChromeOptions {
  readonly frame: boolean;
  readonly titleBarStyle: DesktopWindowTitleBarStyle;
  readonly titleBarOverlay?: {
    readonly color: string;
    readonly symbolColor: string;
    readonly height: number;
  };
  readonly trafficLightPosition?: { readonly x: number; readonly y: number };
}

export const WINDOW_CHROME_NATIVE_INSET_PX = {
  macosLeading: 78,
  windowsTrailing: 138,
} as const;

export interface WindowChromeColors {
  readonly background: string;
  readonly symbol: string;
}

export function resolveWindowChromeOptions(
  platform: DesktopWindowPlatform,
  colors: WindowChromeColors,
): DesktopWindowChromeOptions {
  if (platform === "macos") {
    return {
      frame: true,
      titleBarStyle: "hiddenInset",
      trafficLightPosition: { x: 12, y: 12 },
    };
  }
  if (platform === "windows") {
    return {
      frame: true,
      titleBarStyle: "hidden",
      titleBarOverlay: {
        color: colors.background,
        symbolColor: colors.symbol,
        height: 36,
      },
    };
  }
  return {
    frame: false,
    titleBarStyle: "default",
  };
}

export function rendererOwnsWindowControls(platform: DesktopWindowPlatform): boolean {
  return platform === "linux";
}

export function assertDesktopIpcArgCount(method: DesktopIpcMethod, args: unknown[]): void {
  const expectedArgCount = DESKTOP_IPC_SCHEMA[method].argCount;
  if (args.length !== expectedArgCount) {
    throw new Error(
      `desktop IPC method ${method} expected ${expectedArgCount} arg(s), got ${args.length}`,
    );
  }
}

export function createDesktopStreamOpenRequest(
  spec: DesktopStreamSpec,
  requestId: string,
  args: unknown[],
): DesktopStreamOpenRequest<unknown[]> {
  if (args.length !== spec.argCount) {
    throw new Error(
      `desktop stream channel ${spec.requestChannel} expected ${spec.argCount} arg(s), got ${args.length}`,
    );
  }
  return {
    args,
    requestId: parseDesktopStreamRequestId(requestId),
  };
}

export function parseDesktopStreamOpenRequest(
  method: DesktopStreamMethod,
  value: unknown,
): DesktopStreamOpenRequest<unknown[]> {
  const spec = DESKTOP_IPC_SCHEMA[method];
  if (!isObjectRecord(value)) {
    throw new Error(`desktop stream request ${method} must be an object`);
  }

  const requestId = parseDesktopStreamRequestId(value.requestId);
  const args = value.args;
  if (!Array.isArray(args)) {
    throw new Error(`desktop stream request ${method} args must be an array`);
  }
  if (args.length !== spec.argCount) {
    throw new Error(
      `desktop IPC method ${method} expected ${spec.argCount} arg(s), got ${args.length}`,
    );
  }

  return { args, requestId };
}

export function getDesktopStreamResponseChannel(
  spec: DesktopStreamSpec,
  requestId: string,
): string {
  return `${spec.responseChannelPrefix}${parseDesktopStreamRequestId(requestId)}`;
}

export function createDesktopStreamOpenSuccessResponse(): DesktopStreamOpenResponse {
  return { status: "ok" };
}

export function createDesktopStreamOpenErrorResponse(message: string): DesktopStreamOpenResponse {
  return {
    message: parseDesktopStreamOpenErrorMessage(message),
    status: "error",
  };
}

export function parseDesktopStreamOpenResponse(
  method: DesktopStreamMethod,
  value: unknown,
): DesktopStreamOpenResponse {
  if (!isObjectRecord(value)) {
    throw new Error(`desktop stream response ${method} must be an object`);
  }

  if (value.status === "ok") {
    return { status: "ok" };
  }
  if (value.status === "error") {
    return {
      message: parseDesktopStreamOpenErrorMessage(value.message),
      status: "error",
    };
  }

  throw new Error(`desktop stream response ${method} status must be "ok" or "error"`);
}

export function createDesktopApi(ops: {
  invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
}): DesktopApi {
  const api: Partial<DesktopApi> = {};

  for (const method of DESKTOP_INVOKE_METHODS) {
    const spec = DESKTOP_IPC_SCHEMA[method];
    const invokeSpec = spec;
    (api as Record<string, unknown>)[method] = (...args: unknown[]) =>
      ops.invoke(invokeSpec.channel, ...args);
  }

  return api as DesktopApi;
}

export interface DesktopStreamStatus<Name extends string> {
  latestCursor?: DaemonEventCursor | null;
  stream: Name;
  status: "ready" | "historyGap" | "terminalError";
}

type StreamEventEnvelope<TEvent extends DaemonEventEnvelope["event"]> = Omit<
  DaemonEventEnvelope,
  "event"
> & {
  event: TEvent;
};

export type RunStreamEventEnvelope = StreamEventEnvelope<DaemonEventEnvelope["event"]>;

export type ApprovalStreamEventEnvelope = StreamEventEnvelope<
  Extract<DaemonEventEnvelope["event"], { approval: unknown }>
>;

export type ArtifactStreamEventEnvelope = StreamEventEnvelope<
  Extract<DaemonEventEnvelope["event"], { artifact: unknown }>
>;

export type AgentStreamEventEnvelope = StreamEventEnvelope<
  Extract<DaemonEventEnvelope["event"], { agentStream: unknown }>
>;

export type RunStreamMessage = DesktopStreamStatus<"runs"> | RunStreamEventEnvelope;
export type ApprovalStreamMessage = DesktopStreamStatus<"approvals"> | ApprovalStreamEventEnvelope;
export type ArtifactStreamMessage = DesktopStreamStatus<"artifacts"> | ArtifactStreamEventEnvelope;
export type AgentStreamMessage = DesktopStreamStatus<"agentStream"> | AgentStreamEventEnvelope;

export interface DesktopStreamsApi {
  subscribeRunStream(
    sessionId: SessionId,
    afterCursor: ActivityCursor | null,
    listener: DesktopStreamListener<RunStreamMessage>,
    onError?: DesktopStreamErrorListener,
  ): Promise<DesktopStreamUnsubscribe>;
  subscribeRunEventStream(
    sessionId: SessionId,
    runId: RunId,
    afterSeq: bigint | null,
    listener: DesktopStreamListener<RunEventStreamMessage>,
    onError?: DesktopStreamErrorListener,
  ): Promise<DesktopStreamUnsubscribe>;
  subscribeApprovalStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    listener: DesktopStreamListener<ApprovalStreamMessage>,
    onError?: DesktopStreamErrorListener,
  ): Promise<DesktopStreamUnsubscribe>;
  subscribeArtifactStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    listener: DesktopStreamListener<ArtifactStreamMessage>,
    onError?: DesktopStreamErrorListener,
  ): Promise<DesktopStreamUnsubscribe>;
  subscribeAgentStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    listener: DesktopStreamListener<AgentStreamMessage>,
    onError?: DesktopStreamErrorListener,
  ): Promise<DesktopStreamUnsubscribe>;
}

declare global {
  interface Window {
    desktopApi: DesktopApi;
    desktopStreams: DesktopStreamsApi;
    desktopWindow: DesktopWindowApi;
  }
}

export function createDesktopWindowState(
  platform: DesktopWindowPlatform,
  overrides: Partial<Omit<DesktopWindowState, "controlsAlignment" | "platform">> & {
    controlsAlignment?: DesktopWindowControlsAlignment;
  } = {},
): DesktopWindowState {
  return {
    canClose: overrides.canClose ?? true,
    canMaximize: overrides.canMaximize ?? true,
    canMinimize: overrides.canMinimize ?? true,
    controlsAlignment:
      overrides.controlsAlignment ?? defaultDesktopWindowControlsAlignment(platform),
    isFocused: overrides.isFocused ?? true,
    isFullScreen: overrides.isFullScreen ?? false,
    isMaximized: overrides.isMaximized ?? false,
    platform,
  };
}

export function parseDesktopWindowState(value: unknown): DesktopWindowState {
  if (!isObjectRecord(value)) {
    throw new Error("desktop window state must be an object");
  }

  const platform = parseDesktopWindowPlatform(value.platform);
  return createDesktopWindowState(platform, {
    canClose: parseDesktopWindowFlag(value.canClose, "canClose"),
    canMaximize: parseDesktopWindowFlag(value.canMaximize, "canMaximize"),
    canMinimize: parseDesktopWindowFlag(value.canMinimize, "canMinimize"),
    controlsAlignment:
      value.controlsAlignment === undefined
        ? undefined
        : parseDesktopWindowControlsAlignment(value.controlsAlignment),
    isFocused: parseDesktopWindowFlag(value.isFocused, "isFocused"),
    isFullScreen: parseDesktopWindowFlag(value.isFullScreen, "isFullScreen"),
    isMaximized: parseDesktopWindowFlag(value.isMaximized, "isMaximized"),
  });
}

export function resolveDesktopWindowPlatform(platform: string): DesktopWindowPlatform {
  if (platform === "darwin") {
    return "macos";
  }
  if (platform === "win32") {
    return "windows";
  }
  return "linux";
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function defaultDesktopWindowControlsAlignment(
  platform: DesktopWindowPlatform,
): DesktopWindowControlsAlignment {
  return platform === "macos" ? "leading" : "trailing";
}

function parseDesktopWindowControlsAlignment(value: unknown): DesktopWindowControlsAlignment {
  if (value === "leading" || value === "trailing") {
    return value;
  }
  throw new Error('desktop window controls alignment must be "leading" or "trailing"');
}

function parseDesktopWindowFlag(
  value: unknown,
  field: keyof Omit<DesktopWindowState, "controlsAlignment" | "platform">,
): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`desktop window state ${field} must be a boolean`);
  }
  return value;
}

function parseDesktopWindowPlatform(value: unknown): DesktopWindowPlatform {
  if (value === "linux" || value === "macos" || value === "windows") {
    return value;
  }
  throw new Error('desktop window state platform must be "linux", "macos", or "windows"');
}

function parseDesktopStreamRequestId(value: unknown): string {
  if (typeof value !== "string") {
    throw new Error("desktop stream request id must be a non-empty string");
  }
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new Error("desktop stream request id must be a non-empty string");
  }
  return normalized;
}

function parseDesktopStreamOpenErrorMessage(value: unknown): string {
  if (typeof value !== "string") {
    throw new Error("desktop stream open error message must be a non-empty string");
  }
  const normalized = value.trim();
  if (normalized.length === 0) {
    throw new Error("desktop stream open error message must be a non-empty string");
  }
  return normalized;
}
