use std::{
    io,
    sync::{Arc, Mutex},
};

use serde::{Serialize, de::DeserializeOwned};
use ta_jsonrpc::{
    ClientConfig, JsonRpcClientError, JsonRpcNotificationSubscription, PersistentJsonRpcClient,
};
use ta_protocol::wire::{
    ActivityPageQuery, AgentRuntimeSnapshot, AgentTurnsPageQuery, AgentTurnsPageResult,
    ApprovalSnapshotResult, ArtifactContentResult, ArtifactSnapshotResult, AuthProfileLoginResult,
    AuthProfileLogoutResult, BrowserActionRequest, BrowserActionResult, BrowserClearDataRequest,
    BrowserProfileRequest, BrowserProfileResult, CancelScheduledWorkRequest,
    CodeHostAccountConnectParams, CodeHostAccountConnectResult, CodeHostAccountDisconnectParams,
    CodeHostAccountDisconnectResult, CodeHostAccountListParams, CodeHostAccountListResult,
    CodeHostPage, CodeHostPullRequestActivityParams, CodeHostPullRequestActivityResult,
    CodeHostPullRequestChecksParams, CodeHostPullRequestChecksResult,
    CodeHostPullRequestCommentCreateParams, CodeHostPullRequestCommentCreateResult,
    CodeHostPullRequestDetail, CodeHostPullRequestDetailParams, CodeHostPullRequestEnsureParams,
    CodeHostPullRequestEnsureResult, CodeHostPullRequestListParams, CodeHostPushApplyParams,
    CodeHostPushApplyResult, CodeHostPushPrepareParams, CodeHostPushPrepareResult,
    CodeHostRepositoryContextParams, CodeHostRepositoryContextResult, ContinueRunRequest,
    ContinueRunResult, CreateScheduledWorkRequest, CreateScheduledWorkResult,
    DAEMON_PROTOCOL_VERSION, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimeAuthProfilePreferencesSetParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSetExtensionEnabledParams, DaemonApprovalDecideParams,
    DaemonApprovalDecideResult, DaemonClientCapabilities, DaemonDiagnostics,
    DaemonDiagnosticsParams, DaemonInitializeParams, DaemonInitializeResult,
    DaemonNavigationIntent, DaemonNavigationIntentParams, DaemonNavigationIntentResult,
    DaemonNavigationInvalidatedParams, DaemonNavigationSnapshotParams,
    DaemonNavigationSnapshotResult, DaemonNavigationSubscribeParams,
    DaemonNavigationSubscribeResult, DaemonProjectOpenParams, DaemonProjectOpenResult,
    DaemonSessionAttachParams, DaemonSessionAttachResult, DaemonSessionOpenParams,
    DaemonSessionOpenResult, DaemonSessionSetNextRunSelectionParams, DaemonWorkspaceGetParams,
    DaemonWorkspaceGetResult, DaemonWorkspaceListParams, DaemonWorkspaceListResult,
    DaemonWorkspaceOpenParams, DaemonWorkspaceOpenResult, ForkRunRequest, ForkRunResult,
    GetAgentRuntimeQuery, GetArtifactQuery, GetRunQuery, GetRunTimelineQuery,
    GitCheckpointApplyRevertParams, GitCheckpointListParams, GitCheckpointListResult,
    GitCheckpointPrepareRevertParams, GitCheckpointPrepareRevertResult, GitCommitParams,
    GitDiffParams, GitDiffResult, GitMutationResult, GitPathsMutationParams,
    GitRepositorySnapshotParams, GitRepositorySnapshotResult, InspectPluginPackageRequest,
    InstallPluginPackageRequest, InstallPluginPackageResult, JoinRunRequest, JoinRunResult,
    ListApprovalsQuery, ListArtifactsQuery, ListNativeRunsRequest, ListNativeRunsResult,
    ListPluginInstallationsRequest, ListPluginInstallationsResult, ListRecipesParams,
    ListRunsQuery, ListScheduledWorkRequest, ListScheduledWorkResult, ListSessionsQuery,
    METHOD_DAEMON_ACTIVITY_PAGE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_AGENT_TURNS_PAGE,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_BROWSER_ACTION, METHOD_DAEMON_BROWSER_CLEAR_DATA,
    METHOD_DAEMON_BROWSER_PROFILE, METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT,
    METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT, METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
    METHOD_DAEMON_CODE_HOST_PUSH_PREPARE, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
    METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT, METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT,
    METHOD_DAEMON_GIT_CHECKPOINT_LIST, METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT,
    METHOD_DAEMON_GIT_COMMIT, METHOD_DAEMON_GIT_DIFF, METHOD_DAEMON_GIT_SNAPSHOT,
    METHOD_DAEMON_GIT_STAGE, METHOD_DAEMON_GIT_UNSTAGE, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_NAVIGATION_INTENT, METHOD_DAEMON_NAVIGATION_INVALIDATED,
    METHOD_DAEMON_NAVIGATION_SNAPSHOT, METHOD_DAEMON_NAVIGATION_SUBSCRIBE,
    METHOD_DAEMON_PLUGIN_INSPECT, METHOD_DAEMON_PLUGIN_INSTALL, METHOD_DAEMON_PLUGIN_LIST,
    METHOD_DAEMON_PLUGIN_UNINSTALL, METHOD_DAEMON_PROJECT_OPEN, METHOD_DAEMON_RECIPES_LIST,
    METHOD_DAEMON_RUN_CONTINUE, METHOD_DAEMON_RUN_FORK, METHOD_DAEMON_RUN_GET,
    METHOD_DAEMON_RUN_JOIN, METHOD_DAEMON_RUN_LINEAGE_GRAPH, METHOD_DAEMON_RUN_LIST,
    METHOD_DAEMON_RUN_LIST_NATIVE, METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_SPAWN,
    METHOD_DAEMON_RUN_START, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_RUN_SWITCH_ACCOUNT_AND_RESUME, METHOD_DAEMON_RUN_TIMELINE,
    METHOD_DAEMON_SCHEDULED_WORK_CANCEL, METHOD_DAEMON_SCHEDULED_WORK_CREATE,
    METHOD_DAEMON_SCHEDULED_WORK_LIST, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_LIST,
    METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_SESSION_OVERVIEW,
    METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION, METHOD_DAEMON_TERMINAL_ATTACH,
    METHOD_DAEMON_TERMINAL_CLOSE, METHOD_DAEMON_TERMINAL_DETACH, METHOD_DAEMON_TERMINAL_EVENT,
    METHOD_DAEMON_TERMINAL_INPUT, METHOD_DAEMON_TERMINAL_LIST, METHOD_DAEMON_TERMINAL_RESIZE,
    METHOD_DAEMON_TERMINAL_SPAWN, METHOD_DAEMON_THREAD_WORKSPACE_GET,
    METHOD_DAEMON_THREAD_WORKSPACE_UPDATE, METHOD_DAEMON_VOICE_END, METHOD_DAEMON_VOICE_EXCHANGE,
    METHOD_DAEMON_VOICE_OPEN, METHOD_DAEMON_WORK_ITEM_DISMISS, METHOD_DAEMON_WORK_ITEM_LIST,
    METHOD_DAEMON_WORK_ITEM_REFRESH, METHOD_DAEMON_WORK_ITEM_TRIGGER,
    METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL, METHOD_DAEMON_WORKSPACE_FILE_READ,
    METHOD_DAEMON_WORKSPACE_FILE_TREE, METHOD_DAEMON_WORKSPACE_FILE_WRITE,
    METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST, METHOD_DAEMON_WORKSPACE_OPEN,
    PluginInspection, PublicActivityPageResult, RecipeListResponse, RunDetail, RunEventStreamItem,
    RunEventStreamPayload, RunLineageGraphRequest, RunLineageGraphResult, RunSummary, RunTimeline,
    SessionOverviewQuery, SessionOverviewResult, SessionSummary, SpawnRunRequest, SpawnRunResult,
    StartRunCommand, SubscribeRunEventsRequest, SubscribeRunEventsResult,
    SwitchAccountAndResumeRequest, SwitchAccountAndResumeResult, TerminalAttachParams,
    TerminalAttachResult, TerminalCloseParams, TerminalCloseResult, TerminalDetachParams,
    TerminalDetachResult, TerminalEventParams, TerminalInputParams, TerminalInputResult,
    TerminalListParams, TerminalListResult, TerminalResizeParams, TerminalResizeResult,
    TerminalSessionId, TerminalSpawnParams, TerminalSpawnResult, TerminalStreamEvent,
    ThreadWorkspaceQuery, ThreadWorkspaceResult, ThreadWorkspaceUpdateCommand,
    UninstallPluginRequest, VOICE_FRAME_BYTES, VoiceEvent, VoiceStreamEndParams,
    VoiceStreamEndReason, VoiceStreamEndResult, VoiceStreamExchangeParams,
    VoiceStreamExchangeResult, VoiceStreamOpenParams, VoiceStreamOpenResult, WorkItemDismissParams,
    WorkItemDismissResult, WorkItemListQuery, WorkItemListResult, WorkItemRefreshParams,
    WorkItemTriggerParams, WorkItemTriggerResult, Workspace, WorkspaceFileOpenExternalParams,
    WorkspaceFileOpenExternalResult, WorkspaceFileReadParams, WorkspaceFileReadResult,
    WorkspaceFileTreeParams, WorkspaceFileTreeResult, WorkspaceFileWriteParams,
    WorkspaceFileWriteResult, WorkspaceId, WorkspacePath, decode_voice_audio, encode_voice_audio,
};

use crate::credential_store::remove_session_authority;
use crate::credential_store::{
    load_client_credential, load_session_authority, store_session_authority,
};

pub struct PersistentDaemonClient {
    client_name: String,
    config: ClientConfig,
    transport: PersistentJsonRpcClient,
    /// Serializes the complete load/RPC/remove/store authority rotation across
    /// clones. It deliberately does not cover normal daemon commands.
    authority_rotation: Arc<Mutex<()>>,
}

/// The client-owned result of establishing the one global daemon event
/// subscription. The bridge converts this to a redacted desktop projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonLifecycleSubscriptionState {
    Ready,
}

/// One caller-owned global daemon subscription. It retains daemon lineage and
/// the epoch-aware cursor internally and exposes only the fact that snapshots
/// became invalid or must be rehydrated.
pub struct DaemonLifecycleSubscription {
    notifications: Mutex<JsonRpcNotificationSubscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonLifecycleUpdate {
    Invalidated,
}

impl DaemonLifecycleSubscription {
    pub fn recv(&self) -> Result<DaemonLifecycleUpdate, JsonRpcClientError> {
        loop {
            let notification = self
                .notifications
                .lock()
                .expect("daemon lifecycle notification lock poisoned")
                .recv()
                .map_err(notification_error)?;
            if notification.method != METHOD_DAEMON_NAVIGATION_INVALIDATED {
                continue;
            }
            let params = notification.params.ok_or_else(|| {
                JsonRpcClientError::Deserialize(serde_json::Error::io(io::Error::other(
                    "daemon navigation notification omitted params",
                )))
            })?;
            let _: DaemonNavigationInvalidatedParams =
                serde_json::from_value(params).map_err(JsonRpcClientError::Deserialize)?;
            return Ok(DaemonLifecycleUpdate::Invalidated);
        }
    }
}

/// One caller-owned event subscription. Its receiver and cursor are never
/// shared through the client, so an old quiet subscription cannot consume a
/// newer subscription's notifications.
pub struct RunEventSubscription {
    session_id: ta_protocol::wire::SessionId,
    run_id: ta_protocol::wire::RunId,
    cursor: Mutex<Option<u64>>,
    notifications: Mutex<JsonRpcNotificationSubscription>,
    replay: SubscribeRunEventsResult,
}

pub struct TerminalEventSubscription {
    client: PersistentDaemonClient,
    terminal_id: TerminalSessionId,
    cursor: Mutex<u64>,
    notifications: Mutex<JsonRpcNotificationSubscription>,
    initial: TerminalAttachResult,
}

/// One Rust-only audio stream on the existing persistent daemon connection.
/// Audio packets never enter a public client or generated binding.
#[derive(Clone)]
pub struct VoiceStream {
    client: PersistentDaemonClient,
    run_id: ta_protocol::wire::RunId,
}

pub struct VoiceStreamExchange {
    pub output: Option<[u8; VOICE_FRAME_BYTES]>,
    pub state: Option<VoiceEvent>,
    pub playback_interrupted: bool,
}

impl VoiceStream {
    pub fn exchange(
        &self,
        input: [u8; VOICE_FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<VoiceStreamExchange, JsonRpcClientError> {
        let result: VoiceStreamExchangeResult = self.client.call(
            METHOD_DAEMON_VOICE_EXCHANGE,
            &VoiceStreamExchangeParams {
                run_id: self.run_id.clone(),
                audio_base64: encode_voice_audio(&input),
                playback_completed_frames,
            },
        )?;
        let output = result
            .audio_base64
            .as_deref()
            .map(|value| {
                decode_voice_audio(value).ok_or_else(|| {
                    JsonRpcClientError::Deserialize(serde_json::Error::io(io::Error::other(
                        "daemon returned an invalid voice audio packet",
                    )))
                })
            })
            .transpose()?;
        Ok(VoiceStreamExchange {
            output,
            state: result.state,
            playback_interrupted: result.playback_interrupted,
        })
    }

    pub fn end(&self, reason: VoiceStreamEndReason) -> Result<(), JsonRpcClientError> {
        let _: VoiceStreamEndResult = self.client.call(
            METHOD_DAEMON_VOICE_END,
            &VoiceStreamEndParams {
                run_id: self.run_id.clone(),
                reason,
            },
        )?;
        Ok(())
    }

    pub fn run_id(&self) -> &ta_protocol::wire::RunId {
        &self.run_id
    }
}

impl TerminalEventSubscription {
    pub fn initial(&self) -> &TerminalAttachResult {
        &self.initial
    }

    pub fn recv(&self) -> Result<TerminalEventParams, JsonRpcClientError> {
        loop {
            let notification = self
                .notifications
                .lock()
                .expect("terminal notification lock poisoned")
                .recv()
                .map_err(notification_error)?;
            if notification.method != METHOD_DAEMON_TERMINAL_EVENT {
                continue;
            }
            let params = notification.params.ok_or_else(|| {
                JsonRpcClientError::Deserialize(serde_json::Error::io(io::Error::other(
                    "daemon terminal event notification omitted params",
                )))
            })?;
            let event: TerminalEventParams =
                serde_json::from_value(params).map_err(JsonRpcClientError::Deserialize)?;
            if event.terminal_id != self.terminal_id {
                continue;
            }
            if let TerminalStreamEvent::Output { sequence, .. } = &event.event {
                let mut cursor = self.cursor.lock().expect("terminal cursor lock poisoned");
                if *sequence <= *cursor {
                    continue;
                }
                *cursor = *sequence;
            }
            return Ok(event);
        }
    }

    pub fn detach(&self) -> Result<TerminalDetachResult, JsonRpcClientError> {
        self.client.call(
            METHOD_DAEMON_TERMINAL_DETACH,
            &TerminalDetachParams {
                terminal_id: self.terminal_id.clone(),
            },
        )
    }

    pub fn close_connection(&self) {
        self.client.close();
    }
}

impl RunEventSubscription {
    /// The daemon-validated session identity bound to this subscription.
    pub fn session_id(&self) -> &ta_protocol::wire::SessionId {
        &self.session_id
    }

    pub fn replay(&self) -> &SubscribeRunEventsResult {
        &self.replay
    }

    pub fn recv(&self) -> Result<RunEventStreamItem, JsonRpcClientError> {
        loop {
            let notification = self
                .notifications
                .lock()
                .expect("run notification lock poisoned")
                .recv()
                .map_err(notification_error)?;
            if notification.method != ta_protocol::wire::METHOD_DAEMON_RUN_EVENT {
                continue;
            }
            let params = notification.params.ok_or_else(|| {
                JsonRpcClientError::Deserialize(serde_json::Error::io(io::Error::other(
                    "daemon run event notification omitted params",
                )))
            })?;
            let item: RunEventStreamItem =
                serde_json::from_value(params).map_err(JsonRpcClientError::Deserialize)?;
            // The daemon validates the session at subscribe time. Run identity
            // remains the client-side cursor key because the wire item has no
            // session field by design.
            if item.run_id != self.run_id {
                continue;
            }
            if matches!(&item.payload, RunEventStreamPayload::Delta { delta } if !advance_cursor(&mut self.cursor.lock().expect("run cursor lock poisoned"), delta.seq))
            {
                continue;
            }
            return Ok(item);
        }
    }
}

impl Clone for PersistentDaemonClient {
    fn clone(&self) -> Self {
        Self {
            client_name: self.client_name.clone(),
            config: self.config.clone(),
            transport: self.transport.clone(),
            authority_rotation: Arc::clone(&self.authority_rotation),
        }
    }
}

impl PersistentDaemonClient {
    /// Opens an independently closable connection for one caller-owned stream.
    /// Authentication remains inside the Rust client boundary.
    pub fn fork_connection(&self) -> Result<Self, JsonRpcClientError> {
        let mut client = Self::connect(self.config.clone(), self.client_name.clone())?;
        client.authority_rotation = Arc::clone(&self.authority_rotation);
        client.initialize(
            &self.client_name,
            env!("CARGO_PKG_VERSION"),
            load_client_credential(&self.config, &self.client_name),
        )?;
        Ok(client)
    }

    /// Opens one independently closable connection bound to the requested
    /// session. Session authority loading and rotation remain owned here.
    pub fn fork_session_connection(
        &self,
        session_id: ta_protocol::wire::SessionId,
    ) -> Result<Self, JsonRpcClientError> {
        let mut client = self.fork_connection()?;
        client.attach_session(session_id)?;
        Ok(client)
    }

    pub fn call_public<Params, Response>(
        &self,
        method: &str,
        params: &Params,
    ) -> Result<Response, JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        self.call(method, params)
    }

    pub fn open_voice_stream(
        &self,
        run_id: ta_protocol::wire::RunId,
    ) -> Result<Option<(VoiceStream, VoiceEvent)>, JsonRpcClientError> {
        let result: VoiceStreamOpenResult = self.call(
            METHOD_DAEMON_VOICE_OPEN,
            &VoiceStreamOpenParams {
                run_id: run_id.clone(),
            },
        )?;
        if !result.accepted {
            return Ok(None);
        }
        let state = result.state.ok_or_else(|| {
            JsonRpcClientError::Deserialize(serde_json::Error::io(io::Error::other(
                "daemon accepted a voice stream without public state",
            )))
        })?;
        Ok(Some((
            VoiceStream {
                client: self.clone(),
                run_id,
            },
            state,
        )))
    }
    pub fn connect(config: ClientConfig, client_name: String) -> Result<Self, JsonRpcClientError> {
        Ok(Self {
            client_name,
            transport: PersistentJsonRpcClient::connect(config.clone())?,
            config,
            authority_rotation: Arc::new(Mutex::new(())),
        })
    }

    pub fn initialize(
        &self,
        client_name: &str,
        client_version: &str,
        client_credential: Option<String>,
    ) -> Result<DaemonInitializeResult, JsonRpcClientError> {
        let result: DaemonInitializeResult = self.call(
            METHOD_DAEMON_INITIALIZE,
            &DaemonInitializeParams {
                client_name: client_name.to_string(),
                client_credential,
                client_version: client_version.to_string(),
                protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                capabilities: DaemonClientCapabilities {
                    notifications: true,
                    event_subscriptions: true,
                },
            },
        )?;
        Ok(result)
    }

    pub fn open_session(
        &mut self,
        params: DaemonSessionOpenParams,
    ) -> Result<DaemonSessionOpenResult, JsonRpcClientError> {
        let _rotation = self
            .authority_rotation
            .lock()
            .expect("authority rotation lock poisoned");
        let result: DaemonSessionOpenResult = self.call(METHOD_DAEMON_SESSION_OPEN, &params)?;
        store_session_authority(
            &self.config,
            &self.client_name,
            &result.session.id,
            &result.session_authority,
        )?;
        Ok(result)
    }

    pub fn open_workspace(
        &mut self,
        path: WorkspacePath,
        trust_acknowledged: bool,
    ) -> Result<Workspace, JsonRpcClientError> {
        let result: DaemonWorkspaceOpenResult = self.call(
            METHOD_DAEMON_WORKSPACE_OPEN,
            &DaemonWorkspaceOpenParams {
                path,
                trust_acknowledged,
            },
        )?;
        Ok(result.workspace)
    }

    pub fn list_workspaces(&mut self) -> Result<Vec<Workspace>, JsonRpcClientError> {
        let result: DaemonWorkspaceListResult =
            self.call(METHOD_DAEMON_WORKSPACE_LIST, &DaemonWorkspaceListParams {})?;
        Ok(result.workspaces)
    }

    pub fn get_workspace(&mut self, id: WorkspaceId) -> Result<Workspace, JsonRpcClientError> {
        let result: DaemonWorkspaceGetResult = self.call(
            METHOD_DAEMON_WORKSPACE_GET,
            &DaemonWorkspaceGetParams { id },
        )?;
        Ok(result.workspace)
    }

    pub fn open_project(
        &mut self,
        params: DaemonProjectOpenParams,
    ) -> Result<DaemonProjectOpenResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_PROJECT_OPEN, &params)
    }

    pub fn workspace_file_tree(
        &mut self,
        params: WorkspaceFileTreeParams,
    ) -> Result<WorkspaceFileTreeResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORKSPACE_FILE_TREE, &params)
    }

    pub fn read_workspace_file(
        &mut self,
        params: WorkspaceFileReadParams,
    ) -> Result<WorkspaceFileReadResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORKSPACE_FILE_READ, &params)
    }

    pub fn write_workspace_file(
        &mut self,
        params: WorkspaceFileWriteParams,
    ) -> Result<WorkspaceFileWriteResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORKSPACE_FILE_WRITE, &params)
    }

    pub fn workspace_file_open_external(
        &mut self,
        params: WorkspaceFileOpenExternalParams,
    ) -> Result<WorkspaceFileOpenExternalResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL, &params)
    }

    pub fn code_host_accounts(&mut self) -> Result<CodeHostAccountListResult, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
            &CodeHostAccountListParams {},
        )
    }

    pub fn connect_code_host_account(
        &mut self,
        params: CodeHostAccountConnectParams,
    ) -> Result<CodeHostAccountConnectResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT, &params)
    }

    pub fn disconnect_code_host_account(
        &mut self,
        params: CodeHostAccountDisconnectParams,
    ) -> Result<CodeHostAccountDisconnectResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT, &params)
    }

    pub fn code_host_repository_context(
        &mut self,
        params: CodeHostRepositoryContextParams,
    ) -> Result<CodeHostRepositoryContextResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT, &params)
    }

    pub fn prepare_code_host_push(
        &mut self,
        params: CodeHostPushPrepareParams,
    ) -> Result<CodeHostPushPrepareResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PUSH_PREPARE, &params)
    }

    pub fn apply_code_host_push(
        &mut self,
        params: CodeHostPushApplyParams,
    ) -> Result<CodeHostPushApplyResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PUSH_APPLY, &params)
    }

    pub fn code_host_pull_requests(
        &mut self,
        params: CodeHostPullRequestListParams,
    ) -> Result<CodeHostPage, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, &params)
    }

    pub fn code_host_pull_request_detail(
        &mut self,
        params: CodeHostPullRequestDetailParams,
    ) -> Result<CodeHostPullRequestDetail, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, &params)
    }

    pub fn ensure_code_host_pull_request(
        &mut self,
        params: CodeHostPullRequestEnsureParams,
    ) -> Result<CodeHostPullRequestEnsureResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE, &params)
    }

    pub fn code_host_pull_request_checks(
        &mut self,
        params: CodeHostPullRequestChecksParams,
    ) -> Result<CodeHostPullRequestChecksResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS, &params)
    }

    pub fn code_host_pull_request_activity(
        &mut self,
        params: CodeHostPullRequestActivityParams,
    ) -> Result<CodeHostPullRequestActivityResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY, &params)
    }

    pub fn create_code_host_pull_request_comment(
        &mut self,
        params: CodeHostPullRequestCommentCreateParams,
    ) -> Result<CodeHostPullRequestCommentCreateResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE, &params)
    }

    pub fn git_snapshot(
        &mut self,
        params: GitRepositorySnapshotParams,
    ) -> Result<GitRepositorySnapshotResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_SNAPSHOT, &params)
    }

    pub fn git_diff(&mut self, params: GitDiffParams) -> Result<GitDiffResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_DIFF, &params)
    }

    pub fn git_stage(
        &mut self,
        params: GitPathsMutationParams,
    ) -> Result<GitMutationResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_STAGE, &params)
    }

    pub fn git_unstage(
        &mut self,
        params: GitPathsMutationParams,
    ) -> Result<GitMutationResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_UNSTAGE, &params)
    }

    pub fn git_commit(
        &mut self,
        params: GitCommitParams,
    ) -> Result<GitMutationResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_COMMIT, &params)
    }

    pub fn git_checkpoint_list(
        &mut self,
        params: GitCheckpointListParams,
    ) -> Result<GitCheckpointListResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_CHECKPOINT_LIST, &params)
    }

    pub fn git_checkpoint_prepare_revert(
        &mut self,
        params: GitCheckpointPrepareRevertParams,
    ) -> Result<GitCheckpointPrepareRevertResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT, &params)
    }

    pub fn git_checkpoint_apply_revert(
        &mut self,
        params: GitCheckpointApplyRevertParams,
    ) -> Result<GitMutationResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT, &params)
    }

    pub fn spawn_terminal(
        &mut self,
        params: TerminalSpawnParams,
    ) -> Result<TerminalSpawnResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_TERMINAL_SPAWN, &params)
    }

    pub fn list_terminals(
        &mut self,
        params: TerminalListParams,
    ) -> Result<TerminalListResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_TERMINAL_LIST, &params)
    }

    pub fn terminal_input(
        &mut self,
        params: TerminalInputParams,
    ) -> Result<TerminalInputResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_TERMINAL_INPUT, &params)
    }

    pub fn resize_terminal(
        &mut self,
        params: TerminalResizeParams,
    ) -> Result<TerminalResizeResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_TERMINAL_RESIZE, &params)
    }

    pub fn close_terminal(
        &mut self,
        params: TerminalCloseParams,
    ) -> Result<TerminalCloseResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_TERMINAL_CLOSE, &params)
    }

    pub fn subscribe_terminal(
        &self,
        params: TerminalAttachParams,
    ) -> Result<TerminalEventSubscription, JsonRpcClientError> {
        let (notifications, initial): (JsonRpcNotificationSubscription, TerminalAttachResult) =
            self.transport
                .subscribe_then_call(METHOD_DAEMON_TERMINAL_ATTACH, &params, 256)?;
        Ok(TerminalEventSubscription {
            client: self.clone(),
            terminal_id: params.terminal_id,
            cursor: Mutex::new(initial.latest_sequence),
            notifications: Mutex::new(notifications),
            initial,
        })
    }

    pub fn attach_session(
        &mut self,
        session_id: ta_protocol::wire::SessionId,
    ) -> Result<DaemonSessionAttachResult, JsonRpcClientError> {
        let _rotation = self
            .authority_rotation
            .lock()
            .expect("authority rotation lock poisoned");
        let Some(session_authority) =
            load_session_authority(&self.config, &self.client_name, &session_id)
        else {
            return Err(JsonRpcClientError::Read(io::Error::other(format!(
                "missing local session authority for {}",
                session_id.as_str()
            ))));
        };
        let result: DaemonSessionAttachResult = self
            .call(
                METHOD_DAEMON_SESSION_ATTACH,
                &DaemonSessionAttachParams {
                    session_id: session_id.clone(),
                    session_authority,
                },
            )
            .inspect_err(|error| {
                if is_stale_session_authority_error(error, &session_id) {
                    let _ = remove_session_authority(&self.config, &self.client_name, &session_id);
                }
            })?;
        store_session_authority(
            &self.config,
            &self.client_name,
            &session_id,
            &result.session_authority,
        )?;
        Ok(result)
    }

    pub fn list_sessions(&mut self) -> Result<Vec<SessionSummary>, JsonRpcClientError> {
        self.call(METHOD_DAEMON_SESSION_LIST, &ListSessionsQuery {})
    }

    pub fn set_session_next_run_selection(
        &mut self,
        params: DaemonSessionSetNextRunSelectionParams,
    ) -> Result<SessionSummary, JsonRpcClientError> {
        self.call(METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION, &params)
    }

    pub fn session_overview(
        &mut self,
        query: SessionOverviewQuery,
    ) -> Result<SessionOverviewResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_SESSION_OVERVIEW, &query)
    }

    pub fn navigation_snapshot(
        &mut self,
        search: Option<String>,
    ) -> Result<DaemonNavigationSnapshotResult, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_NAVIGATION_SNAPSHOT,
            &DaemonNavigationSnapshotParams { search },
        )
    }

    pub fn navigation_intent(
        &mut self,
        intent: DaemonNavigationIntent,
    ) -> Result<DaemonNavigationIntentResult, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_NAVIGATION_INTENT,
            &DaemonNavigationIntentParams { intent },
        )
    }

    pub fn list_runs(
        &mut self,
        query: ListRunsQuery,
    ) -> Result<Vec<RunSummary>, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_LIST, &query)
    }

    pub fn list_native_runs(
        &mut self,
        request: ListNativeRunsRequest,
    ) -> Result<ListNativeRunsResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_LIST_NATIVE, &request)
    }
    pub fn run_lineage_graph(
        &mut self,
        request: RunLineageGraphRequest,
    ) -> Result<RunLineageGraphResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_LINEAGE_GRAPH, &request)
    }

    pub fn get_run(&mut self, query: GetRunQuery) -> Result<Option<RunDetail>, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_GET, &query)
    }

    pub fn run_timeline(
        &mut self,
        query: GetRunTimelineQuery,
    ) -> Result<RunTimeline, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_TIMELINE, &query)
    }

    pub fn activity_page(
        &mut self,
        query: ActivityPageQuery,
    ) -> Result<PublicActivityPageResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_ACTIVITY_PAGE, &query)
    }

    pub fn replay_run_events(
        &mut self,
        query: SubscribeRunEventsRequest,
    ) -> Result<SubscribeRunEventsResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_REPLAY_EVENTS, &query)
    }

    pub fn agent_turns_page(
        &mut self,
        query: AgentTurnsPageQuery,
    ) -> Result<AgentTurnsPageResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_TURNS_PAGE, &query)
    }

    pub fn thread_workspace(&mut self) -> Result<ThreadWorkspaceResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_THREAD_WORKSPACE_GET, &ThreadWorkspaceQuery {})
    }

    pub fn update_thread_workspace(
        &mut self,
        command: ThreadWorkspaceUpdateCommand,
    ) -> Result<ThreadWorkspaceResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_THREAD_WORKSPACE_UPDATE, &command)
    }

    pub fn list_approvals(
        &mut self,
        query: ListApprovalsQuery,
    ) -> Result<ApprovalSnapshotResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_APPROVAL_LIST, &query)
    }

    pub fn diagnostics_snapshot(&mut self) -> Result<DaemonDiagnostics, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT,
            &DaemonDiagnosticsParams {},
        )
    }

    pub fn list_recipes(&mut self) -> Result<RecipeListResponse, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RECIPES_LIST, &ListRecipesParams {})
    }

    pub fn browser_profile(&mut self) -> Result<BrowserProfileResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_BROWSER_PROFILE, &BrowserProfileRequest {})
    }
    pub fn browser_action(
        &mut self,
        request: BrowserActionRequest,
    ) -> Result<BrowserActionResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_BROWSER_ACTION, &request)
    }
    pub fn clear_browser_data(
        &mut self,
        request: BrowserClearDataRequest,
    ) -> Result<BrowserActionResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_BROWSER_CLEAR_DATA, &request)
    }

    pub fn list_work_items(
        &mut self,
        query: WorkItemListQuery,
    ) -> Result<WorkItemListResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORK_ITEM_LIST, &query)
    }

    pub fn refresh_work_items(
        &mut self,
        params: WorkItemRefreshParams,
    ) -> Result<WorkItemListResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORK_ITEM_REFRESH, &params)
    }

    pub fn dismiss_work_item(
        &mut self,
        params: WorkItemDismissParams,
    ) -> Result<WorkItemDismissResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORK_ITEM_DISMISS, &params)
    }

    pub fn trigger_work_item(
        &mut self,
        params: WorkItemTriggerParams,
    ) -> Result<WorkItemTriggerResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_WORK_ITEM_TRIGGER, &params)
    }

    pub fn create_scheduled_work(
        &mut self,
        request: CreateScheduledWorkRequest,
    ) -> Result<CreateScheduledWorkResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_SCHEDULED_WORK_CREATE, &request)
    }

    pub fn list_scheduled_work(&mut self) -> Result<ListScheduledWorkResult, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_SCHEDULED_WORK_LIST,
            &ListScheduledWorkRequest {},
        )
    }

    pub fn cancel_scheduled_work(
        &mut self,
        request: CancelScheduledWorkRequest,
    ) -> Result<(), JsonRpcClientError> {
        self.call(METHOD_DAEMON_SCHEDULED_WORK_CANCEL, &request)
    }

    pub fn inspect_plugin_package(
        &mut self,
        request: InspectPluginPackageRequest,
    ) -> Result<PluginInspection, JsonRpcClientError> {
        self.call(METHOD_DAEMON_PLUGIN_INSPECT, &request)
    }

    pub fn install_plugin_package(
        &mut self,
        request: InstallPluginPackageRequest,
    ) -> Result<InstallPluginPackageResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_PLUGIN_INSTALL, &request)
    }

    pub fn list_plugin_installations(
        &mut self,
    ) -> Result<ListPluginInstallationsResult, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_PLUGIN_LIST,
            &ListPluginInstallationsRequest {},
        )
    }

    pub fn uninstall_plugin(
        &mut self,
        request: UninstallPluginRequest,
    ) -> Result<(), JsonRpcClientError> {
        self.call(METHOD_DAEMON_PLUGIN_UNINSTALL, &request)
    }

    pub fn list_artifacts(
        &mut self,
        query: ListArtifactsQuery,
    ) -> Result<ArtifactSnapshotResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_ARTIFACT_LIST, &query)
    }

    pub fn get_artifact(
        &mut self,
        query: GetArtifactQuery,
    ) -> Result<Option<ArtifactContentResult>, JsonRpcClientError> {
        self.call(METHOD_DAEMON_ARTIFACT_GET, &query)
    }

    pub fn start_run(
        &mut self,
        command: StartRunCommand,
    ) -> Result<RunSummary, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_START, &command)
    }

    pub fn fork_run(
        &mut self,
        request: ForkRunRequest,
    ) -> Result<ForkRunResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_FORK, &request)
    }

    pub fn continue_run(
        &mut self,
        request: ContinueRunRequest,
    ) -> Result<ContinueRunResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_CONTINUE, &request)
    }

    pub fn switch_account_and_resume(
        &mut self,
        request: SwitchAccountAndResumeRequest,
    ) -> Result<SwitchAccountAndResumeResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_SWITCH_ACCOUNT_AND_RESUME, &request)
    }

    pub fn spawn_run(
        &mut self,
        request: SpawnRunRequest,
    ) -> Result<SpawnRunResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_SPAWN, &request)
    }

    pub fn join_run(
        &mut self,
        request: JoinRunRequest,
    ) -> Result<JoinRunResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_JOIN, &request)
    }

    pub fn subscribe_run_events(
        &self,
        params: SubscribeRunEventsRequest,
    ) -> Result<RunEventSubscription, JsonRpcClientError> {
        // The transport registers this receiver before writing the subscribe
        // request, so a live notification can never race ahead of replay.
        let (notifications, replay): (JsonRpcNotificationSubscription, SubscribeRunEventsResult) =
            self.transport
                .subscribe_then_call(METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS, &params, 128)?;
        let mut cursor = params.after_seq;
        let events = replay
            .events
            .into_iter()
            .filter(|event| advance_cursor(&mut cursor, event.seq))
            .collect();
        let replay = SubscribeRunEventsResult {
            events,
            latest_event_seq: replay.latest_event_seq,
        };
        Ok(RunEventSubscription {
            session_id: params.session_id,
            run_id: params.run_id,
            cursor: Mutex::new(cursor),
            notifications: Mutex::new(notifications),
            replay,
        })
    }

    /// Establishes the principal-scoped navigation subscription. The transport
    /// installs its receiver before request write, and the empty response is
    /// the boundary after which invalidations may be delivered.
    pub fn subscribe_lifecycle(
        &self,
    ) -> Result<
        (
            DaemonLifecycleSubscription,
            DaemonLifecycleSubscriptionState,
        ),
        JsonRpcClientError,
    > {
        let (notifications, _result): (
            JsonRpcNotificationSubscription,
            DaemonNavigationSubscribeResult,
        ) = self.transport.subscribe_then_call(
            METHOD_DAEMON_NAVIGATION_SUBSCRIBE,
            &DaemonNavigationSubscribeParams {},
            128,
        )?;
        Ok((
            DaemonLifecycleSubscription {
                notifications: Mutex::new(notifications),
            },
            DaemonLifecycleSubscriptionState::Ready,
        ))
    }

    pub fn close(&self) {
        self.transport.close();
    }

    pub fn decide_approval(
        &self,
        params: DaemonApprovalDecideParams,
    ) -> Result<DaemonApprovalDecideResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_APPROVAL_DECIDE, &params)
    }

    pub fn get_agent_runtime(&mut self) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_GET, &GetAgentRuntimeQuery {})
    }

    pub fn patch_agent_runtime_profile(
        &mut self,
        params: DaemonAgentRuntimePatchProfileParams,
    ) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, &params)
    }

    pub fn login_agent_runtime_auth_profile(
        &mut self,
        params: DaemonAgentRuntimeAuthLoginParams,
    ) -> Result<AuthProfileLoginResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN, &params)
    }

    pub fn complete_agent_runtime_auth_profile_login(
        &mut self,
        params: DaemonAgentRuntimeAuthLoginCompleteParams,
    ) -> Result<AuthProfileLoginResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE, &params)
    }

    pub fn logout_agent_runtime_auth_profile(
        &mut self,
        params: DaemonAgentRuntimeAuthLogoutParams,
    ) -> Result<AuthProfileLogoutResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT, &params)
    }

    pub fn replace_agent_runtime_auth_profile_preferences(
        &mut self,
        params: DaemonAgentRuntimeAuthProfilePreferencesSetParams,
    ) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET,
            &params,
        )
    }

    pub fn set_agent_runtime_extension_enabled(
        &mut self,
        params: DaemonAgentRuntimeSetExtensionEnabledParams,
    ) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, &params)
    }

    fn call<Params, Response>(
        &self,
        method: &str,
        params: &Params,
    ) -> Result<Response, JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        self.transport.call(method, params)
    }
}

fn advance_cursor(cursor: &mut Option<u64>, sequence: u64) -> bool {
    if cursor.is_some_and(|current| sequence <= current) {
        return false;
    }
    *cursor = Some(sequence);
    true
}

fn notification_error(error: ta_jsonrpc::NotificationReceiveError) -> JsonRpcClientError {
    match error {
        ta_jsonrpc::NotificationReceiveError::Backpressure => JsonRpcClientError::Backpressure,
        ta_jsonrpc::NotificationReceiveError::ConnectionClosed => {
            JsonRpcClientError::ConnectionClosed
        }
    }
}

fn is_stale_session_authority_error(
    error: &JsonRpcClientError,
    session_id: &ta_protocol::wire::SessionId,
) -> bool {
    matches!(
        error,
        JsonRpcClientError::Remote(remote)
            if remote.error.code == ta_jsonrpc::INVALID_PARAMS_ERROR_CODE
                && remote.error.message == format!("session does not exist: {}", session_id.as_str())
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::thread;
    use ta_jsonrpc::{
        INVALID_PARAMS_ERROR_CODE, JsonLineCodec, JsonRpcError, JsonRpcErrorObject, JsonRpcMessage,
        JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, bind_listener,
    };
    use ta_protocol::wire::{
        AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeSelection, AgentRuntimeSnapshot,
        AgentRuntimeStrategyHealth, AgentRuntimeStrategyHealthStatus, AgentRuntimeStrategyId,
        AgentRuntimeStrategyInfo, ApprovalAttentionState, AuthMethodId, AuthMethodRef,
        AuthProfileConnectionState, AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult,
        AuthProfileRef, AuthProfileState, DaemonAgentRuntimeAuthLoginParams,
        DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimePatchProfileParams,
        DaemonAgentRuntimeSetExtensionEnabledParams, METHOD_DAEMON_THREAD_WORKSPACE_GET,
        METHOD_DAEMON_THREAD_WORKSPACE_UPDATE, PublicDaemonEvent, RunEvent, RunEventDelta,
        RunEventStreamItem, RunEventStreamPayload, RunId, RunStatus, RunSummary,
        RuntimeExtensionAvailability, RuntimeExtensionDescriptor, RuntimeExtensionId,
        RuntimeExtensionState, RuntimePolicyMode, RuntimeProfileId, RuntimeProfilePatch,
        RuntimeProfileSummary, SessionId, SessionOverview, SessionOverviewLaneStatus,
        SessionOverviewQuery, SessionOverviewResult, SessionStatus, ThreadWorkspaceMutation,
        ThreadWorkspaceResult, ThreadWorkspaceUpdateCommand, WorkspaceId,
    };

    use super::*;
    use crate::credential_store::{load_session_authority, store_session_authority};

    #[test]
    fn scheduled_work_transport_uses_canonical_rpc_methods() {
        assert_eq!(
            ta_protocol::wire::METHOD_DAEMON_SCHEDULED_WORK_CREATE,
            "daemon.scheduled_work.create"
        );
        assert_eq!(
            ta_protocol::wire::METHOD_DAEMON_SCHEDULED_WORK_LIST,
            "daemon.scheduled_work.list"
        );
        assert_eq!(
            ta_protocol::wire::METHOD_DAEMON_SCHEDULED_WORK_CANCEL,
            "daemon.scheduled_work.cancel"
        );
    }

    #[test]
    fn session_open_persists_authority_while_public_summary_stays_redacted() {
        let socket_name = format!("ta-daemon-client-open-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-open").expect("session id");
        let session_id_server = session_id.clone();
        let authority = ta_protocol::wire::SessionAuthority::new(
            "session-authority-open-session-authority".to_string(),
        )
        .expect("authority");
        let authority_server = authority.clone();
        let expected_params = DaemonSessionOpenParams {
            title: "Native session".to_string(),
            workspace: ta_protocol::wire::WorkspaceSelector::ById {
                id: WorkspaceId::new("workspace-open").expect("workspace id"),
            },
        };
        let expected_params_server = expected_params.clone();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("connection");
            let request = read_request(&mut stream);
            assert_eq!(request.method, METHOD_DAEMON_SESSION_OPEN);
            let params: DaemonSessionOpenParams =
                serde_json::from_value(request.params.expect("params")).expect("open params");
            assert_eq!(params, expected_params_server);
            write_response(
                &mut stream,
                request.id,
                DaemonSessionOpenResult {
                    session: SessionSummary {
                        id: session_id_server,
                        title: "Native session".to_string(),
                        status: SessionStatus::Idle,
                        next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
                    },
                    latest_cursor: None,
                    session_authority: authority_server,
                },
            );
        });

        let mut client =
            PersistentDaemonClient::connect(config.clone(), "taugentic-desktop".into())
                .expect("client");
        let result = client.open_session(expected_params).expect("open session");
        assert_eq!(
            load_session_authority(&config, "taugentic-desktop", &session_id),
            Some(authority)
        );
        let public_summary = serde_json::to_string(&result.session).expect("summary JSON");
        assert!(!public_summary.contains("authority"));
        assert!(!public_summary.contains("session-authority"));
        client.close();
        server.join().expect("server join");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn thread_workspace_client_uses_canonical_methods_and_typed_results() {
        let socket_name = format!("ta-daemon-client-thread-workspace-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-thread-workspace").expect("session id");
        let result = ThreadWorkspaceResult {
            session_id: session_id.clone(),
            goal: "goal".to_string(),
            plan: String::new(),
            notes: String::new(),
            recap: String::new(),
            pins: Vec::new(),
            work_log: Vec::new(),
        };
        let server_result = result.clone();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("connection");
            let get = read_request(&mut stream);
            assert_eq!(get.method, METHOD_DAEMON_THREAD_WORKSPACE_GET);
            assert_eq!(get.params, Some(serde_json::json!({})));
            write_response(&mut stream, get.id, server_result.clone());
            let update = read_request(&mut stream);
            assert_eq!(update.method, METHOD_DAEMON_THREAD_WORKSPACE_UPDATE);
            let command: ThreadWorkspaceUpdateCommand =
                serde_json::from_value(update.params.expect("update params"))
                    .expect("typed update params");
            assert_eq!(
                command.mutation,
                ThreadWorkspaceMutation::GoalSet {
                    value: "next".to_string()
                }
            );
            write_response(&mut stream, update.id, server_result);
        });
        let mut client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");
        assert_eq!(client.thread_workspace().expect("get result"), result);
        assert_eq!(
            client
                .update_thread_workspace(ThreadWorkspaceUpdateCommand {
                    mutation: ThreadWorkspaceMutation::GoalSet {
                        value: "next".to_string()
                    }
                })
                .expect("update result"),
            result
        );
        client.close();
        server.join().expect("server should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn attach_session_purges_stale_local_authority_after_terminal_remote_denial() {
        let socket_name = format!("ta-daemon-client-attach-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-1").expect("session id");
        let authority = ta_protocol::wire::SessionAuthority::new(
            "session-authority-1session-authority-1".to_string(),
        )
        .expect("session authority");
        store_session_authority(&config, "ta-cli", &session_id, &authority)
            .expect("session authority should persist");
        let server_session_id = session_id.clone();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            assert_eq!(request.method, METHOD_DAEMON_SESSION_ATTACH);
            let error_line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Error(JsonRpcError::new(
                    Some(request.id),
                    JsonRpcErrorObject::new(
                        INVALID_PARAMS_ERROR_CODE,
                        format!("session does not exist: {}", server_session_id.as_str()),
                    ),
                )))
                .expect("error should encode");
            reader
                .get_mut()
                .write_all(error_line.as_bytes())
                .expect("error should write");
            reader.get_mut().flush().expect("error should flush");
        });

        let mut client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");
        let error = client
            .attach_session(session_id.clone())
            .expect_err("stale authority should fail");
        assert!(matches!(error, JsonRpcClientError::Remote(_)));
        assert_eq!(load_session_authority(&config, "ta-cli", &session_id), None);
        let second_error = client
            .attach_session(session_id.clone())
            .expect_err("missing local authority should fail locally");
        assert!(
            second_error
                .to_string()
                .contains("missing local session authority for session-1")
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn attach_session_persists_rotated_authority_for_next_successful_reattach() {
        let socket_name = format!("ta-daemon-client-attach-success-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-1").expect("session id");
        let authority_a1 = ta_protocol::wire::SessionAuthority::new(
            "session-authority-1session-authority-1".to_string(),
        )
        .expect("session authority");
        let authority_a2 = ta_protocol::wire::SessionAuthority::new(
            "session-authority-2session-authority-2".to_string(),
        )
        .expect("session authority");
        let authority_a3 = ta_protocol::wire::SessionAuthority::new(
            "session-authority-3session-authority-3".to_string(),
        )
        .expect("session authority");
        store_session_authority(&config, "ta-cli", &session_id, &authority_a1)
            .expect("session authority should persist");
        let server_session_id = session_id.clone();
        let authority_a1_server = authority_a1.clone();
        let authority_a2_server = authority_a2.clone();
        let authority_a3_server = authority_a3.clone();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            for (index, expected_authority, next_authority, next_sequence) in [
                (
                    1_i64,
                    authority_a1_server,
                    authority_a2_server.clone(),
                    11_u64,
                ),
                (2_i64, authority_a2_server, authority_a3_server, 13_u64),
            ] {
                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
                    .expect("request should read");
                let request = match JsonLineCodec
                    .decode_message(&request_line)
                    .expect("request should decode")
                {
                    JsonRpcMessage::Request(request) => request,
                    other => panic!("expected request, got {other:?}"),
                };
                assert_eq!(request.method, METHOD_DAEMON_SESSION_ATTACH);
                let params: DaemonSessionAttachParams =
                    serde_json::from_value(request.params.expect("attach params should exist"))
                        .expect("attach params should deserialize");
                assert_eq!(params.session_id, server_session_id);
                assert_eq!(params.session_authority, expected_authority);
                let response_line = JsonLineCodec
                    .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                        request.id,
                        serde_json::to_value(DaemonSessionAttachResult {
                            session: SessionSummary {
                                id: server_session_id.clone(),
                                title: "Build daemon app server".to_string(),
                                status: SessionStatus::Running,
                                next_run_selection:
                                    ta_protocol::wire::SessionNextRunSelection::Unselected,
                            },
                            latest_cursor: Some(ta_protocol::wire::DaemonEventCursor {
                                daemon_instance_id: "daemon-1".to_string(),
                                session_id: server_session_id.clone(),
                                sequence: next_sequence,
                            }),
                            session_authority: next_authority,
                        })
                        .expect("attach result should serialize"),
                    )))
                    .expect("response should encode");
                reader
                    .get_mut()
                    .write_all(response_line.as_bytes())
                    .expect("response should write");
                reader.get_mut().flush().expect("response should flush");
                assert!(index >= 1);
            }
        });

        let mut client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");
        let attached_once = client
            .attach_session(session_id.clone())
            .expect("first attach should succeed");
        assert_eq!(attached_once.session_authority, authority_a2);
        assert_eq!(
            load_session_authority(&config, "ta-cli", &session_id),
            Some(authority_a2.clone())
        );

        let attached_twice = client
            .attach_session(session_id.clone())
            .expect("second attach should reuse rotated authority");
        assert_eq!(attached_twice.session_authority, authority_a3);
        assert_eq!(
            load_session_authority(&config, "ta-cli", &session_id),
            Some(authority_a3)
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn cloned_attach_serializes_authority_rotation_before_next_request() {
        let socket_name = format!("ta-daemon-client-attach-concurrent-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session = SessionId::new("session-concurrent").expect("session");
        let a1 = ta_protocol::wire::SessionAuthority::new("session-authority-concurrent-a1")
            .expect("a1");
        let a2 = ta_protocol::wire::SessionAuthority::new("session-authority-concurrent-a2")
            .expect("a2");
        let a3 = ta_protocol::wire::SessionAuthority::new("session-authority-concurrent-a3")
            .expect("a3");
        store_session_authority(&config, "ta-cli", &session, &a1).expect("store a1");
        let expected_session = session.clone();
        let expected_a1 = a1.clone();
        let expected_a2 = a2.clone();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one connection");
            for (expected, next) in [(expected_a1, a2), (expected_a2, a3)] {
                let request = read_request(&mut stream);
                let params: DaemonSessionAttachParams =
                    serde_json::from_value(request.params.expect("params")).expect("attach params");
                assert_eq!(params.session_id, expected_session);
                assert_eq!(params.session_authority, expected);
                write_response(
                    &mut stream,
                    request.id,
                    DaemonSessionAttachResult {
                        session: SessionSummary {
                            id: expected_session.clone(),
                            title: "test".into(),
                            status: ta_protocol::wire::SessionStatus::Idle,
                            next_run_selection:
                                ta_protocol::wire::SessionNextRunSelection::Unselected,
                        },
                        session_authority: next,
                        latest_cursor: None,
                    },
                );
            }
        });
        let client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".into()).expect("connect");
        let mut left = client.clone();
        let mut right = client.clone();
        let left_session = session.clone();
        let first = thread::spawn(move || left.attach_session(left_session));
        let second = thread::spawn(move || right.attach_session(session));
        first.join().expect("first join").expect("first attach");
        second.join().expect("second join").expect("second attach");
        client.close();
        server.join().expect("server join");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn forked_session_connection_initializes_then_attaches_before_stream_use() {
        let socket_name = format!("ta-daemon-client-fork-session-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-stream").expect("session id");
        let authority_a1 =
            ta_protocol::wire::SessionAuthority::new("session-authority-stream-a1".to_string())
                .expect("a1");
        let authority_a2 =
            ta_protocol::wire::SessionAuthority::new("session-authority-stream-a2".to_string())
                .expect("a2");
        store_session_authority(&config, "ta-cli", &session_id, &authority_a1).expect("store a1");
        let server_session_id = session_id.clone();
        let server_a1 = authority_a1.clone();
        let server_a2 = authority_a2.clone();

        let server = thread::spawn(move || {
            let mut base_connection = listener.accept().expect("base connection");
            let base_initialize = read_request(&mut base_connection);
            assert_eq!(base_initialize.method, METHOD_DAEMON_INITIALIZE);
            write_response(
                &mut base_connection,
                base_initialize.id,
                initialized_daemon("daemon-base-session"),
            );

            let mut stream = listener.accept().expect("forked connection");

            let initialize = read_request(&mut stream);
            assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
            write_response(
                &mut stream,
                initialize.id,
                initialized_daemon("daemon-fork-session"),
            );

            let attach = read_request(&mut stream);
            assert_eq!(attach.method, METHOD_DAEMON_SESSION_ATTACH);
            let params: DaemonSessionAttachParams =
                serde_json::from_value(attach.params.expect("attach params"))
                    .expect("attach params should decode");
            assert_eq!(params.session_id, server_session_id);
            assert_eq!(params.session_authority, server_a1);
            write_response(
                &mut stream,
                attach.id,
                DaemonSessionAttachResult {
                    session: SessionSummary {
                        id: server_session_id,
                        title: "Stream session".to_string(),
                        status: SessionStatus::Running,
                        next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
                    },
                    session_authority: server_a2,
                    latest_cursor: None,
                },
            );
        });

        let client = PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string())
            .expect("base client");
        client
            .initialize("ta-cli", "test", None)
            .expect("initialize base client");
        let stream_client = client
            .fork_session_connection(session_id.clone())
            .expect("forked session client");
        assert_eq!(
            load_session_authority(&config, "ta-cli", &session_id),
            Some(authority_a2)
        );

        stream_client.close();
        client.close();
        server.join().expect("server join");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn session_overview_roundtrips_the_canonical_visualizer_surface() {
        let socket_name = format!("ta-daemon-client-overview-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-1").expect("session id");
        let session_id_server = session_id.clone();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            assert_eq!(request.method, METHOD_DAEMON_SESSION_OVERVIEW);
            let params: SessionOverviewQuery =
                serde_json::from_value(request.params.expect("overview params should exist"))
                    .expect("overview params should deserialize");
            assert_eq!(params.recent_activity_limit, 3);
            let response_line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    serde_json::to_value(SessionOverviewResult {
                        sessions: vec![SessionOverview {
                            session: SessionSummary {
                                id: session_id_server,
                                title: "Build daemon app server".to_string(),
                                status: SessionStatus::Running,
                                next_run_selection:
                                    ta_protocol::wire::SessionNextRunSelection::Unselected,
                            },
                            latest_run: Some(RunSummary {
                                id: ta_protocol::wire::RunId::new("run-1").expect("run id"),
                                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new(
                                    "runtime-codex-safe",
                                )
                                .expect("runtime profile id"),
                                objective: "Build daemon app server".to_string(),
                                status: RunStatus::WaitingForApproval,
                            }),
                            lane_status: SessionOverviewLaneStatus::WaitingForApproval,
                            is_active: true,
                            approval_attention: ApprovalAttentionState::Pending,
                            pending_approval_count: 1,
                            last_activity_at_ms: Some(42),
                            last_event_preview: Some("Approval requested: execute run".to_string()),
                            recent_activity: Vec::new(),
                        }],
                    })
                    .expect("overview result should serialize"),
                )))
                .expect("response should encode");
            reader
                .get_mut()
                .write_all(response_line.as_bytes())
                .expect("response should write");
            reader.get_mut().flush().expect("response should flush");
        });

        let mut client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");
        let result = client
            .session_overview(SessionOverviewQuery {
                recent_activity_limit: 3,
            })
            .expect("session overview should succeed");

        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].session.id, session_id);
        assert_eq!(
            result.sessions[0].latest_run.as_ref().map(|run| run.status),
            Some(RunStatus::WaitingForApproval)
        );
        assert_eq!(
            result.sessions[0].lane_status,
            SessionOverviewLaneStatus::WaitingForApproval
        );

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn agent_runtime_methods_roundtrip_the_canonical_runtime_surface() {
        let socket_name = format!("ta-daemon-client-agent-runtime-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("listener should accept");
            let mut reader = BufReader::new(&mut stream);
            let expected_snapshot = AgentRuntimeSnapshot {
                providers: vec![AgentRuntimeStrategyInfo {
                    id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                    display_name: "Codex".to_string(),
                    models: vec![AgentRuntimeModelRef {
                        id: AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id"),
                        display_name: "GPT-5.6 Sol".to_string(),
                        context_limit: None,
                        input_cost_per_million_micros: None,
                        output_cost_per_million_micros: None,
                        reasoning: true,
                        tool_call: true,
                        structured_output: false,
                        media_capabilities: ta_protocol::wire::AgentRuntimeMediaCapabilities {
                            image_input: ta_protocol::wire::AgentRuntimeMediaCapability::Supported,
                            image_output: ta_protocol::wire::AgentRuntimeMediaCapability::Supported,
                            voice_input:
                                ta_protocol::wire::AgentRuntimeMediaCapability::Unsupported,
                            voice_output:
                                ta_protocol::wire::AgentRuntimeMediaCapability::Unsupported,
                        },
                    }],
                    model_capability: ta_protocol::wire::AgentRuntimeModelCapability {
                        availability: ta_protocol::wire::AgentRuntimeModelAvailability::Enumerated,
                        can_set_model: true,
                        current_model_id: None,
                        detail: None,
                    },
                    health: AgentRuntimeStrategyHealth {
                        status: AgentRuntimeStrategyHealthStatus::Ready,
                        message: Some("codex runtime ready".to_string()),
                    },
                }],
                auth_methods: vec![AuthMethodRef {
                    id: AuthMethodId::new("method-test").expect("auth method id"),
                    provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                    display_name: "Test method".to_string(),
                    management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    supports_multiple_profiles: true,
                }],
                auth_profiles: vec![AuthProfileState {
                    profile: AuthProfileRef {
                        id: AuthProfileId::new("profile-test").expect("auth profile id"),
                        auth_method_id: AuthMethodId::new("method-test").expect("auth method id"),
                        provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                        display_name: "Test profile".to_string(),
                        account_hint: None,
                        plan_tier: None,
                    },
                    preferences: ta_protocol::wire::AuthProfilePreferences {
                        label: "Codex ChatGPT".to_string(),
                        order: 0,
                        is_default: true,
                    },
                    usage: ta_protocol::wire::AuthProfileUsage::Unavailable,
                    exhaustion: None,
                    connection_state: AuthProfileConnectionState::Connected,
                    last_error: None,
                    management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    can_login: true,
                    can_logout: true,
                    platform_org_linked: None,
                    setup_steps: Vec::new(),
                    action: None,
                    methods: vec![ta_protocol::wire::AuthProfileMethodInfo {
                        id: "method-test".to_string(),
                        display_name: "Codex ChatGPT".to_string(),
                        management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    }],
                }],
                runtime_profiles: vec![RuntimeProfileSummary {
                    id: RuntimeProfileId::new("runtime-codex-safe").expect("runtime profile id"),
                    display_name: "Codex Safe".to_string(),
                    provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                    auth_method_id: Some(AuthMethodId::new("method-test").expect("auth method id")),
                    policy_mode: RuntimePolicyMode::RequireApproval,
                    execution_kind: ta_protocol::wire::RuntimeProfileExecutionKind::AgentRun,
                }],
                runtime_extensions: vec![RuntimeExtensionState {
                    descriptor: RuntimeExtensionDescriptor {
                        id: RuntimeExtensionId::new("local-shell-tools").expect("extension id"),
                        display_name: "Local Shell Tools".to_string(),
                        description: "Builtin local shell execution support".to_string(),
                    },
                    availability: RuntimeExtensionAvailability::Available,
                    enabled: false,
                    mcp_server: None,
                }],
            };
            let mut expected_patched_snapshot = expected_snapshot.clone();
            expected_patched_snapshot.runtime_profiles[0].policy_mode = RuntimePolicyMode::Allow;
            let expected_login = AuthProfileLoginResult {
                auth_profile: AuthProfileState {
                    profile: AuthProfileRef {
                        id: AuthProfileId::new("profile-test").expect("auth profile id"),
                        auth_method_id: AuthMethodId::new("method-test").expect("auth method id"),
                        provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                        display_name: "Test profile".to_string(),
                        account_hint: None,
                        plan_tier: None,
                    },
                    preferences: ta_protocol::wire::AuthProfilePreferences {
                        label: "Codex ChatGPT".to_string(),
                        order: 0,
                        is_default: true,
                    },
                    usage: ta_protocol::wire::AuthProfileUsage::Unavailable,
                    exhaustion: None,
                    connection_state: AuthProfileConnectionState::Connected,
                    last_error: None,
                    management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    can_login: true,
                    can_logout: true,
                    platform_org_linked: None,
                    setup_steps: Vec::new(),
                    action: None,
                    methods: vec![ta_protocol::wire::AuthProfileMethodInfo {
                        id: "method-test".to_string(),
                        display_name: "Codex ChatGPT".to_string(),
                        management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    }],
                },
                challenge: None,
            };
            let expected_logout = AuthProfileLogoutResult {
                auth_profile_id: AuthProfileId::new("profile-test").expect("auth profile id"),
                disconnected: true,
            };

            for (expected_method, respond) in [
                (
                    METHOD_DAEMON_AGENT_RUNTIME_GET,
                    serde_json::to_value(expected_snapshot.clone()).expect("snapshot"),
                ),
                (
                    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH,
                    serde_json::to_value(expected_patched_snapshot.clone()).expect("snapshot"),
                ),
                (
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
                    serde_json::to_value(expected_login.clone()).expect("login result"),
                ),
                (
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
                    serde_json::to_value(expected_logout.clone()).expect("logout result"),
                ),
                (
                    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
                    serde_json::to_value(expected_snapshot.clone()).expect("snapshot"),
                ),
            ] {
                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
                    .expect("request should read");
                let request = match JsonLineCodec
                    .decode_message(&request_line)
                    .expect("request should decode")
                {
                    JsonRpcMessage::Request(request) => request,
                    other => panic!("expected request, got {other:?}"),
                };
                assert_eq!(request.method, expected_method);
                let params = request.params.clone().expect("params should exist");
                match expected_method {
                    METHOD_DAEMON_AGENT_RUNTIME_GET => {
                        let query: GetAgentRuntimeQuery =
                            serde_json::from_value(params).expect("get params should deserialize");
                        assert_eq!(query, GetAgentRuntimeQuery::default());
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH => {
                        let params: DaemonAgentRuntimePatchProfileParams =
                            serde_json::from_value(params)
                                .expect("patch params should deserialize");
                        assert_eq!(params.runtime_profile_id.as_str(), "runtime-codex-safe");
                        assert_eq!(params.patch.policy_mode, Some(RuntimePolicyMode::Allow));
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN => {
                        let params: DaemonAgentRuntimeAuthLoginParams =
                            serde_json::from_value(params)
                                .expect("login params should deserialize");
                        assert_eq!(params.auth_method_id.as_str(), "method-test");
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT => {
                        let params: DaemonAgentRuntimeAuthLogoutParams =
                            serde_json::from_value(params)
                                .expect("logout params should deserialize");
                        assert_eq!(params.auth_profile_id.as_str(), "profile-test");
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET => {
                        let params: DaemonAgentRuntimeSetExtensionEnabledParams =
                            serde_json::from_value(params)
                                .expect("extension params should deserialize");
                        assert_eq!(params.extension_id.as_str(), "local-shell-tools");
                        assert!(!params.enabled);
                    }
                    other => panic!("unexpected method under test: {other}"),
                }

                let response_line = JsonLineCodec
                    .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                        request.id, respond,
                    )))
                    .expect("response should encode");
                reader
                    .get_mut()
                    .write_all(response_line.as_bytes())
                    .expect("response should write");
                reader.get_mut().flush().expect("response should flush");
            }
        });

        let mut client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");

        let snapshot = client.get_agent_runtime().expect("get should succeed");
        assert_eq!(snapshot.runtime_profiles.len(), 1);

        let patched = client
            .patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
                runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                patch: RuntimeProfilePatch {
                    policy_mode: Some(RuntimePolicyMode::Allow),
                    ..Default::default()
                },
            })
            .expect("patch should succeed");
        assert_eq!(
            patched.runtime_profiles[0].policy_mode,
            RuntimePolicyMode::Allow
        );

        let login = client
            .login_agent_runtime_auth_profile(DaemonAgentRuntimeAuthLoginParams {
                auth_method_id: AuthMethodId::new("method-test").expect("auth method id"),
            })
            .expect("login should succeed");
        assert_eq!(login.auth_profile.profile.id.as_str(), "profile-test");

        let logout = client
            .logout_agent_runtime_auth_profile(DaemonAgentRuntimeAuthLogoutParams {
                auth_profile_id: AuthProfileId::new("profile-test").expect("auth profile id"),
            })
            .expect("logout should succeed");
        assert!(logout.disconnected);

        let toggled = client
            .set_agent_runtime_extension_enabled(DaemonAgentRuntimeSetExtensionEnabledParams {
                extension_id: RuntimeExtensionId::new("local-shell-tools").expect("extension id"),
                enabled: false,
            })
            .expect("extension set should succeed");
        assert!(!toggled.runtime_extensions[0].enabled);

        server.join().expect("server thread should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn subscription_buffers_pre_response_live_event_until_replay_cursor_is_committed() {
        let socket_name = format!("ta-daemon-client-subscribe-splice-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-splice").expect("session id");
        let run_id = RunId::new("run-splice").expect("run id");
        let server_session_id = session_id.clone();
        let server_run_id = run_id.clone();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one persistent connection");
            let request = read_request(&mut stream);
            assert_eq!(request.method, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS);
            let params: SubscribeRunEventsRequest =
                serde_json::from_value(request.params.expect("subscribe params should exist"))
                    .expect("subscribe params should decode");
            assert_eq!(params.session_id, server_session_id);
            assert_eq!(params.run_id, server_run_id);

            write_notification(
                &mut stream,
                run_event_notification(run_event_item(&server_run_id, 11)),
            );
            write_response(
                &mut stream,
                request.id,
                SubscribeRunEventsResult {
                    events: vec![run_event_delta(&server_run_id, 10)],
                    latest_event_seq: Some(10),
                },
            );
            release_receiver
                .recv()
                .expect("release server after assertions");
        });

        let client = PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string())
            .expect("client should connect");
        let subscription = client
            .subscribe_run_events(SubscribeRunEventsRequest {
                session_id,
                run_id,
                after_seq: None,
            })
            .expect("subscribe should return replay after committing cursor");
        assert_eq!(
            subscription
                .replay()
                .events
                .iter()
                .map(|event| event.seq)
                .collect::<Vec<_>>(),
            vec![10]
        );
        assert_eq!(
            subscription.recv().expect("buffered live event"),
            run_event_item(&RunId::new("run-splice").expect("run id"), 11)
        );

        release_sender.send(()).expect("release server");
        server.join().expect("server should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn mismatched_run_event_does_not_advance_active_u64_cursor() {
        let socket_name = format!("ta-daemon-client-run-filter-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-filter").expect("session id");
        let run_id = RunId::new("run-active").expect("run id");
        let other_run_id = RunId::new("run-other").expect("other run id");
        let server_session_id = session_id.clone();
        let server_run_id = run_id.clone();
        let server_other_run_id = other_run_id.clone();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one persistent connection");
            let request = read_request(&mut stream);
            let params: SubscribeRunEventsRequest =
                serde_json::from_value(request.params.expect("subscribe params should exist"))
                    .expect("subscribe params should decode");
            assert_eq!(params.session_id, server_session_id);
            assert_eq!(params.run_id, server_run_id);
            assert_eq!(params.after_seq, Some(8));
            write_response(
                &mut stream,
                request.id,
                SubscribeRunEventsResult {
                    events: Vec::new(),
                    latest_event_seq: Some(8),
                },
            );
            write_notification(
                &mut stream,
                run_event_notification(run_event_item(&server_other_run_id, 9)),
            );
            write_notification(
                &mut stream,
                run_event_notification(run_event_item(&server_run_id, 9)),
            );
            release_receiver
                .recv()
                .expect("release server after assertions");
        });

        let client = PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string())
            .expect("client should connect");
        let subscription = client
            .subscribe_run_events(SubscribeRunEventsRequest {
                session_id,
                run_id: run_id.clone(),
                after_seq: Some(8),
            })
            .expect("subscribe should succeed");
        let delivered = subscription
            .recv()
            .expect("matching sequence should remain valid");
        assert_eq!(delivered.run_id, run_id);
        assert_eq!(event_sequence(&delivered), Some(9));

        release_sender.send(()).expect("release server");
        server.join().expect("server should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn quiet_old_subscription_and_new_subscription_do_not_cross_consume() {
        let socket_name = format!("ta-daemon-client-two-streams-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session = SessionId::new("session-streams").expect("session");
        let old_run = RunId::new("run-old").expect("old run");
        let new_run = RunId::new("run-new").expect("new run");
        let server_new = new_run.clone();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one persistent connection");
            let first = read_request(&mut stream);
            write_response(
                &mut stream,
                first.id,
                SubscribeRunEventsResult {
                    events: Vec::new(),
                    latest_event_seq: None,
                },
            );
            let second = read_request(&mut stream);
            write_response(
                &mut stream,
                second.id,
                SubscribeRunEventsResult {
                    events: Vec::new(),
                    latest_event_seq: None,
                },
            );
            write_notification(
                &mut stream,
                run_event_notification(run_event_item(&server_new, 1)),
            );
            release_receiver
                .recv()
                .expect("release peer after assertions");
        });
        let client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".into()).expect("connect");
        let old = client
            .subscribe_run_events(SubscribeRunEventsRequest {
                session_id: session.clone(),
                run_id: old_run,
                after_seq: None,
            })
            .expect("old subscribe");
        let (old_started_sender, old_started_receiver) = std::sync::mpsc::channel();
        let old_waiter = thread::spawn(move || {
            old_started_sender.send(()).expect("old waiter started");
            old.recv()
        });
        old_started_receiver.recv().expect("old waiter is blocked");
        let new = client
            .subscribe_run_events(SubscribeRunEventsRequest {
                session_id: session,
                run_id: new_run.clone(),
                after_seq: None,
            })
            .expect("new subscribe");
        assert_eq!(new.recv().expect("new stream event").run_id, new_run);
        client.close();
        assert!(matches!(
            old_waiter.join().expect("old waiter join"),
            Err(JsonRpcClientError::ConnectionClosed)
        ));
        release_sender.send(()).expect("release peer");
        server.join().expect("server join");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn cancel_completes_on_the_same_connection_while_event_thread_waits_for_live_data() {
        let socket_name = format!("ta-daemon-client-cancel-mux-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let session_id = SessionId::new("session-cancel").expect("session id");
        let run_id = RunId::new("run-cancel").expect("run id");
        let server_session_id = session_id.clone();
        let server_run_id = run_id.clone();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();

        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one persistent connection");
            let subscribe = read_request(&mut stream);
            assert_eq!(subscribe.method, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS);
            let params: SubscribeRunEventsRequest =
                serde_json::from_value(subscribe.params.expect("subscribe params should exist"))
                    .expect("subscribe params should decode");
            assert_eq!(params.session_id, server_session_id);
            assert_eq!(params.run_id, server_run_id);
            write_response(
                &mut stream,
                subscribe.id,
                SubscribeRunEventsResult {
                    events: Vec::new(),
                    latest_event_seq: None,
                },
            );

            let cancel = read_request(&mut stream);
            assert_eq!(cancel.method, ta_protocol::wire::METHOD_DAEMON_RUN_CANCEL);
            write_response(&mut stream, cancel.id, serde_json::json!({}));
            release_receiver
                .recv()
                .expect("release server after cancel assertion");
        });

        let client = PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string())
            .expect("client should connect");
        let subscription = client
            .subscribe_run_events(SubscribeRunEventsRequest {
                session_id,
                run_id: run_id.clone(),
                after_seq: None,
            })
            .expect("subscribe should succeed");
        let (event_started_sender, event_started_receiver) = std::sync::mpsc::channel();
        let event_waiter = thread::spawn(move || {
            event_started_sender.send(()).expect("event waiter started");
            subscription.recv()
        });
        event_started_receiver
            .recv()
            .expect("event waiter should start before cancel");

        let _: serde_json::Value = client
            .call_public(
                ta_protocol::wire::METHOD_DAEMON_RUN_CANCEL,
                &ta_protocol::wire::DaemonRunCancelParams {
                    run_id,
                    reason: None,
                },
            )
            .expect("cancel must complete while the event waiter is blocked");

        client.close();
        assert!(matches!(
            event_waiter.join().expect("event waiter should join"),
            Err(JsonRpcClientError::ConnectionClosed)
        ));
        release_sender.send(()).expect("release server");
        server.join().expect("server should complete");
        cleanup_socket_address(&config.socket_address);
    }

    #[test]
    fn lifecycle_subscription_uses_empty_navigation_contract_and_buffers_invalidation() {
        let socket_name = format!("ta-daemon-client-lifecycle-ready-{}", unique_id_suffix());
        let config = ClientConfig::local_default("ta-daemon-test", &socket_name);
        let listener = bind_listener(&config.socket_address).expect("listener should bind");
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one connection");
            let initialize = read_request(&mut stream);
            assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
            write_response(
                &mut stream,
                initialize.id,
                initialized_daemon("daemon-lifecycle"),
            );

            let subscribe = read_request(&mut stream);
            assert_eq!(subscribe.method, METHOD_DAEMON_NAVIGATION_SUBSCRIBE);
            let params: DaemonNavigationSubscribeParams =
                serde_json::from_value(subscribe.params.expect("subscribe params"))
                    .expect("subscribe params should decode");
            assert_eq!(params, DaemonNavigationSubscribeParams {});
            write_notification(
                &mut stream,
                JsonRpcNotification::new(
                    METHOD_DAEMON_NAVIGATION_INVALIDATED,
                    Some(serde_json::json!({})),
                ),
            );
            write_response(
                &mut stream,
                subscribe.id,
                DaemonNavigationSubscribeResult {},
            );
            release_receiver.recv().expect("release server");
        });

        let client =
            PersistentDaemonClient::connect(config.clone(), "ta-cli".to_string()).expect("connect");
        client
            .initialize("ta-cli", "test", None)
            .expect("initialize");
        let (subscription, state) = client.subscribe_lifecycle().expect("subscribe");
        assert_eq!(state, DaemonLifecycleSubscriptionState::Ready);
        assert_eq!(
            subscription.recv().expect("buffered lifecycle event"),
            DaemonLifecycleUpdate::Invalidated
        );

        client.close();
        release_sender.send(()).expect("release server");
        server.join().expect("server should complete");
        cleanup_socket_address(&config.socket_address);
    }

    fn read_request(stream: &mut impl std::io::Read) -> JsonRpcRequest {
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .expect("request should read");
        match JsonLineCodec
            .decode_message(&line)
            .expect("request should decode")
        {
            JsonRpcMessage::Request(request) => request,
            other => panic!("expected request, got {other:?}"),
        }
    }

    fn write_response<T: serde::Serialize>(
        stream: &mut impl std::io::Write,
        request_id: ta_jsonrpc::RequestId,
        value: T,
    ) {
        let line = JsonLineCodec
            .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                request_id,
                serde_json::to_value(value).expect("response should serialize"),
            )))
            .expect("response should encode");
        stream
            .write_all(line.as_bytes())
            .expect("response should write");
        stream.flush().expect("response should flush");
    }

    fn write_notification(stream: &mut impl std::io::Write, notification: JsonRpcNotification) {
        let line = JsonLineCodec
            .encode_message(&JsonRpcMessage::Notification(notification))
            .expect("notification should encode");
        stream
            .write_all(line.as_bytes())
            .expect("notification should write");
        stream.flush().expect("notification should flush");
    }

    fn run_event_delta(run_id: &RunId, sequence: u64) -> RunEventDelta {
        RunEventDelta {
            seq: sequence,
            event: PublicDaemonEvent::Run(
                RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
                    .expect("active status"),
            ),
        }
    }

    fn run_event_item(run_id: &RunId, sequence: u64) -> RunEventStreamItem {
        RunEventStreamItem {
            run_id: run_id.clone(),
            payload: RunEventStreamPayload::Delta {
                delta: run_event_delta(run_id, sequence),
            },
        }
    }

    fn run_event_notification(item: RunEventStreamItem) -> JsonRpcNotification {
        JsonRpcNotification::new(
            ta_protocol::wire::METHOD_DAEMON_RUN_EVENT,
            Some(serde_json::to_value(item).expect("event should serialize")),
        )
    }

    fn initialized_daemon(daemon_instance_id: &str) -> DaemonInitializeResult {
        DaemonInitializeResult {
            daemon_instance_id: daemon_instance_id.to_string(),
            daemon_version: "test".to_string(),
            client_credential: "test-client-credential".to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
            capabilities: ta_protocol::wire::DaemonServerCapabilities {
                notifications: true,
                event_subscriptions: true,
            },
        }
    }

    fn event_sequence(item: &RunEventStreamItem) -> Option<u64> {
        match &item.payload {
            RunEventStreamPayload::Delta { delta } => Some(delta.seq),
            RunEventStreamPayload::Error { .. } => None,
        }
    }

    fn cleanup_socket_address(socket_address: &ta_jsonrpc::SocketAddress) {
        if let ta_jsonrpc::SocketAddress::Unix(path) = socket_address {
            let _ = fs::remove_file(path);
        }
    }

    fn unique_id_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    }
}
