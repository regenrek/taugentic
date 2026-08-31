use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    DAEMON_PROTOCOL_VERSION, DaemonInitializeResult, DaemonNavigationIntentResult,
    DaemonNavigationSnapshotResult, DaemonNavigationSubscribeResult, DaemonProjectOpenResult,
    DaemonServerCapabilities, DaemonSessionAttachResult, DaemonSessionOpenResult,
    DaemonSubscribeResult, DaemonWorkspaceGetResult, DaemonWorkspaceListResult,
    DaemonWorkspaceOpenResult, JsonRpcHandlerResult, JsonRpcRequest, JsonRpcServerSession,
    METHOD_DAEMON_ACTIVITY_PAGE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_APPROVAL_DECIDE,
    METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET, METHOD_DAEMON_ARTIFACT_LIST,
    METHOD_DAEMON_BROWSER_ACTION, METHOD_DAEMON_BROWSER_CLEAR_DATA, METHOD_DAEMON_BROWSER_PROFILE,
    METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT, METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT,
    METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
    METHOD_DAEMON_CODE_HOST_PUSH_PREPARE, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
    METHOD_DAEMON_CONTEXT_RECEIPTS_LIST, METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE,
    METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE, METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT,
    METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT, METHOD_DAEMON_GIT_CHECKPOINT_LIST,
    METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT, METHOD_DAEMON_GIT_COMMIT, METHOD_DAEMON_GIT_DIFF,
    METHOD_DAEMON_GIT_SNAPSHOT, METHOD_DAEMON_GIT_STAGE, METHOD_DAEMON_GIT_UNSTAGE,
    METHOD_DAEMON_NAVIGATION_INTENT, METHOD_DAEMON_NAVIGATION_SNAPSHOT,
    METHOD_DAEMON_NAVIGATION_SUBSCRIBE, METHOD_DAEMON_PLUGIN_INSPECT, METHOD_DAEMON_PLUGIN_INSTALL,
    METHOD_DAEMON_PLUGIN_LIST, METHOD_DAEMON_PLUGIN_UNINSTALL, METHOD_DAEMON_PROJECT_OPEN,
    METHOD_DAEMON_RECIPES_LIST, METHOD_DAEMON_RUN_CANCEL, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT,
    METHOD_DAEMON_RUN_CONTINUE, METHOD_DAEMON_RUN_FORK, METHOD_DAEMON_RUN_GET,
    METHOD_DAEMON_RUN_JOIN, METHOD_DAEMON_RUN_LINEAGE_GRAPH, METHOD_DAEMON_RUN_LIST,
    METHOD_DAEMON_RUN_LIST_NATIVE, METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_RESUME,
    METHOD_DAEMON_RUN_SPAWN, METHOD_DAEMON_RUN_START, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_RUN_SWITCH_ROUTE_AND_RESUME, METHOD_DAEMON_RUN_TIMELINE,
    METHOD_DAEMON_SCHEDULED_WORK_CANCEL, METHOD_DAEMON_SCHEDULED_WORK_CREATE,
    METHOD_DAEMON_SCHEDULED_WORK_LIST, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_GET,
    METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_SESSION_OVERVIEW,
    METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION, METHOD_DAEMON_TERMINAL_ATTACH,
    METHOD_DAEMON_TERMINAL_CLOSE, METHOD_DAEMON_TERMINAL_DETACH, METHOD_DAEMON_TERMINAL_INPUT,
    METHOD_DAEMON_TERMINAL_LIST, METHOD_DAEMON_TERMINAL_RESIZE, METHOD_DAEMON_TERMINAL_SPAWN,
    METHOD_DAEMON_WORK_ITEM_DISMISS, METHOD_DAEMON_WORK_ITEM_LIST, METHOD_DAEMON_WORK_ITEM_REFRESH,
    METHOD_DAEMON_WORK_ITEM_TRIGGER, METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL,
    METHOD_DAEMON_WORKSPACE_FILE_READ, METHOD_DAEMON_WORKSPACE_FILE_TREE,
    METHOD_DAEMON_WORKSPACE_FILE_WRITE, METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST,
    METHOD_DAEMON_WORKSPACE_OPEN, METHOD_WORKFLOW_LOAD, METHOD_WORKFLOW_RELOAD,
    METHOD_WORKFLOW_STATUS, METHOD_WORKFLOW_VALIDATE, OpenSessionRequest, OpenWorkspaceRequest,
    RecipeListResponse, WorkspaceSelector,
    host::{
        bootstrap::BootstrapState,
        control::rpc::handle_control_status_request,
        internal_stop::{InternalDaemonStopResult, METHOD_DAEMON_INTERNAL_STOP},
    },
    internal_error, invalid_params,
};

use super::errors::map_app_service_error;
use super::request::DaemonRpcRequest;
use super::state::{
    DaemonRpcSessionState, approval_actor_from_session, ensure_initialized,
    require_attached_session, require_client_name, require_internal_handoff_client,
    require_principal_id, validate_client_capabilities, validate_client_name,
};
use super::{
    daemon_status_result, defer_navigation_invalidation, json_deferred_mutation_result,
    json_result, spawn_event_forwarder, spawn_navigation_invalidation_forwarder,
    spawn_run_event_forwarder, spawn_terminal_event_forwarder, wake_local_server_accept_loop,
};

enum SessionOpenPlacement {
    Standalone,
    Project(crate::ProjectId),
    Temporary,
}

pub(super) async fn handle_request(
    state: &BootstrapState,
    shutdown_requested: &Arc<AtomicBool>,
    session: &JsonRpcServerSession,
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    request: JsonRpcRequest,
) -> JsonRpcHandlerResult {
    let request = DaemonRpcRequest::parse(&request)?;
    let _rpc_guard = state.track_rpc_request();
    match request {
        DaemonRpcRequest::Initialize(params) => {
            if params.protocol_version != DAEMON_PROTOCOL_VERSION {
                return Err(invalid_params(format!(
                    "unsupported protocol version {}; expected {}",
                    params.protocol_version, DAEMON_PROTOCOL_VERSION
                )));
            }
            validate_client_name(&params.client_name)?;
            let client_name = params.client_name.trim().to_string();
            validate_client_capabilities(&params.capabilities)?;
            let principal = state
                .app
                .resolve_or_issue_session_principal(
                    &client_name,
                    params.client_credential.as_deref(),
                )
                .map_err(map_app_service_error)?;
            let mut session_state = session_state
                .lock()
                .expect("daemon rpc session state should not be poisoned");
            session_state.initialized = true;
            session_state.client_name = Some(principal.client_name.clone());
            session_state.client_credential = Some(principal.client_credential.clone());
            session_state.principal_id = Some(principal.principal_id);
            session_state.attached_session_id = None;
            json_result(DaemonInitializeResult {
                daemon_instance_id: state.runtime.daemon_instance_id(),
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                client_credential: principal.client_credential,
                protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                capabilities: DaemonServerCapabilities {
                    notifications: true,
                    event_subscriptions: true,
                },
            })
        }
        DaemonRpcRequest::Status => {
            let status = daemon_status_result(state);
            tracing::debug!(
                daemon.ready = status.ready,
                daemon.log_path = %status.log_path,
                daemon.version = %status.version,
                "daemon status request completed"
            );
            json_result(status)
        }
        DaemonRpcRequest::ControlStatus => handle_control_status_request(state),
        DaemonRpcRequest::DiagnosticsSnapshot => {
            ensure_initialized(session_state, METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT)?;
            let snapshot = state
                .app
                .diagnostics_snapshot(
                    &state.runtime.host_platform,
                    state.uptime_ms(),
                    u32::try_from(state.in_flight_rpc_count()).unwrap_or(u32::MAX),
                )
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::SessionOpen(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_OPEN)?;
            let client_name = require_client_name(session_state, METHOD_DAEMON_SESSION_OPEN)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_SESSION_OPEN)?;
            let (workspace_id, placement) = match params.workspace {
                WorkspaceSelector::ById { id } => (id, SessionOpenPlacement::Standalone),
                WorkspaceSelector::ByPath {
                    path,
                    trust_acknowledged,
                } => (
                    state
                        .app
                        .open_workspace(&OpenWorkspaceRequest {
                            path,
                            trust_acknowledged,
                        })
                        .map_err(map_app_service_error)?
                        .id,
                    SessionOpenPlacement::Standalone,
                ),
                WorkspaceSelector::ByProject {
                    project_id,
                    workspace_id,
                } => (workspace_id, SessionOpenPlacement::Project(project_id)),
                WorkspaceSelector::ByTemporary { workspace_id } => {
                    (workspace_id, SessionOpenPlacement::Temporary)
                }
            };
            let request = OpenSessionRequest {
                title: params.title,
                workspace_id,
            };
            let opened_session = match placement {
                SessionOpenPlacement::Standalone => {
                    state
                        .app
                        .open_session(&client_name, &principal_id, &request)
                }
                SessionOpenPlacement::Project(project_id) => state.app.open_project_session(
                    &client_name,
                    &principal_id,
                    &request,
                    project_id,
                ),
                SessionOpenPlacement::Temporary => {
                    state
                        .app
                        .open_temporary_session(&client_name, &principal_id, &request)
                }
            }
            .map_err(map_app_service_error)?;
            let latest_cursor = state
                .app
                .latest_event_cursor_for_session(&opened_session.session.id)
                .map_err(map_app_service_error)?;
            session_state
                .lock()
                .expect("daemon rpc session state should not be poisoned")
                .attached_session_id = Some(opened_session.session.id.clone());
            state
                .runtime
                .register_navigation_session(&opened_session.session.id, &principal_id);
            defer_navigation_invalidation(session, state.runtime.clone(), principal_id);
            json_result(DaemonSessionOpenResult {
                session: opened_session.session,
                latest_cursor,
                session_authority: opened_session.session_authority,
            })
        }
        DaemonRpcRequest::WorkspaceOpen(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_OPEN)?;
            let workspace = state
                .app
                .open_workspace(&OpenWorkspaceRequest {
                    path: params.path,
                    trust_acknowledged: params.trust_acknowledged,
                })
                .map_err(map_app_service_error)?;
            json_result(DaemonWorkspaceOpenResult { workspace })
        }
        DaemonRpcRequest::WorkspaceList(_params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_LIST)?;
            let workspaces = state.app.list_workspaces().map_err(map_app_service_error)?;
            json_result(DaemonWorkspaceListResult { workspaces })
        }
        DaemonRpcRequest::WorkspaceGet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_GET)?;
            let workspace = state
                .app
                .get_workspace(&params.id)
                .map_err(map_app_service_error)?;
            json_result(DaemonWorkspaceGetResult { workspace })
        }
        DaemonRpcRequest::WorkspaceFileTree(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_FILE_TREE)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_WORKSPACE_FILE_TREE)?;
            let result = state
                .app
                .workspace_file_tree(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::WorkspaceFileRead(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_FILE_READ)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_WORKSPACE_FILE_READ)?;
            let result = state
                .app
                .read_workspace_file(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::WorkspaceFileWrite(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_FILE_WRITE)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_WORKSPACE_FILE_WRITE)?;
            let result = state
                .app
                .write_workspace_file(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::WorkspaceFileOpenExternal(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL)?;
            let result = state
                .app
                .workspace_file_open_external(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::CodeHostAccountList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST)?;
            json_result(
                state
                    .app
                    .code_host_accounts(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostAccountConnect(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT)?;
            json_result(
                state
                    .app
                    .connect_code_host_account(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostAccountDisconnect(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT)?;
            json_result(
                state
                    .app
                    .disconnect_code_host_account(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostRepositoryContext(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT)?;
            json_result(
                state
                    .app
                    .code_host_repository_context(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPushPrepare(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PUSH_PREPARE)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PUSH_PREPARE)?;
            json_result(
                state
                    .app
                    .prepare_code_host_push(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPushApply(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PUSH_APPLY)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PUSH_APPLY)?;
            json_result(
                state
                    .app
                    .apply_code_host_push(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST)?;
            json_result(
                state
                    .app
                    .list_code_host_pull_requests(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestDetail(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL)?;
            json_result(
                state
                    .app
                    .code_host_pull_request_detail(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestEnsure(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE)?;
            json_result(
                state
                    .app
                    .ensure_code_host_pull_request(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestChecks(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS)?;
            json_result(
                state
                    .app
                    .code_host_pull_request_checks(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestActivity(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY)?;
            json_result(
                state
                    .app
                    .code_host_pull_request_activity(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::CodeHostPullRequestCommentCreate(params) => {
            ensure_initialized(
                session_state,
                METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
            )?;
            let principal_id = require_principal_id(
                session_state,
                METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
            )?;
            json_result(
                state
                    .app
                    .create_code_host_pull_request_comment(&principal_id, &params)
                    .await
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitSnapshot(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_SNAPSHOT)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_GIT_SNAPSHOT)?;
            json_result(
                state
                    .app
                    .git_repository_snapshot(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitDiff(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_DIFF)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_GIT_DIFF)?;
            json_result(
                state
                    .app
                    .git_diff(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitStage(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_STAGE)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_GIT_STAGE)?;
            json_result(
                state
                    .app
                    .git_stage_paths(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitUnstage(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_UNSTAGE)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_GIT_UNSTAGE)?;
            json_result(
                state
                    .app
                    .git_unstage_paths(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitCommit(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_COMMIT)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_GIT_COMMIT)?;
            json_result(
                state
                    .app
                    .git_commit(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitCheckpointList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_CHECKPOINT_LIST)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_GIT_CHECKPOINT_LIST)?;
            json_result(
                state
                    .app
                    .git_checkpoints(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitCheckpointPrepareRevert(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT)?;
            json_result(
                state
                    .app
                    .git_prepare_checkpoint_revert(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::GitCheckpointApplyRevert(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT)?;
            json_result(
                state
                    .app
                    .git_apply_checkpoint_revert(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::TerminalSpawn(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_SPAWN)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_SPAWN)?;
            let result = state
                .app
                .spawn_terminal(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::TerminalList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_LIST)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_LIST)?;
            let result = state
                .app
                .list_terminals(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::TerminalAttach(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_ATTACH)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_ATTACH)?;
            let subscription = state
                .app
                .attach_terminal(&principal_id, &params, session.connection_id())
                .map_err(map_app_service_error)?;
            let result = subscription.result.clone();
            let terminal_id = params.terminal_id;
            let forwarder_session = session.clone();
            session.defer_until_response(Box::new(move || {
                spawn_terminal_event_forwarder(forwarder_session, terminal_id, subscription);
            }));
            json_result(result)
        }
        DaemonRpcRequest::TerminalInput(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_INPUT)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_INPUT)?;
            let result = state
                .app
                .terminal_input(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::TerminalResize(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_RESIZE)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_RESIZE)?;
            let result = state
                .app
                .resize_terminal(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::TerminalDetach(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_DETACH)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_DETACH)?;
            let result = state
                .app
                .detach_terminal(&principal_id, &params, session.connection_id())
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::TerminalClose(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_TERMINAL_CLOSE)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_TERMINAL_CLOSE)?;
            let result = state
                .app
                .close_terminal(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::ProjectOpen(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_PROJECT_OPEN)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_PROJECT_OPEN)?;
            let (project_id, snapshot) = state
                .app
                .open_project(&principal_id, params.path, params.trust_acknowledged)
                .map_err(map_app_service_error)?;
            defer_navigation_invalidation(session, state.runtime.clone(), principal_id);
            json_result(DaemonProjectOpenResult {
                project_id,
                snapshot,
            })
        }
        DaemonRpcRequest::SessionAttach(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_ATTACH)?;
            let client_name = require_client_name(session_state, METHOD_DAEMON_SESSION_ATTACH)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_SESSION_ATTACH)?;
            let attached_session = state
                .app
                .attach_session(
                    &client_name,
                    &principal_id,
                    &params.session_id,
                    &params.session_authority,
                )
                .map_err(map_app_service_error)?;
            let latest_cursor = state
                .app
                .latest_event_cursor_for_session(&attached_session.id)
                .map_err(map_app_service_error)?;
            session_state
                .lock()
                .expect("daemon rpc session state should not be poisoned")
                .attached_session_id = Some(attached_session.id.clone());
            json_result(DaemonSessionAttachResult {
                session: attached_session.session,
                latest_cursor,
                session_authority: attached_session.session_authority,
            })
        }
        DaemonRpcRequest::SessionSetNextRunSelection(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION)?;
            let session_id = require_attached_session(
                session_state,
                METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION,
            )?;
            json_result(
                state
                    .app
                    .set_session_next_run_selection(&session_id, params.selection)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::ActivityPage(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_ACTIVITY_PAGE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_ACTIVITY_PAGE)?;
            let page = state
                .app
                .activity_page(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(page)
        }
        DaemonRpcRequest::AgentTurnsPage(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_AGENT_TURNS_PAGE)?;
            let attached_session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_AGENT_TURNS_PAGE)?;
            let page = state
                .app
                .agent_turns_page(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(page)
        }
        DaemonRpcRequest::ThreadWorkspaceGet(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_THREAD_WORKSPACE_GET)?;
            let session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_THREAD_WORKSPACE_GET)?;
            json_result(
                state
                    .app
                    .thread_workspace(&session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::ThreadWorkspaceUpdate(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_THREAD_WORKSPACE_UPDATE)?;
            let session_id = require_attached_session(
                session_state,
                crate::METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            )?;
            json_result(
                state
                    .app
                    .update_thread_workspace(&session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::SessionList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_LIST)?;
            let client_name = require_client_name(session_state, METHOD_DAEMON_SESSION_LIST)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_SESSION_LIST)?;
            let sessions = state
                .app
                .list_sessions(&client_name, &principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(sessions)
        }
        DaemonRpcRequest::SessionOverview(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_OVERVIEW)?;
            let client_name = require_client_name(session_state, METHOD_DAEMON_SESSION_OVERVIEW)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_SESSION_OVERVIEW)?;
            let snapshot = state
                .app
                .session_overview(&client_name, &principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::NavigationSnapshot(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_NAVIGATION_SNAPSHOT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_NAVIGATION_SNAPSHOT)?;
            let snapshot = state
                .app
                .navigation_snapshot(&principal_id, params.search.as_deref())
                .map_err(map_app_service_error)?;
            json_result(DaemonNavigationSnapshotResult { snapshot })
        }
        DaemonRpcRequest::NavigationIntent(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_NAVIGATION_INTENT)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_NAVIGATION_INTENT)?;
            let snapshot = state
                .app
                .apply_navigation_intent(&principal_id, params.intent)
                .map_err(map_app_service_error)?;
            defer_navigation_invalidation(session, state.runtime.clone(), principal_id);
            json_result(DaemonNavigationIntentResult { snapshot })
        }
        DaemonRpcRequest::NavigationSubscribe(_params) => {
            ensure_initialized(session_state, METHOD_DAEMON_NAVIGATION_SUBSCRIBE)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_NAVIGATION_SUBSCRIBE)?;
            state
                .app
                .register_navigation_sessions_for_principal(&principal_id)
                .map_err(map_app_service_error)?;
            let subscription = state.runtime.subscribe_navigation(&principal_id);
            let forwarder_session = session.clone();
            session.defer_until_response(Box::new(move || {
                spawn_navigation_invalidation_forwarder(forwarder_session, subscription);
            }));
            json_result(DaemonNavigationSubscribeResult {})
        }
        DaemonRpcRequest::SessionGet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SESSION_GET)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_SESSION_GET)?;
            let _ = params;
            let session = state
                .app
                .get_session(&attached_session_id)
                .map_err(|error| internal_error(error.to_string()))?;
            json_result(session)
        }
        DaemonRpcRequest::ApprovalList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_APPROVAL_LIST)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_APPROVAL_LIST)?;
            let approvals = state
                .app
                .list_approvals(&attached_session_id, &params)
                .map_err(|error| internal_error(error.to_string()))?;
            json_result(approvals)
        }
        DaemonRpcRequest::ApprovalDecide(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_APPROVAL_DECIDE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_APPROVAL_DECIDE)?;
            let actor = approval_actor_from_session(session_state, METHOD_DAEMON_APPROVAL_DECIDE)?;
            let decided = state
                .app
                .decide_approval(&attached_session_id, &actor, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), decided, |result| {
                crate::DaemonApprovalDecideResult { run: result.body }
            })
        }
        DaemonRpcRequest::WorkItemList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORK_ITEM_LIST)?;
            let items = state
                .app
                .list_work_items(&params)
                .map_err(map_app_service_error)?;
            json_result(items)
        }
        DaemonRpcRequest::WorkItemRefresh(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORK_ITEM_REFRESH)?;
            let items = state
                .app
                .refresh_work_items(&params)
                .map_err(map_app_service_error)?;
            json_result(items)
        }
        DaemonRpcRequest::WorkItemDismiss(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORK_ITEM_DISMISS)?;
            let dismissed = state
                .app
                .dismiss_work_item(&params)
                .map_err(map_app_service_error)?;
            json_result(dismissed)
        }
        DaemonRpcRequest::WorkItemTrigger(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_WORK_ITEM_TRIGGER)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_WORK_ITEM_TRIGGER)?;
            let triggered = state
                .app
                .trigger_work_item(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), triggered, |result| {
                result.body
            })
        }
        DaemonRpcRequest::ScheduledWorkCreate(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SCHEDULED_WORK_CREATE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_SCHEDULED_WORK_CREATE)?;
            let control = crate::read_persisted_runtime_control_state()
                .map_err(|error| internal_error(error.to_string()))?;
            if !control.background_opt_in {
                return Err(invalid_params(
                    "scheduled work requires explicit background opt-in",
                ));
            }
            let created = state
                .app
                .create_scheduled_work(&attached_session_id, params)
                .map_err(map_app_service_error)?;
            state.scheduled_work_deadline.rearm();
            json_result(created)
        }
        DaemonRpcRequest::ScheduledWorkList(_params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SCHEDULED_WORK_LIST)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_SCHEDULED_WORK_LIST)?;
            json_result(
                state
                    .app
                    .list_scheduled_work(&attached_session_id)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::ScheduledWorkCancel(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_SCHEDULED_WORK_CANCEL)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_SCHEDULED_WORK_CANCEL)?;
            let actor =
                approval_actor_from_session(session_state, METHOD_DAEMON_SCHEDULED_WORK_CANCEL)?;
            state
                .app
                .cancel_scheduled_work(&attached_session_id, &actor, &params)
                .map_err(map_app_service_error)?;
            state.scheduled_work_deadline.rearm();
            json_result(serde_json::json!({}))
        }
        DaemonRpcRequest::PluginInspect(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_PLUGIN_INSPECT)?;
            let inspected = state
                .app
                .inspect_plugin_package(&params)
                .map_err(map_app_service_error)?;
            json_result(inspected)
        }
        DaemonRpcRequest::PluginInstall(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_PLUGIN_INSTALL)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_PLUGIN_INSTALL)?;
            let installed = state
                .app
                .install_plugin_package(&principal_id, &params, &state.config.plugin_root())
                .map_err(map_app_service_error)?;
            json_result(installed)
        }
        DaemonRpcRequest::PluginList(_params) => {
            ensure_initialized(session_state, METHOD_DAEMON_PLUGIN_LIST)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_PLUGIN_LIST)?;
            json_result(
                state
                    .app
                    .list_plugin_installations(&principal_id)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::PluginUninstall(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_PLUGIN_UNINSTALL)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_PLUGIN_UNINSTALL)?;
            state
                .app
                .uninstall_plugin(&principal_id, &params)
                .map_err(map_app_service_error)?;
            json_result(serde_json::json!({}))
        }
        DaemonRpcRequest::BrowserProfile(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_BROWSER_PROFILE)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_BROWSER_PROFILE)?;
            json_result(
                state
                    .app
                    .browser_profile(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::BrowserAction(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_BROWSER_ACTION)?;
            let principal_id = require_principal_id(session_state, METHOD_DAEMON_BROWSER_ACTION)?;
            json_result(
                state
                    .app
                    .decide_browser_action(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::BrowserClearData(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_BROWSER_CLEAR_DATA)?;
            let principal_id =
                require_principal_id(session_state, METHOD_DAEMON_BROWSER_CLEAR_DATA)?;
            json_result(
                state
                    .app
                    .clear_browser_data(&principal_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::WorkflowLoad(params) => {
            ensure_initialized(session_state, METHOD_WORKFLOW_LOAD)?;
            let status = state
                .app
                .load_workflow(&params)
                .map_err(map_app_service_error)?;
            json_result(status)
        }
        DaemonRpcRequest::WorkflowStatus => {
            ensure_initialized(session_state, METHOD_WORKFLOW_STATUS)?;
            json_result(state.app.workflow_status())
        }
        DaemonRpcRequest::WorkflowReload(params) => {
            ensure_initialized(session_state, METHOD_WORKFLOW_RELOAD)?;
            let status = state
                .app
                .reload_workflow(&params)
                .map_err(map_app_service_error)?;
            json_result(status)
        }
        DaemonRpcRequest::WorkflowValidate(params) => {
            ensure_initialized(session_state, METHOD_WORKFLOW_VALIDATE)?;
            let report = state
                .app
                .validate_workflow(&params)
                .map_err(map_app_service_error)?;
            json_result(report)
        }
        DaemonRpcRequest::ArtifactGet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_ARTIFACT_GET)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_ARTIFACT_GET)?;
            let artifact = state
                .app
                .get_artifact(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(artifact)
        }
        DaemonRpcRequest::ArtifactList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_ARTIFACT_LIST)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_ARTIFACT_LIST)?;
            let artifacts = state
                .app
                .list_artifacts(&attached_session_id, &params)
                .map_err(|error| internal_error(error.to_string()))?;
            json_result(artifacts)
        }
        DaemonRpcRequest::ContextReceiptsList(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_LIST)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_LIST)?;
            let receipts = state
                .app
                .list_receipts(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(receipts)
        }
        DaemonRpcRequest::ContextReceiptsPromote(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE)?;
            let promoted = state
                .app
                .promote_receipt(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), promoted, |result| {
                result.body
            })
        }
        DaemonRpcRequest::ContextReceiptsQuarantine(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE)?;
            let quarantined = state
                .app
                .quarantine_receipt(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), quarantined, |result| {
                result.body
            })
        }
        DaemonRpcRequest::RunStart(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_START)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_START)?;
            let started = state
                .app
                .start_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), started, |result| {
                result.body
            })
        }
        DaemonRpcRequest::RunCompleteWithResult(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT)?;
            let completed = state
                .app
                .complete_run_with_result(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), completed, |result| {
                result.body
            })
        }
        DaemonRpcRequest::RunResume(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_RESUME)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_RESUME)?;
            let resumed = state
                .app
                .resume_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(resumed)
        }
        DaemonRpcRequest::RunFork(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_FORK)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_FORK)?;
            let forked = state
                .app
                .fork_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(forked)
        }
        DaemonRpcRequest::RunContinue(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_CONTINUE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_CONTINUE)?;
            let continued = state
                .app
                .continue_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(continued)
        }
        DaemonRpcRequest::RunSwitchRouteAndResume(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_SWITCH_ROUTE_AND_RESUME)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_SWITCH_ROUTE_AND_RESUME)?;
            let continued = state
                .app
                .switch_route_and_resume(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(continued)
        }
        DaemonRpcRequest::RunSpawn(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_SPAWN)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_SPAWN)?;
            let spawned = state
                .app
                .spawn_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(spawned)
        }
        DaemonRpcRequest::RunJoin(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_JOIN)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_JOIN)?;
            let joined = state
                .app
                .join_run(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(joined)
        }
        DaemonRpcRequest::RunReplayEvents(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_REPLAY_EVENTS)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_REPLAY_EVENTS)?;
            let replay = state
                .app
                .replay_run_events(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(replay)
        }
        DaemonRpcRequest::RunSubscribeEvents(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS)?;
            let subscription = state
                .app
                .subscribe_run_events(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            let result = crate::SubscribeRunEventsResult {
                events: subscription.replay.clone(),
                latest_event_seq: subscription.latest_event_seq,
            };
            let run_id = params.run_id;
            let forwarder_session = session.clone();
            session.defer_until_response(Box::new(move || {
                spawn_run_event_forwarder(forwarder_session, run_id, subscription);
            }));
            json_result(result)
        }
        DaemonRpcRequest::RunCancel(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_CANCEL)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_CANCEL)?;
            let actor = approval_actor_from_session(session_state, METHOD_DAEMON_RUN_CANCEL)?;
            let cancelled = state
                .app
                .cancel_run(&attached_session_id, &actor, &params.run_id, params.reason)
                .map_err(map_app_service_error)?;
            json_deferred_mutation_result(session, state.runtime.clone(), cancelled, |result| {
                result.body
            })
        }
        DaemonRpcRequest::RunList(_params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_LIST)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_LIST)?;
            let runs = state
                .app
                .list_runs(&attached_session_id)
                .map_err(|error| internal_error(error.to_string()))?;
            json_result(runs)
        }
        DaemonRpcRequest::RunListNative(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_LIST_NATIVE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_LIST_NATIVE)?;
            let runs = state
                .app
                .list_native_runs(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(runs)
        }
        DaemonRpcRequest::RunLineageGraph(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_LINEAGE_GRAPH)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_LINEAGE_GRAPH)?;
            json_result(
                state
                    .app
                    .run_lineage_graph(&attached_session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::RunGet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_GET)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_GET)?;
            let run = state
                .app
                .get_run(&attached_session_id, &params)
                .map_err(|error| internal_error(error.to_string()))?;
            json_result(run)
        }
        DaemonRpcRequest::RunTimeline(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_RUN_TIMELINE)?;
            let attached_session_id =
                require_attached_session(session_state, METHOD_DAEMON_RUN_TIMELINE)?;
            let timeline = state
                .app
                .run_timeline(&attached_session_id, &params)
                .map_err(map_app_service_error)?;
            json_result(timeline)
        }
        DaemonRpcRequest::VoiceOpen(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_VOICE_OPEN)?;
            let session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_VOICE_OPEN)?;
            json_result(
                state
                    .app
                    .open_voice_stream(&session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::VoiceExchange(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_VOICE_EXCHANGE)?;
            let session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_VOICE_EXCHANGE)?;
            json_result(
                state
                    .app
                    .exchange_voice_stream(&session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::VoiceEnd(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_VOICE_END)?;
            let session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_VOICE_END)?;
            json_result(
                state
                    .app
                    .end_voice_stream(&session_id, &params)
                    .map_err(map_app_service_error)?,
            )
        }
        DaemonRpcRequest::RecipesList => {
            ensure_initialized(session_state, METHOD_DAEMON_RECIPES_LIST)?;
            json_result(RecipeListResponse {
                recipes: state.app.list_recipes(),
            })
        }
        DaemonRpcRequest::AgentRuntimeGet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_GET)?;
            let snapshot = state
                .app
                .get_agent_runtime(&params)
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::AgentRuntimeProfilePatch(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH)?;
            let snapshot = state
                .app
                .patch_agent_runtime_profile(&params)
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::AgentRuntimeAuthLogin(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN)?;
            let result = state
                .app
                .login_agent_runtime_auth_profile(&params)
                .await
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::AgentRuntimeAuthLoginComplete(params) => {
            ensure_initialized(
                session_state,
                METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE,
            )?;
            let result = state
                .app
                .complete_agent_runtime_auth_profile_login(&params)
                .await
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::AgentRuntimeAuthLogout(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT)?;
            let result = state
                .app
                .logout_agent_runtime_auth_profile(&params)
                .await
                .map_err(map_app_service_error)?;
            json_result(result)
        }
        DaemonRpcRequest::AgentRuntimeAuthProfilePreferencesSet(params) => {
            ensure_initialized(
                session_state,
                METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET,
            )?;
            let snapshot = state
                .app
                .replace_agent_runtime_auth_profile_preferences(&params)
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::AgentRuntimeExtensionSet(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET)?;
            let snapshot = state
                .app
                .set_agent_runtime_extension_enabled(&params)
                .map_err(map_app_service_error)?;
            json_result(snapshot)
        }
        DaemonRpcRequest::InternalStop(params) => {
            require_internal_handoff_client(session_state, METHOD_DAEMON_INTERNAL_STOP)?;
            let Some(expected_token) = state.config.control_token.as_ref() else {
                return Err(invalid_params(
                    "daemon.internal.stop is disabled for runtimes without a configured control token",
                ));
            };
            if params.control_token != expected_token.as_str() {
                return Err(invalid_params(
                    "daemon.internal.stop control token mismatch",
                ));
            }
            shutdown_requested.store(true, Ordering::SeqCst);
            wake_local_server_accept_loop(state);
            json_result(InternalDaemonStopResult { stopping: true })
        }
        DaemonRpcRequest::Subscribe(params) => {
            ensure_initialized(session_state, crate::METHOD_DAEMON_SUBSCRIBE)?;
            let attached_session_id =
                require_attached_session(session_state, crate::METHOD_DAEMON_SUBSCRIBE)?;
            let latest_durable_sequence = state
                .app
                .latest_event_cursor_for_session(&attached_session_id)
                .map_err(map_app_service_error)?
                .map(|cursor| cursor.sequence);
            let subscription = state.runtime.subscribe_events(
                &attached_session_id,
                &params.kinds,
                latest_durable_sequence,
                params.after_cursor.as_ref(),
            );
            let crate::host::event_hub::RuntimeEventSubscription {
                latest_cursor,
                backlog,
                receiver,
                overflowed,
                has_gap,
                cleanup,
            } = subscription;
            let session = session.clone();
            let forwarder_session = session.clone();
            session.defer_until_response(Box::new(move || {
                spawn_event_forwarder(
                    forwarder_session,
                    backlog,
                    receiver,
                    overflowed,
                    Some(cleanup),
                );
            }));
            let result = if has_gap {
                DaemonSubscribeResult::HistoryGap { latest_cursor }
            } else {
                DaemonSubscribeResult::Ready { latest_cursor }
            };
            json_result(result)
        }
    }
}
