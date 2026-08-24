import { BrowserWindow, dialog, type MessagePortMain } from "electron";

import type {
  AgentTurnsPageQuery,
  AgentTurnsPageResult,
  ActivityCursor,
  ActivityPageQuery,
  ActivityPageResult,
  ApprovalDecision,
  ApprovalId,
  ApprovalSnapshotResult,
  ArtifactSnapshotResult,
  ArtifactSummary,
  ReadArtifactContentQuery,
  ReadArtifactContentResult,
  SaveArtifactAsQuery,
  SaveArtifactAsResult,
  SessionOverviewQuery,
  SessionOverviewResult,
  DaemonEventCursor,
  DaemonApprovalDecideResult,
  DaemonWorkspaceOpenParams,
  DesktopWorkspaceOpenResult,
  DesktopInvokeHandlers,
  ForkRunRequest,
  ForkRunResult,
  DesktopStreamMethod,
  ListApprovalsQuery,
  ListArtifactsQuery,
  ListNativeRunsRequest,
  ListNativeRunsResult,
  RecipeListResponse,
  RunDetail,
  RunTimeline,
  SessionId,
  RunId,
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
} from "@taugentic/desktop-shared";
import {
  parseAgentTurnsPageQuery,
  parseActivityPageQuery,
  parseApprovalDecision,
  parseApprovalId,
  parseSessionOverviewQuery,
  parseDaemonSessionOpenParams,
  parseDaemonWorkspaceOpenParams,
  parseListApprovalsQuery,
  parseListArtifactsQuery,
  parseListNativeRunsRequest,
  parseRunId,
  parseSessionId,
  parseStartRunCommand,
  parseForkRunRequest,
  parseWorkItemDismissParams,
  parseWorkflowLoadParams,
  parseWorkflowValidateParams,
  parseWorkItemRefreshParams,
  parseWorkItemTriggerParams,
} from "@taugentic/desktop-shared/validation";

import {
  loadDesktopSessionAuthority,
  removeDesktopSessionAuthority,
} from "./daemon-session-authority.js";
import { DaemonJsonRpcError } from "./daemon-rpc-connection.js";
import { DaemonSessionRequestClient } from "./daemon-session-request-client.js";
import {
  createProductionDesktopArtifactIo,
  handleReadArtifactContent,
  handleSaveArtifactAs,
  resolveFocusedBrowserWindow,
  type DesktopArtifactIo,
} from "./desktop-artifact-service.js";
import { SessionStreamConnection } from "./session-stream-connection.js";
import { RunEventStreamConnection } from "./run-event-stream-connection.js";

const sharedDaemonSession = new DaemonSessionRequestClient();
const attachedStreamSessions = new Map<SessionId, SessionStreamConnection>();
const attachedRequestSessions = new Map<SessionId, DaemonSessionRequestClient>();
const attachedRunEventStreams = new Set<RunEventStreamConnection>();

async function hasLocalSessionAuthority(sessionId: SessionId): Promise<boolean> {
  return (await loadDesktopSessionAuthority("desktop-main", sessionId)) !== null;
}

async function filterSessionsWithLocalAuthority<T extends { session: SessionSummary }>(
  items: T[],
): Promise<T[]> {
  const itemsWithAuthority = await Promise.all(
    items.map(async (item) => ({
      hasLocalAuthority: await hasLocalSessionAuthority(item.session.id),
      item,
    })),
  );
  return itemsWithAuthority.filter((entry) => entry.hasLocalAuthority).map((entry) => entry.item);
}

function getAttachedStreamSession(sessionId: SessionId): SessionStreamConnection {
  const existing = attachedStreamSessions.get(sessionId);
  if (existing) {
    return existing;
  }

  const created = new SessionStreamConnection(sessionId, () => {
    attachedStreamSessions.delete(sessionId);
    created.dispose();
  });
  attachedStreamSessions.set(sessionId, created);
  return created;
}

function getAttachedRequestSession(sessionId: SessionId): DaemonSessionRequestClient {
  const existing = attachedRequestSessions.get(sessionId);
  if (existing) {
    return existing;
  }

  const created = new DaemonSessionRequestClient(sessionId);
  attachedRequestSessions.set(sessionId, created);
  return created;
}

function disposeAttachedRequestSession(sessionId: SessionId): void {
  const session = attachedRequestSessions.get(sessionId);
  if (!session) {
    return;
  }

  attachedRequestSessions.delete(sessionId);
  session.dispose();
}

async function withAttachedRequestSession<Result>(
  sessionId: SessionId,
  operation: (session: DaemonSessionRequestClient) => Promise<Result>,
): Promise<Result> {
  const session = getAttachedRequestSession(sessionId);
  try {
    return await operation(session);
  } catch (error) {
    if (isRejectedSessionAuthorityError(error, sessionId)) {
      await removeDesktopSessionAuthority("desktop-main", sessionId);
      disposeAttachedRequestSession(sessionId);
    }
    throw error;
  }
}

export async function attachRunStreamPort(
  sessionId: SessionId,
  port: MessagePortMain,
  afterCursor: ActivityCursor | null = null,
): Promise<void> {
  await getAttachedStreamSession(sessionId).attachRunPort(port, afterCursor);
}

async function attachRunEventStreamPort(
  sessionId: SessionId,
  runId: RunId,
  port: MessagePortMain,
  afterSeq: bigint | null = null,
): Promise<void> {
  const stream = new RunEventStreamConnection(sessionId, runId, port, afterSeq, () => {
    attachedRunEventStreams.delete(stream);
  });
  attachedRunEventStreams.add(stream);
  try {
    await stream.open();
  } catch (error) {
    stream.dispose();
    throw error;
  }
}

export async function attachApprovalStreamPort(
  sessionId: SessionId,
  port: MessagePortMain,
  afterCursor: DaemonEventCursor | null = null,
): Promise<void> {
  await getAttachedStreamSession(sessionId).attachApprovalPort(port, afterCursor);
}

async function attachArtifactStreamPort(
  sessionId: SessionId,
  port: MessagePortMain,
  afterCursor: DaemonEventCursor | null = null,
): Promise<void> {
  await getAttachedStreamSession(sessionId).attachArtifactPort(port, afterCursor);
}

async function attachAgentStreamPort(
  sessionId: SessionId,
  port: MessagePortMain,
  afterCursor: DaemonEventCursor | null = null,
): Promise<void> {
  await getAttachedStreamSession(sessionId).attachAgentStreamPort(port, afterCursor);
}

export async function listDaemonSessions(): Promise<SessionSummary[]> {
  const sessions = await sharedDaemonSession.listSessions();
  return (await filterSessionsWithLocalAuthority(sessions.map((session) => ({ session })))).map(
    (entry) => entry.session,
  );
}

export async function getDaemonSessionOverview(
  query: SessionOverviewQuery,
): Promise<SessionOverviewResult> {
  const overview = await sharedDaemonSession.getSessionOverview(query);
  return {
    ...overview,
    sessions: await filterSessionsWithLocalAuthority(overview.sessions ?? []),
  };
}

function isRejectedSessionAuthorityError(error: unknown, sessionId: SessionId): boolean {
  return (
    error instanceof DaemonJsonRpcError &&
    error.code === -32_602 &&
    (error.rpcMessage === `session does not exist: ${sessionId}` ||
      error.rpcMessage === `session authority rejected: ${sessionId}`)
  );
}

export async function getSession(sessionId: SessionId): Promise<SessionSummary | null> {
  return withAttachedRequestSession(sessionId, (session) => session.getSession());
}

async function pickWorkspaceFolder(): Promise<string | null> {
  const parentWindow = BrowserWindow.getFocusedWindow() ?? undefined;
  const result = parentWindow
    ? await dialog.showOpenDialog(parentWindow, {
        properties: ["openDirectory"],
      })
    : await dialog.showOpenDialog({
        properties: ["openDirectory"],
      });
  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return result.filePaths[0] ?? null;
}

async function openWorkspace(
  params: DaemonWorkspaceOpenParams,
): Promise<DesktopWorkspaceOpenResult> {
  const request = parseDaemonWorkspaceOpenParams(params);
  try {
    const result = await sharedDaemonSession.openWorkspace(request);
    return {
      status: "opened",
      workspace: result.workspace,
    };
  } catch (error) {
    if (isWorkspaceTrustRequiredError(error)) {
      return {
        status: "trustRequired",
        path: request.path,
      };
    }
    throw error;
  }
}

function isWorkspaceTrustRequiredError(error: unknown): boolean {
  if (!(error instanceof DaemonJsonRpcError) || error.code !== -32_602) {
    return false;
  }
  const data = error.data;
  return typeof data === "object" && data !== null && "code" in data
    ? data.code === "WorkspaceTrustRequired"
    : false;
}

async function openSession(title: string, workspace: WorkspaceSelector): Promise<SessionSummary> {
  return sharedDaemonSession.openSession(title, workspace);
}

async function listRecipes(): Promise<RecipeListResponse> {
  return sharedDaemonSession.listRecipes();
}

async function listWorkItems(): Promise<WorkItemListResult> {
  return sharedDaemonSession.listWorkItems();
}

async function loadWorkflow(params: WorkflowLoadParams): Promise<WorkflowStatusResult> {
  return sharedDaemonSession.loadWorkflow(parseWorkflowLoadParams(params));
}

async function getWorkflowStatus(): Promise<WorkflowStatusResult> {
  return sharedDaemonSession.getWorkflowStatus();
}

async function reloadWorkflow(): Promise<WorkflowStatusResult> {
  return sharedDaemonSession.reloadWorkflow();
}

async function validateWorkflow(params: WorkflowValidateParams): Promise<WorkflowValidationReport> {
  return sharedDaemonSession.validateWorkflow(parseWorkflowValidateParams(params));
}

async function refreshWorkItems(params: WorkItemRefreshParams): Promise<WorkItemListResult> {
  return sharedDaemonSession.refreshWorkItems(parseWorkItemRefreshParams(params));
}

async function dismissWorkItem(params: WorkItemDismissParams): Promise<WorkItemDismissResult> {
  return sharedDaemonSession.dismissWorkItem(parseWorkItemDismissParams(params));
}

async function triggerWorkItem(
  sessionId: SessionId,
  params: WorkItemTriggerParams,
): Promise<WorkItemTriggerResult> {
  return withAttachedRequestSession(sessionId, (session) =>
    session.triggerWorkItem(parseWorkItemTriggerParams(params)),
  );
}

async function startRun(sessionId: SessionId, command: StartRunCommand): Promise<RunSummary> {
  return withAttachedRequestSession(sessionId, (session) => session.startRun(command));
}

async function forkRun(sessionId: SessionId, request: ForkRunRequest): Promise<ForkRunResult> {
  const parsedSessionId = parseSessionId(sessionId);
  const parsedRequest = parseForkRunRequest(request);
  return withAttachedRequestSession(parsedSessionId, (session) => session.forkRun(parsedRequest));
}

async function decideApproval(
  sessionId: SessionId,
  approvalId: ApprovalId,
  decision: ApprovalDecision,
): Promise<RunSummary> {
  const result: DaemonApprovalDecideResult = await withAttachedRequestSession(
    sessionId,
    (session) => session.decideApproval(approvalId, decision),
  );
  return result.run;
}

export async function listDaemonRuns(sessionId: SessionId): Promise<RunSummary[]> {
  return withAttachedRequestSession(sessionId, (session) => session.listRuns({}));
}

export async function listDaemonNativeRuns(
  sessionId: SessionId,
  query: ListNativeRunsRequest,
): Promise<ListNativeRunsResult> {
  return withAttachedRequestSession(sessionId, (session) =>
    session.listNativeRuns(parseListNativeRunsRequest(query)),
  );
}

async function getRunDetail(sessionId: SessionId, runId: RunId): Promise<RunDetail | null> {
  return withAttachedRequestSession(sessionId, (session) => session.getRunDetail(runId));
}

async function getRunTimeline(sessionId: SessionId, rootRunId: RunId): Promise<RunTimeline> {
  return withAttachedRequestSession(sessionId, (session) => session.getRunTimeline(rootRunId));
}

async function getActivityPage(
  sessionId: SessionId,
  query: ActivityPageQuery,
): Promise<ActivityPageResult> {
  return withAttachedRequestSession(sessionId, (session) => session.getActivityPage(query));
}

async function getAgentTurnsPage(
  sessionId: SessionId,
  query: AgentTurnsPageQuery,
): Promise<AgentTurnsPageResult> {
  return withAttachedRequestSession(sessionId, (session) => session.getAgentTurnsPage(query));
}

async function replayRunEvents(
  sessionId: SessionId,
  runId: RunId,
  afterSeq: bigint | null,
): Promise<SubscribeRunEventsResult> {
  return withAttachedRequestSession(sessionId, (session) =>
    session.replayRunEvents(runId, afterSeq),
  );
}

async function listApprovals(
  sessionId: SessionId,
  query: ListApprovalsQuery,
): Promise<ApprovalSnapshotResult> {
  return withAttachedRequestSession(sessionId, (session) => session.listApprovals(query));
}

async function listArtifacts(
  sessionId: SessionId,
  query: ListArtifactsQuery,
): Promise<ArtifactSnapshotResult> {
  return withAttachedRequestSession(sessionId, (session) => session.listArtifacts(query));
}

const desktopArtifactRoot: string | null = null;

function resolveDesktopArtifactIo(): DesktopArtifactIo {
  return createProductionDesktopArtifactIo();
}

async function getArtifactForSession(
  sessionId: SessionId,
  artifactId: SaveArtifactAsQuery["artifactId"],
): Promise<ArtifactSummary | null> {
  return withAttachedRequestSession(sessionId, (session) => session.getArtifact(artifactId));
}

async function readArtifactContent(
  query: ReadArtifactContentQuery,
): Promise<ReadArtifactContentResult> {
  const io = resolveDesktopArtifactIo();
  return handleReadArtifactContent(query, {
    io,
    artifactRoot: desktopArtifactRoot,
    getArtifact: (parsed) => getArtifactForSession(parsed.sessionId, parsed.artifactId),
  });
}

async function saveArtifactAs(query: SaveArtifactAsQuery): Promise<SaveArtifactAsResult> {
  const io = resolveDesktopArtifactIo();
  return handleSaveArtifactAs(query, resolveFocusedBrowserWindow(), {
    io,
    artifactRoot: desktopArtifactRoot,
    getArtifact: (parsed) => getArtifactForSession(parsed.sessionId, parsed.artifactId),
  });
}

export const desktopSessionInvokeHandlers: Pick<
  DesktopInvokeHandlers,
  | "pickWorkspaceFolder"
  | "openWorkspace"
  | "openSession"
  | "listRecipes"
  | "listWorkItems"
  | "loadWorkflow"
  | "getWorkflowStatus"
  | "reloadWorkflow"
  | "validateWorkflow"
  | "refreshWorkItems"
  | "dismissWorkItem"
  | "triggerWorkItem"
  | "startRun"
  | "forkRun"
  | "decideApproval"
  | "getSessionOverview"
  | "getSession"
  | "getActivityPage"
  | "getAgentTurnsPage"
  | "listApprovals"
  | "listArtifacts"
  | "readArtifactContent"
  | "saveArtifactAs"
  | "getRunDetail"
  | "getRunTimeline"
  | "replayRunEvents"
> = {
  pickWorkspaceFolder: () => pickWorkspaceFolder(),
  openWorkspace: (params) => openWorkspace(params),
  openSession: (title, workspace) => {
    const parsed = parseDaemonSessionOpenParams({ title, workspace });
    return openSession(parsed.title, parsed.workspace);
  },
  listRecipes: () => listRecipes(),
  listWorkItems: () => listWorkItems(),
  loadWorkflow: (params) => loadWorkflow(params),
  getWorkflowStatus: () => getWorkflowStatus(),
  reloadWorkflow: () => reloadWorkflow(),
  validateWorkflow: (params) => validateWorkflow(params),
  refreshWorkItems: (params) => refreshWorkItems(params),
  dismissWorkItem: (params) => dismissWorkItem(params),
  triggerWorkItem: (sessionId, params) => triggerWorkItem(parseSessionId(sessionId), params),
  startRun: (sessionId, command) =>
    startRun(parseSessionId(sessionId), parseStartRunCommand(command)),
  forkRun: (sessionId, request) => forkRun(parseSessionId(sessionId), request),
  decideApproval: (sessionId, approvalId, decision) =>
    decideApproval(
      parseSessionId(sessionId),
      parseApprovalId(approvalId),
      parseApprovalDecision(decision),
    ),
  getSessionOverview: (query) => getDaemonSessionOverview(parseSessionOverviewQuery(query)),
  getSession: (sessionId) => getSession(parseSessionId(sessionId)),
  getActivityPage: (sessionId, query) =>
    getActivityPage(parseSessionId(sessionId), parseActivityPageQuery(query)),
  getAgentTurnsPage: (sessionId, query) =>
    getAgentTurnsPage(parseSessionId(sessionId), parseAgentTurnsPageQuery(query)),
  getRunDetail: (sessionId, runId) => getRunDetail(parseSessionId(sessionId), parseRunId(runId)),
  getRunTimeline: (sessionId, rootRunId) =>
    getRunTimeline(parseSessionId(sessionId), parseRunId(rootRunId)),
  replayRunEvents: (sessionId, runId, afterSeq) =>
    replayRunEvents(
      parseSessionId(sessionId),
      parseRunId(runId),
      parseNullableRunEventSeq(afterSeq),
    ),
  listApprovals: (sessionId, query) =>
    listApprovals(parseSessionId(sessionId), parseListApprovalsQuery(query)),
  listArtifacts: (sessionId, query) =>
    listArtifacts(parseSessionId(sessionId), parseListArtifactsQuery(query)),
  readArtifactContent: (query) => readArtifactContent(query),
  saveArtifactAs: (query) => saveArtifactAs(query),
};

function parseNullableRunEventSeq(value: unknown): bigint | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value !== "bigint" || value < 0n) {
    throw new Error("desktop replayRunEvents afterSeq must be a uint64 bigint");
  }
  return value;
}

export const desktopSessionStreamHandlers = {
  openRunStream: attachRunStreamPort,
  openRunEventStream: attachRunEventStreamPort,
  openApprovalStream: attachApprovalStreamPort,
  openArtifactStream: attachArtifactStreamPort,
  openAgentStream: attachAgentStreamPort,
} satisfies {
  [Method in DesktopStreamMethod]: Method extends "openRunStream"
    ? typeof attachRunStreamPort
    : Method extends "openRunEventStream"
      ? typeof attachRunEventStreamPort
      : Method extends "openApprovalStream"
        ? typeof attachApprovalStreamPort
        : Method extends "openArtifactStream"
          ? typeof attachArtifactStreamPort
          : typeof attachAgentStreamPort;
};
