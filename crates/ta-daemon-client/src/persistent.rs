use std::{
    io,
    sync::{Arc, Mutex},
};

use serde::{Serialize, de::DeserializeOwned};
use ta_jsonrpc::{
    ClientConfig, JsonRpcClientError, JsonRpcNotificationSubscription, PersistentJsonRpcClient,
};
use ta_protocol::wire::{
    AgentRuntimeSnapshot, ApprovalSnapshotResult, ArtifactSnapshotResult, AuthProfileLoginResult,
    AuthProfileLogoutResult, DAEMON_PROTOCOL_VERSION, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    DaemonApprovalDecideParams, DaemonApprovalDecideResult, DaemonClientCapabilities,
    DaemonInitializeParams, DaemonInitializeResult, DaemonNavigationIntent,
    DaemonNavigationIntentParams, DaemonNavigationIntentResult, DaemonNavigationInvalidatedParams,
    DaemonNavigationSnapshotParams, DaemonNavigationSnapshotResult,
    DaemonNavigationSubscribeParams, DaemonNavigationSubscribeResult, DaemonProjectOpenParams,
    DaemonProjectOpenResult, DaemonSessionAttachParams, DaemonSessionAttachResult,
    DaemonSessionOpenParams, DaemonSessionOpenResult, DaemonWorkspaceGetParams,
    DaemonWorkspaceGetResult, DaemonWorkspaceListParams, DaemonWorkspaceListResult,
    DaemonWorkspaceOpenParams, DaemonWorkspaceOpenResult, GetAgentRuntimeQuery, ListApprovalsQuery,
    ListArtifactsQuery, ListRunsQuery, ListSessionsQuery, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_APPROVAL_DECIDE,
    METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_NAVIGATION_INTENT, METHOD_DAEMON_NAVIGATION_INVALIDATED,
    METHOD_DAEMON_NAVIGATION_SNAPSHOT, METHOD_DAEMON_NAVIGATION_SUBSCRIBE,
    METHOD_DAEMON_PROJECT_OPEN, METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_START,
    METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_LIST,
    METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_SESSION_OVERVIEW, METHOD_DAEMON_WORKSPACE_GET,
    METHOD_DAEMON_WORKSPACE_LIST, METHOD_DAEMON_WORKSPACE_OPEN, RunEventStreamItem,
    RunEventStreamPayload, RunSummary, SessionOverviewQuery, SessionOverviewResult, SessionSummary,
    StartRunCommand, SubscribeRunEventsRequest, SubscribeRunEventsResult, Workspace, WorkspaceId,
    WorkspacePath,
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

    pub fn list_approvals(
        &mut self,
        query: ListApprovalsQuery,
    ) -> Result<ApprovalSnapshotResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_APPROVAL_LIST, &query)
    }

    pub fn list_artifacts(
        &mut self,
        query: ListArtifactsQuery,
    ) -> Result<ArtifactSnapshotResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_ARTIFACT_LIST, &query)
    }

    pub fn start_run(
        &mut self,
        command: StartRunCommand,
    ) -> Result<RunSummary, JsonRpcClientError> {
        self.call(METHOD_DAEMON_RUN_START, &command)
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
        AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeSnapshot,
        AgentRuntimeStrategyHealth, AgentRuntimeStrategyHealthStatus, AgentRuntimeStrategyId,
        AgentRuntimeStrategyInfo, ApprovalAttentionState, AuthMethodId, AuthMethodRef,
        AuthProfileConnectionState, AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult,
        AuthProfileRef, AuthProfileState, DaemonAgentRuntimeAuthLoginParams,
        DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimePatchProfileParams,
        DaemonAgentRuntimeSetExtensionEnabledParams, PublicDaemonEvent, RunEvent, RunEventDelta,
        RunEventStreamItem, RunEventStreamPayload, RunId, RunStatus, RuntimeExtensionAvailability,
        RuntimeExtensionDescriptor, RuntimeExtensionId, RuntimeExtensionState, RuntimePolicyMode,
        RuntimeProfileId, RuntimeProfilePatch, RuntimeProfileSummary, SessionId, SessionOverview,
        SessionOverviewLaneStatus, SessionOverviewQuery, SessionOverviewResult, SessionStatus,
        WorkspaceId,
    };

    use super::*;
    use crate::credential_store::{load_session_authority, store_session_authority};

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
                        input_modalities: Vec::new(),
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
            event: PublicDaemonEvent::Run(RunEvent {
                run_id: run_id.clone(),
                status: RunStatus::Running,
                detail: format!("event-{sequence}"),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
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
