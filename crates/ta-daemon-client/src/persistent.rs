use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};
use ta_jsonrpc::{
    ClientConfig, JsonLineCodec, JsonRpcClientError, JsonRpcMessage, JsonRpcRequest,
    JsonRpcResponse, RequestId, SocketConnection, configure_connection_timeouts, connect_socket,
};
use ta_protocol::wire::{
    AgentRuntimeSnapshot, ApprovalSnapshotResult, ArtifactSnapshotResult, AuthProfileLoginResult,
    AuthProfileLogoutResult, DAEMON_PROTOCOL_VERSION, DaemonAgentRuntimeAuthLoginParams,
    DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSelectProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    DaemonApprovalDecideParams, DaemonApprovalDecideResult, DaemonClientCapabilities,
    DaemonInitializeParams, DaemonInitializeResult, DaemonSessionAttachParams,
    DaemonSessionAttachResult, DaemonSessionOpenParams, DaemonSessionOpenResult,
    GetAgentRuntimeQuery, ListApprovalsQuery, ListArtifactsQuery, ListRunsQuery, ListSessionsQuery,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_LIST,
    METHOD_DAEMON_INITIALIZE, METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_START,
    METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN,
    METHOD_DAEMON_SESSION_OVERVIEW, RunSummary, SessionOverviewQuery, SessionOverviewResult,
    SessionSummary, StartRunCommand,
};

use crate::credential_store::remove_session_authority;
use crate::credential_store::{load_session_authority, store_session_authority};

#[derive(Debug)]
pub struct PersistentDaemonClient {
    client_name: String,
    config: ClientConfig,
    codec: JsonLineCodec,
    next_request_id: i64,
    reader: BufReader<SocketConnection>,
}

impl PersistentDaemonClient {
    pub fn connect(config: ClientConfig, client_name: String) -> Result<Self, JsonRpcClientError> {
        let stream = connect_socket(&config.socket_address)?;
        configure_connection_timeouts(&stream, Some(config.io_timeout))
            .map_err(JsonRpcClientError::ConfigureTimeout)?;

        Ok(Self {
            client_name,
            config,
            codec: JsonLineCodec,
            next_request_id: 1,
            reader: BufReader::new(stream),
        })
    }

    pub fn initialize(
        &mut self,
        client_name: &str,
        client_version: &str,
        client_credential: Option<String>,
    ) -> Result<DaemonInitializeResult, JsonRpcClientError> {
        self.call(
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
        )
    }

    pub fn open_session(
        &mut self,
        title: &str,
    ) -> Result<DaemonSessionOpenResult, JsonRpcClientError> {
        let result: DaemonSessionOpenResult = self.call(
            METHOD_DAEMON_SESSION_OPEN,
            &DaemonSessionOpenParams {
                title: title.to_string(),
            },
        )?;
        store_session_authority(
            &self.config,
            &self.client_name,
            &result.session.id,
            &result.session_authority,
        )?;
        Ok(result)
    }

    pub fn attach_session(
        &mut self,
        session_id: ta_protocol::wire::SessionId,
    ) -> Result<DaemonSessionAttachResult, JsonRpcClientError> {
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

    pub fn decide_approval(
        &mut self,
        params: DaemonApprovalDecideParams,
    ) -> Result<DaemonApprovalDecideResult, JsonRpcClientError> {
        self.call(METHOD_DAEMON_APPROVAL_DECIDE, &params)
    }

    pub fn get_agent_runtime(&mut self) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_GET, &GetAgentRuntimeQuery {})
    }

    pub fn select_agent_runtime_profile(
        &mut self,
        params: DaemonAgentRuntimeSelectProfileParams,
    ) -> Result<AgentRuntimeSnapshot, JsonRpcClientError> {
        self.call(METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT, &params)
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
        &mut self,
        method: &str,
        params: &Params,
    ) -> Result<Response, JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        let request_id = RequestId::Integer(self.next_request_id);
        self.next_request_id += 1;
        let request = JsonRpcRequest::new(
            request_id.clone(),
            method,
            Some(serde_json::to_value(params).map_err(JsonRpcClientError::Serialize)?),
        );
        let response = self.send_request(request)?;
        serde_json::from_value(response.result).map_err(JsonRpcClientError::Deserialize)
    }

    fn send_request(
        &mut self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, JsonRpcClientError> {
        let line = self
            .codec
            .encode_message(&JsonRpcMessage::Request(request.clone()))?;
        let stream = self.reader.get_mut();
        stream
            .write_all(line.as_bytes())
            .map_err(JsonRpcClientError::Write)?;
        stream.flush().map_err(JsonRpcClientError::Flush)?;

        let mut response_line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut response_line)
            .map_err(|error| map_read_error(self.config.io_timeout, error))?;
        if bytes_read == 0 {
            return Err(JsonRpcClientError::ConnectionClosed);
        }

        match self.codec.decode_message(&response_line)? {
            JsonRpcMessage::Response(message) => {
                if message.id != request.id {
                    return Err(JsonRpcClientError::MismatchedResponseId {
                        expected: request.id,
                        actual: message.id,
                    });
                }
                Ok(message)
            }
            JsonRpcMessage::Error(message) => {
                if message.id.as_ref() != Some(&request.id) {
                    return Err(JsonRpcClientError::MismatchedErrorId {
                        expected: Some(request.id),
                        actual: message.id,
                    });
                }
                Err(JsonRpcClientError::Remote(message))
            }
            other => Err(JsonRpcClientError::UnexpectedMessage(other)),
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

fn map_read_error(timeout: Duration, error: io::Error) -> JsonRpcClientError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        return JsonRpcClientError::ResponseTimeout {
            timeout,
            source: error,
        };
    }

    JsonRpcClientError::Read(error)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::thread;
    use ta_jsonrpc::{
        INVALID_PARAMS_ERROR_CODE, JsonLineCodec, JsonRpcError, JsonRpcErrorObject, JsonRpcMessage,
        JsonRpcResponse, bind_listener,
    };
    use ta_protocol::wire::{
        AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeSelection, AgentRuntimeSnapshot,
        AgentRuntimeStrategyHealth, AgentRuntimeStrategyHealthStatus, AgentRuntimeStrategyId,
        AgentRuntimeStrategyInfo, ApprovalAttentionState, AuthProfileConnectionState,
        AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult, AuthProfileRef,
        AuthProfileState, DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
        DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSelectProfileParams,
        DaemonAgentRuntimeSetExtensionEnabledParams, RunStatus, RuntimeExtensionAvailability,
        RuntimeExtensionDescriptor, RuntimeExtensionId, RuntimeExtensionState, RuntimePolicyMode,
        RuntimeProfileId, RuntimeProfilePatch, RuntimeProfileSummary, SessionId, SessionOverview,
        SessionOverviewLaneStatus, SessionOverviewQuery, SessionOverviewResult, SessionStatus,
    };

    use super::*;
    use crate::credential_store::{load_session_authority, store_session_authority};

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
                selection: AgentRuntimeSelection {
                    runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                        .expect("runtime profile id"),
                },
                providers: vec![AgentRuntimeStrategyInfo {
                    id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                    display_name: "Codex".to_string(),
                    models: vec![AgentRuntimeModelRef {
                        id: AgentRuntimeModelId::new("gpt-5.4").expect("model id"),
                        display_name: "GPT-5.4".to_string(),
                        context_limit: None,
                        input_token_cost_micros: None,
                        output_token_cost_micros: None,
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
                auth_profiles: vec![AuthProfileState {
                    profile: AuthProfileRef {
                        id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
                        provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                        display_name: "Codex ChatGPT".to_string(),
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
                        id: "auth-codex-chatgpt".to_string(),
                        display_name: "Codex ChatGPT".to_string(),
                        management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    }],
                }],
                runtime_profiles: vec![RuntimeProfileSummary {
                    id: RuntimeProfileId::new("runtime-codex-safe").expect("runtime profile id"),
                    display_name: "Codex Safe".to_string(),
                    provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                    model_id: Some(AgentRuntimeModelId::new("gpt-5.4").expect("model id")),
                    auth_profile_id: Some(
                        AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
                    ),
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
                        id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
                        provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
                        display_name: "Codex ChatGPT".to_string(),
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
                        id: "auth-codex-chatgpt".to_string(),
                        display_name: "Codex ChatGPT".to_string(),
                        management_mode: ta_protocol::wire::AuthProfileManagementMode::Interactive,
                    }],
                },
                challenge: None,
            };
            let expected_logout = AuthProfileLogoutResult {
                auth_profile_id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
                disconnected: true,
            };

            for (expected_method, respond) in [
                (
                    METHOD_DAEMON_AGENT_RUNTIME_GET,
                    serde_json::to_value(expected_snapshot.clone()).expect("snapshot"),
                ),
                (
                    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
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
                    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT => {
                        let params: DaemonAgentRuntimeSelectProfileParams =
                            serde_json::from_value(params)
                                .expect("select params should deserialize");
                        assert_eq!(params.runtime_profile_id.as_str(), "runtime-codex-safe");
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH => {
                        let params: DaemonAgentRuntimePatchProfileParams =
                            serde_json::from_value(params)
                                .expect("patch params should deserialize");
                        assert_eq!(params.runtime_profile_id.as_str(), "runtime-codex-safe");
                        assert_eq!(params.patch.policy_mode, Some(RuntimePolicyMode::Allow));
                        assert_eq!(params.patch.model_id, None);
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN => {
                        let params: DaemonAgentRuntimeAuthLoginParams =
                            serde_json::from_value(params)
                                .expect("login params should deserialize");
                        assert_eq!(params.auth_profile_id.as_str(), "auth-codex-chatgpt");
                    }
                    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT => {
                        let params: DaemonAgentRuntimeAuthLogoutParams =
                            serde_json::from_value(params)
                                .expect("logout params should deserialize");
                        assert_eq!(params.auth_profile_id.as_str(), "auth-codex-chatgpt");
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
        assert_eq!(
            snapshot.selection.runtime_profile_id.as_str(),
            "runtime-codex-safe"
        );

        let selected = client
            .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
                runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
            })
            .expect("select should succeed");
        assert_eq!(selected.runtime_profiles.len(), 1);

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
                auth_profile_id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
            })
            .expect("login should succeed");
        assert_eq!(login.auth_profile.profile.id.as_str(), "auth-codex-chatgpt");

        let logout = client
            .logout_agent_runtime_auth_profile(DaemonAgentRuntimeAuthLogoutParams {
                auth_profile_id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
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
