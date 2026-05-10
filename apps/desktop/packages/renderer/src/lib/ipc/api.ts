import type {
  AgentRuntimeSnapshot,
  AgentTurnsPageQuery,
  AgentTurnsPageResult,
  ActivityPageQuery,
  ActivityPageResult,
  ApprovalSnapshotResult,
  ApprovalDecision,
  ApprovalId,
  ArtifactSnapshotResult,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonAgentRuntimeTestLocalEndpointParams,
  DaemonDiagnostics,
  ForkRunRequest,
  ForkRunResult,
  ReadArtifactContentQuery,
  ReadArtifactContentResult,
  SaveArtifactAsQuery,
  SaveArtifactAsResult,
  SessionOverviewQuery,
  SessionOverviewResult,
  DaemonControlSnapshot,
  ListApprovalsQuery,
  ListArtifactsQuery,
  ListNativeRunsRequest,
  ListNativeRunsResult,
  LocalModelEndpointTestResult,
  RecipeListResponse,
  RunDetail,
  RunId,
  RunTimeline,
  RunSummary,
  SessionId,
  SessionSummary,
  StartRunCommand,
  SubscribeRunEventsResult,
  WorkItemDismissParams,
  WorkItemDismissResult,
  WorkItemListResult,
  WorkItemRefreshParams,
  WorkItemTriggerParams,
  WorkItemTriggerResult,
  WorkflowStatusResult,
} from "@taugentic/desktop-shared";

export function getDaemonStatus(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.getDaemonStatus();
}

export function startDaemon(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.startDaemon();
}

export function stopDaemon(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.stopDaemon();
}

export function enableBackgroundService(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.enableBackgroundService();
}

export function disableBackgroundService(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.disableBackgroundService();
}

export function reconcileDaemon(): Promise<DaemonControlSnapshot> {
  return window.desktopApi.reconcileDaemon();
}

export function getDaemonDiagnostics(): Promise<DaemonDiagnostics> {
  return window.desktopApi.getDaemonDiagnostics();
}

export function getAgentRuntime(): Promise<AgentRuntimeSnapshot> {
  return window.desktopApi.getAgentRuntime();
}

export function selectAgentRuntimeProfile(
  params: DaemonAgentRuntimeSelectProfileParams,
): Promise<AgentRuntimeSnapshot> {
  return window.desktopApi.selectAgentRuntimeProfile(params);
}

export function patchAgentRuntimeProfile(
  params: DaemonAgentRuntimePatchProfileParams,
): Promise<AgentRuntimeSnapshot> {
  return window.desktopApi.patchAgentRuntimeProfile(params);
}

export function loginAgentRuntimeAuthProfile(
  params: DaemonAgentRuntimeAuthLoginParams,
): Promise<AuthProfileLoginResult> {
  return window.desktopApi.loginAgentRuntimeAuthProfile(params);
}

export function logoutAgentRuntimeAuthProfile(
  params: DaemonAgentRuntimeAuthLogoutParams,
): Promise<AuthProfileLogoutResult> {
  return window.desktopApi.logoutAgentRuntimeAuthProfile(params);
}

export function setAgentRuntimeExtensionEnabled(
  params: DaemonAgentRuntimeSetExtensionEnabledParams,
): Promise<AgentRuntimeSnapshot> {
  return window.desktopApi.setAgentRuntimeExtensionEnabled(params);
}

export function testLocalModelEndpoint(
  params: DaemonAgentRuntimeTestLocalEndpointParams,
): Promise<LocalModelEndpointTestResult> {
  return window.desktopApi.testLocalModelEndpoint(params);
}

export function listSessions(): Promise<SessionSummary[]> {
  return window.desktopApi.listSessions();
}

export function getSessionOverview(query: SessionOverviewQuery): Promise<SessionOverviewResult> {
  return window.desktopApi.getSessionOverview(query);
}

export function openSession(title: string): Promise<SessionSummary> {
  return window.desktopApi.openSession(title);
}

export function listRecipes(): Promise<RecipeListResponse> {
  return window.desktopApi.listRecipes();
}

export function listWorkItems(): Promise<WorkItemListResult> {
  return window.desktopApi.listWorkItems();
}

export function getWorkflowStatus(): Promise<WorkflowStatusResult> {
  return window.desktopApi.getWorkflowStatus();
}

export function refreshWorkItems(params: WorkItemRefreshParams): Promise<WorkItemListResult> {
  return window.desktopApi.refreshWorkItems(params);
}

export function dismissWorkItem(params: WorkItemDismissParams): Promise<WorkItemDismissResult> {
  return window.desktopApi.dismissWorkItem(params);
}

export function triggerWorkItem(
  sessionId: SessionId,
  params: WorkItemTriggerParams,
): Promise<WorkItemTriggerResult> {
  return window.desktopApi.triggerWorkItem(sessionId, params);
}

export function startRun(sessionId: SessionId, command: StartRunCommand): Promise<RunSummary> {
  return window.desktopApi.startRun(sessionId, command);
}

export function forkRun(sessionId: SessionId, request: ForkRunRequest): Promise<ForkRunResult> {
  return window.desktopApi.forkRun(sessionId, request);
}

export function decideApproval(
  sessionId: SessionId,
  approvalId: ApprovalId,
  decision: ApprovalDecision,
): Promise<RunSummary> {
  return window.desktopApi.decideApproval(sessionId, approvalId, decision);
}

export function getActivityPage(
  sessionId: SessionId,
  query: ActivityPageQuery,
): Promise<ActivityPageResult> {
  return window.desktopApi.getActivityPage(sessionId, query);
}

export function getAgentTurnsPage(
  sessionId: SessionId,
  query: AgentTurnsPageQuery,
): Promise<AgentTurnsPageResult> {
  return window.desktopApi.getAgentTurnsPage(sessionId, query);
}

export function listApprovals(
  sessionId: SessionId,
  query: ListApprovalsQuery,
): Promise<ApprovalSnapshotResult> {
  return window.desktopApi.listApprovals(sessionId, query);
}

export function listArtifacts(
  sessionId: SessionId,
  query: ListArtifactsQuery,
): Promise<ArtifactSnapshotResult> {
  return window.desktopApi.listArtifacts(sessionId, query);
}

export function readArtifactContent(
  query: ReadArtifactContentQuery,
): Promise<ReadArtifactContentResult> {
  return window.desktopApi.readArtifactContent(query);
}

export function saveArtifactAs(query: SaveArtifactAsQuery): Promise<SaveArtifactAsResult> {
  return window.desktopApi.saveArtifactAs(query);
}

export function listRuns(sessionId: SessionId): Promise<RunSummary[]> {
  return window.desktopApi.listRuns(sessionId);
}

export function getRunDetail(sessionId: SessionId, runId: RunId): Promise<RunDetail | null> {
  return window.desktopApi.getRunDetail(sessionId, runId);
}

export function listNativeRuns(
  sessionId: SessionId,
  query: ListNativeRunsRequest,
): Promise<ListNativeRunsResult> {
  return window.desktopApi.listNativeRuns(sessionId, query);
}

export function getRunTimeline(sessionId: SessionId, rootRunId: RunId): Promise<RunTimeline> {
  return window.desktopApi.getRunTimeline(sessionId, rootRunId);
}

export function replayRunEvents(
  sessionId: SessionId,
  runId: RunId,
  afterSeq: bigint | null,
): Promise<SubscribeRunEventsResult> {
  return window.desktopApi.replayRunEvents(sessionId, runId, afterSeq);
}
