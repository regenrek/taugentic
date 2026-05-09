use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    DAEMON_PROTOCOL_VERSION, DaemonInitializeResult, DaemonServerCapabilities,
    DaemonSessionAttachResult, DaemonSessionOpenResult, DaemonSubscribeResult,
    DaemonWorkspaceGetResult, DaemonWorkspaceListResult, DaemonWorkspaceOpenResult,
    JsonRpcHandlerResult, JsonRpcRequest, JsonRpcServerSession, METHOD_DAEMON_ACTIVITY_PAGE,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_CONTEXT_RECEIPTS_LIST,
    METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE, METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE,
    METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT, METHOD_DAEMON_RECIPES_LIST, METHOD_DAEMON_RUN_CANCEL,
    METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT, METHOD_DAEMON_RUN_FORK, METHOD_DAEMON_RUN_GET,
    METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_LIST_NATIVE, METHOD_DAEMON_RUN_REPLAY_EVENTS,
    METHOD_DAEMON_RUN_RESUME, METHOD_DAEMON_RUN_START, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_RUN_TIMELINE, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_GET,
    METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_SESSION_OVERVIEW,
    METHOD_DAEMON_WORK_ITEM_DISMISS, METHOD_DAEMON_WORK_ITEM_LIST, METHOD_DAEMON_WORK_ITEM_REFRESH,
    METHOD_DAEMON_WORK_ITEM_TRIGGER, METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST,
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
    daemon_status_result, json_deferred_mutation_result, json_result, spawn_event_forwarder,
    spawn_run_event_forwarder, wake_local_server_accept_loop,
};

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
            let workspace_id = match params.workspace {
                WorkspaceSelector::ById { id } => id,
                WorkspaceSelector::ByPath {
                    path,
                    trust_acknowledged,
                } => {
                    state
                        .app
                        .open_workspace(&OpenWorkspaceRequest {
                            path,
                            trust_acknowledged,
                        })
                        .map_err(map_app_service_error)?
                        .id
                }
            };
            let opened_session = state
                .app
                .open_session(
                    &client_name,
                    &principal_id,
                    &OpenSessionRequest {
                        title: params.title,
                        workspace_id,
                    },
                )
                .map_err(map_app_service_error)?;
            let latest_cursor = state
                .app
                .latest_event_cursor_for_session(&opened_session.session.id)
                .map_err(map_app_service_error)?;
            session_state
                .lock()
                .expect("daemon rpc session state should not be poisoned")
                .attached_session_id = Some(opened_session.session.id.clone());
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
                .map_err(|error| internal_error(error.to_string()))?;
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
        DaemonRpcRequest::AgentRuntimeProfileSelect(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT)?;
            let snapshot = state
                .app
                .select_agent_runtime_profile(&params)
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
        DaemonRpcRequest::AgentRuntimeAuthLogout(params) => {
            ensure_initialized(session_state, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT)?;
            let result = state
                .app
                .logout_agent_runtime_auth_profile(&params)
                .await
                .map_err(map_app_service_error)?;
            json_result(result)
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
