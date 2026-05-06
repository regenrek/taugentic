import type {
  AgentRuntimeSnapshot,
  AgentTurnsPageQuery,
  AgentTurnsPageResult,
  ActivityPageQuery,
  ActivityPageResult,
  ApprovalSnapshotResult,
  ApprovalDecision,
  ApprovalId,
  ArtifactId,
  ArtifactSnapshotResult,
  ArtifactSummary,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonDiagnostics,
  SessionOverviewQuery,
  SessionOverviewResult,
  DaemonApprovalDecideResult,
  ForkRunRequest,
  ForkRunResult,
  ListApprovalsQuery,
  ListArtifactsQuery,
  ListNativeRunsRequest,
  ListNativeRunsResult,
  ListRunsQuery,
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
  WorkflowLoadParams,
  WorkflowStatusResult,
  WorkflowValidateParams,
  WorkflowValidationReport,
} from "@taugentic/desktop-shared";
import {
  METHOD_DAEMON_ACTIVITY_PAGE,
  METHOD_DAEMON_AGENT_TURNS_PAGE,
  METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
  METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
  METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
  METHOD_DAEMON_AGENT_RUNTIME_GET,
  METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT,
  METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH,
  METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
  METHOD_DAEMON_APPROVAL_DECIDE,
  METHOD_DAEMON_APPROVAL_LIST,
  METHOD_DAEMON_ARTIFACT_GET,
  METHOD_DAEMON_ARTIFACT_LIST,
  METHOD_DAEMON_WORK_ITEM_DISMISS,
  METHOD_DAEMON_WORK_ITEM_LIST,
  METHOD_DAEMON_WORK_ITEM_REFRESH,
  METHOD_DAEMON_WORK_ITEM_TRIGGER,
  METHOD_DAEMON_RECIPES_LIST,
  METHOD_DAEMON_SESSION_OVERVIEW,
  METHOD_DAEMON_RUN_LIST_NATIVE,
  METHOD_DAEMON_RUN_LIST,
  METHOD_DAEMON_RUN_GET,
  METHOD_DAEMON_RUN_FORK,
  METHOD_DAEMON_RUN_TIMELINE,
  METHOD_DAEMON_RUN_START,
  METHOD_DAEMON_RUN_REPLAY_EVENTS,
  METHOD_DAEMON_SESSION_GET,
  METHOD_DAEMON_SESSION_LIST,
  METHOD_DAEMON_SESSION_OPEN,
  METHOD_WORKFLOW_LOAD,
  METHOD_WORKFLOW_RELOAD,
  METHOD_WORKFLOW_STATUS,
  METHOD_WORKFLOW_VALIDATE,
} from "@taugentic/desktop-shared";
import {
  parseAgentRuntimeSnapshot,
  parseArtifactSummary,
  parseAuthProfileLoginResult,
  parseAuthProfileLogoutResult,
  parseDaemonAgentRuntimeAuthLoginParams,
  parseDaemonAgentRuntimeAuthLogoutParams,
  parseDaemonAgentRuntimePatchProfileParams,
  parseDaemonAgentRuntimeSelectProfileParams,
  parseDaemonAgentRuntimeSetExtensionEnabledParams,
  parseDaemonDiagnostics,
  parseAgentTurnsPageResult,
  parseActivityPageResult,
  parseApprovalSnapshotResult,
  parseArtifactSnapshotResult,
  parseDaemonApprovalDecideResult,
  parseForkRunRequest,
  parseForkRunResult,
  parseListNativeRunsRequest,
  parseListNativeRunsResult,
  parseRecipeListResponse,
  parseRunDetail,
  parseRunTimeline,
  parseSessionOverviewQuery,
  parseSessionOverviewResult,
  parseDaemonSessionOpenResult,
  parseRunSummary,
  parseRunSummaryList,
  parseSessionSummary,
  parseSessionSummaryList,
  parseStartRunCommand,
  parseSubscribeRunEventsResult,
  parseWorkItemDismissParams,
  parseWorkItemDismissResult,
  parseWorkItemListResult,
  parseWorkItemRefreshParams,
  parseWorkItemTriggerParams,
  parseWorkItemTriggerResult,
  parseWorkflowLoadParams,
  parseWorkflowStatusResult,
  parseWorkflowValidateParams,
  parseWorkflowValidationReport,
} from "@taugentic/desktop-shared/validation";

import { storeDesktopSessionAuthority } from "./daemon-session-authority.js";
import { DaemonSessionConnection } from "./daemon-session-connection.js";
import type { DaemonRequestTimeoutPolicy } from "./daemon-rpc-connection.js";
import { DAEMON_REQUEST_TIMEOUT_STANDARD } from "./daemon-rpc-connection.js";

interface DaemonSessionRequestClientOptions {
  requestTimeout?: DaemonRequestTimeoutPolicy;
}

export class DaemonSessionRequestClient {
  constructor(
    private readonly attachedSessionId: SessionId | null = null,
    options: DaemonSessionRequestClientOptions = {},
  ) {
    this.connection = new DaemonSessionConnection(this.attachedSessionId, {
      requestTimeout: options.requestTimeout ?? DAEMON_REQUEST_TIMEOUT_STANDARD,
    });
  }

  private readonly connection: DaemonSessionConnection;

  async listSessions(): Promise<SessionSummary[]> {
    return this.withConnectedRequest(METHOD_DAEMON_SESSION_LIST, {}, parseSessionSummaryList);
  }

  async getAgentRuntime(): Promise<AgentRuntimeSnapshot> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_GET,
      {},
      parseAgentRuntimeSnapshot,
    );
  }

  async getDaemonDiagnostics(): Promise<DaemonDiagnostics> {
    return this.withConnectedRequest(
      METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT,
      {},
      parseDaemonDiagnostics,
    );
  }

  async selectAgentRuntimeProfile(
    params: DaemonAgentRuntimeSelectProfileParams,
  ): Promise<AgentRuntimeSnapshot> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
      parseDaemonAgentRuntimeSelectProfileParams(params),
      parseAgentRuntimeSnapshot,
    );
  }

  async patchAgentRuntimeProfile(
    params: DaemonAgentRuntimePatchProfileParams,
  ): Promise<AgentRuntimeSnapshot> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH,
      parseDaemonAgentRuntimePatchProfileParams(params),
      parseAgentRuntimeSnapshot,
    );
  }

  async loginAgentRuntimeAuthProfile(
    params: DaemonAgentRuntimeAuthLoginParams,
  ): Promise<AuthProfileLoginResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
      parseDaemonAgentRuntimeAuthLoginParams(params),
      parseAuthProfileLoginResult,
    );
  }

  async logoutAgentRuntimeAuthProfile(
    params: DaemonAgentRuntimeAuthLogoutParams,
  ): Promise<AuthProfileLogoutResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
      parseDaemonAgentRuntimeAuthLogoutParams(params),
      parseAuthProfileLogoutResult,
    );
  }

  async setAgentRuntimeExtensionEnabled(
    params: DaemonAgentRuntimeSetExtensionEnabledParams,
  ): Promise<AgentRuntimeSnapshot> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
      parseDaemonAgentRuntimeSetExtensionEnabledParams(params),
      parseAgentRuntimeSnapshot,
    );
  }

  async getSessionOverview(query: SessionOverviewQuery): Promise<SessionOverviewResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_SESSION_OVERVIEW,
      parseSessionOverviewQuery(query),
      parseSessionOverviewResult,
    );
  }

  async listRuns(query: ListRunsQuery): Promise<RunSummary[]> {
    return this.withConnectedRequest(METHOD_DAEMON_RUN_LIST, query, parseRunSummaryList);
  }

  async listNativeRuns(query: ListNativeRunsRequest): Promise<ListNativeRunsResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_RUN_LIST_NATIVE,
      parseListNativeRunsRequest(query),
      parseListNativeRunsResult,
    );
  }

  async listRecipes(): Promise<RecipeListResponse> {
    return this.withConnectedRequest(METHOD_DAEMON_RECIPES_LIST, {}, parseRecipeListResponse);
  }

  async listWorkItems(): Promise<WorkItemListResult> {
    return this.withConnectedRequest(METHOD_DAEMON_WORK_ITEM_LIST, {}, parseWorkItemListResult);
  }

  async loadWorkflow(params: WorkflowLoadParams): Promise<WorkflowStatusResult> {
    return this.withConnectedRequest(
      METHOD_WORKFLOW_LOAD,
      parseWorkflowLoadParams(params),
      parseWorkflowStatusResult,
    );
  }

  async getWorkflowStatus(): Promise<WorkflowStatusResult> {
    return this.withConnectedRequest(METHOD_WORKFLOW_STATUS, {}, parseWorkflowStatusResult);
  }

  async reloadWorkflow(): Promise<WorkflowStatusResult> {
    return this.withConnectedRequest(METHOD_WORKFLOW_RELOAD, {}, parseWorkflowStatusResult);
  }

  async validateWorkflow(params: WorkflowValidateParams): Promise<WorkflowValidationReport> {
    return this.withConnectedRequest(
      METHOD_WORKFLOW_VALIDATE,
      parseWorkflowValidateParams(params),
      parseWorkflowValidationReport,
    );
  }

  async refreshWorkItems(params: WorkItemRefreshParams): Promise<WorkItemListResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_WORK_ITEM_REFRESH,
      parseWorkItemRefreshParams(params),
      parseWorkItemListResult,
    );
  }

  async dismissWorkItem(params: WorkItemDismissParams): Promise<WorkItemDismissResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_WORK_ITEM_DISMISS,
      parseWorkItemDismissParams(params),
      parseWorkItemDismissResult,
    );
  }

  async triggerWorkItem(params: WorkItemTriggerParams): Promise<WorkItemTriggerResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_WORK_ITEM_TRIGGER,
      parseWorkItemTriggerParams(params),
      parseWorkItemTriggerResult,
    );
  }

  async getRunDetail(runId: RunId): Promise<RunDetail | null> {
    return this.withConnectedRequest(METHOD_DAEMON_RUN_GET, { runId }, parseOptionalRunDetail);
  }

  async getRunTimeline(rootRunId: RunId): Promise<RunTimeline> {
    return this.withConnectedRequest(
      METHOD_DAEMON_RUN_TIMELINE,
      {
        sessionId: this.requireAttachedSessionId(METHOD_DAEMON_RUN_TIMELINE),
        rootRunId,
      },
      parseRunTimeline,
    );
  }

  async replayRunEvents(runId: RunId, afterSeq: bigint | null): Promise<SubscribeRunEventsResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_RUN_REPLAY_EVENTS,
      {
        sessionId: this.requireAttachedSessionId(METHOD_DAEMON_RUN_REPLAY_EVENTS),
        runId,
        afterSeq: afterSeq === null ? undefined : afterSeq.toString(),
      },
      parseSubscribeRunEventsResult,
    );
  }

  async startRun(command: StartRunCommand): Promise<RunSummary> {
    return this.withConnectedRequest(
      METHOD_DAEMON_RUN_START,
      parseStartRunCommand(command),
      parseRunSummary,
    );
  }

  async forkRun(request: ForkRunRequest): Promise<ForkRunResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_RUN_FORK,
      parseForkRunRequest(request),
      parseForkRunResult,
    );
  }

  async decideApproval(
    approvalId: ApprovalId,
    decision: ApprovalDecision,
  ): Promise<DaemonApprovalDecideResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_APPROVAL_DECIDE,
      { approvalId, decision },
      parseDaemonApprovalDecideResult,
    );
  }

  async getActivityPage(query: ActivityPageQuery): Promise<ActivityPageResult> {
    return this.withConnectedRequest(METHOD_DAEMON_ACTIVITY_PAGE, query, parseActivityPageResult);
  }

  async getAgentTurnsPage(query: AgentTurnsPageQuery): Promise<AgentTurnsPageResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_AGENT_TURNS_PAGE,
      query,
      parseAgentTurnsPageResult,
    );
  }

  async listApprovals(query: ListApprovalsQuery): Promise<ApprovalSnapshotResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_APPROVAL_LIST,
      query,
      parseApprovalSnapshotResult,
    );
  }

  async listArtifacts(query: ListArtifactsQuery): Promise<ArtifactSnapshotResult> {
    return this.withConnectedRequest(
      METHOD_DAEMON_ARTIFACT_LIST,
      query,
      parseArtifactSnapshotResult,
    );
  }

  async getArtifact(artifactId: ArtifactId): Promise<ArtifactSummary | null> {
    return this.withConnectedRequest(
      METHOD_DAEMON_ARTIFACT_GET,
      { artifactId },
      parseOptionalArtifactSummary,
    );
  }

  async getSession(): Promise<SessionSummary | null> {
    return this.withConnectedRequest(METHOD_DAEMON_SESSION_GET, {}, parseOptionalSessionSummary);
  }

  async openSession(title: string): Promise<SessionSummary> {
    const result = await this.withConnectedRequest(
      METHOD_DAEMON_SESSION_OPEN,
      { title },
      parseDaemonSessionOpenResult,
    );
    await storeDesktopSessionAuthority("desktop-main", result.session.id, result.sessionAuthority);
    return result.session;
  }

  protected async ensureConnected(): Promise<void> {
    return this.connection.ensureConnected();
  }

  protected enqueueOperation<Result>(operation: () => Promise<Result>): Promise<Result> {
    return this.connection.enqueueOperation(operation);
  }

  protected sendRequest<Result>(
    method: string,
    params: Record<string, unknown>,
    parseResult: (value: unknown) => Result,
  ): Promise<Result> {
    return this.connection.request(method, params, parseResult);
  }

  private withConnectedRequest<Result>(
    method: string,
    params: Record<string, unknown>,
    parseResult: (value: unknown) => Result,
  ): Promise<Result> {
    return this.enqueueOperation(async () => {
      await this.ensureConnected();
      return this.sendRequest(method, params, parseResult);
    });
  }

  private requireAttachedSessionId(method: string): SessionId {
    if (this.attachedSessionId === null) {
      throw new Error(`${method} requires an attached desktop session`);
    }
    return this.attachedSessionId;
  }

  dispose(): void {
    this.connection.dispose();
  }
}

function parseOptionalSessionSummary(value: unknown): SessionSummary | null {
  if (value === null) {
    return null;
  }

  return parseSessionSummary(value);
}

function parseOptionalArtifactSummary(value: unknown): ArtifactSummary | null {
  if (value === null) {
    return null;
  }

  return parseArtifactSummary(value);
}

function parseOptionalRunDetail(value: unknown): RunDetail | null {
  if (value === null) {
    return null;
  }

  return parseRunDetail(value);
}
