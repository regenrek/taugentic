use super::*;

#[test]
fn daemon_status_returns_ready_payload() {
    let mut config = test_config();
    config.server = ServerConfig::local_default("ta-daemon", DAEMON_DEFAULT_SOCKET_NAME);
    let state = boot(config);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(1),
            method: METHOD_DAEMON_STATUS.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect("daemon.status should succeed");

    let status: DaemonStatusResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert!(status.ready);
    assert_eq!(status.runtime_mode, state.config.runtime_mode);
    assert_eq!(
        status.socket_path,
        state.config.socket_address().to_string()
    );
    assert_eq!(
        status.log_path,
        state.config.log_path().display().to_string()
    );
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn daemon_diagnostics_snapshot_returns_runtime_payload() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(2),
            method: METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect("daemon.diagnostics.snapshot should succeed");

    let diagnostics: DaemonDiagnostics =
        serde_json::from_value(response).expect("response should deserialize");
    assert!(diagnostics.in_flight_rpc_count >= 1);
    assert_eq!(diagnostics.in_flight_capsule_run_count, 0);
    assert_eq!(diagnostics.worktree_count, 0);
    assert_eq!(diagnostics.claim_count, 0);
    assert_eq!(diagnostics.recent_error_count, 0);
    assert_eq!(
        diagnostics.sandbox.sandbox_kind,
        state.runtime.host_platform.capabilities.sandbox.to_string()
    );
    assert_eq!(diagnostics.provider_health.len(), 12);
}

#[test]
fn daemon_diagnostics_snapshot_requires_initialize_first() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(3),
            method: METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("daemon.diagnostics.snapshot should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT));
}

#[test]
fn daemon_session_list_returns_daemon_owned_read_models() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");
    state
        .app
        .open_session(
            "other-client",
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Ignore me".to_string(),
            },
        )
        .expect("other session should open");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(12),
            method: METHOD_DAEMON_SESSION_LIST.to_string(),
            params: Some(serde_json::to_value(ListSessionsQuery {}).expect("params")),
        },
    )
    .expect("daemon.session.list should succeed");

    let sessions: Vec<SessionSummary> =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, opened.id);
    assert_eq!(sessions[0].title, "Build daemon app server");
}

#[test]
fn daemon_session_list_requires_initialize_first() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(13),
            method: METHOD_DAEMON_SESSION_LIST.to_string(),
            params: Some(serde_json::to_value(ListSessionsQuery {}).expect("params")),
        },
    )
    .expect_err("daemon.session.list should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_SESSION_LIST));
}

#[test]
fn daemon_agent_runtime_get_requires_initialize_first() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(14),
            method: METHOD_DAEMON_AGENT_RUNTIME_GET.to_string(),
            params: Some(serde_json::to_value(GetAgentRuntimeQuery {}).expect("params")),
        },
    )
    .expect_err("daemon.agent.runtime.get should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_AGENT_RUNTIME_GET));
}

#[test]
fn daemon_agent_runtime_get_returns_snapshot_without_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(15),
            method: METHOD_DAEMON_AGENT_RUNTIME_GET.to_string(),
            params: Some(serde_json::to_value(GetAgentRuntimeQuery {}).expect("params")),
        },
    )
    .expect("daemon.agent.runtime.get should succeed");

    let snapshot: AgentRuntimeSnapshot =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        snapshot.selection.runtime_profile_id.as_str(),
        "runtime-codex-safe"
    );
    assert_eq!(snapshot.providers.len(), 13);
    for provider_id in [
        "codex",
        "openai",
        "anthropic",
        "local-model",
        "deepseek",
        "groq",
        "openrouter",
        "xai",
        "codex-acp",
        "claude-acp",
        "cursor",
        "opencode",
        "copilot-acp",
    ] {
        assert!(
            snapshot
                .providers
                .iter()
                .any(|provider| provider.id.as_str() == provider_id),
            "snapshot should contain provider {provider_id}"
        );
    }
    assert_eq!(snapshot.auth_profiles.len(), 9);
    assert!(
        snapshot
            .auth_profiles
            .iter()
            .any(|profile| profile.profile.id.as_str() == "auth-codex-chatgpt")
    );
    assert!(
        snapshot
            .auth_profiles
            .iter()
            .any(|profile| profile.profile.id.as_str() == "auth-codex-api-key")
    );
    for auth_profile_id in [
        "auth-openai-api-key",
        "auth-openai-chatgpt",
        "auth-anthropic-api-key",
        "deepseek-api-key",
        "groq-api-key",
        "openrouter-api-key",
        "xai-api-key",
    ] {
        assert!(
            snapshot
                .auth_profiles
                .iter()
                .any(|profile| profile.profile.id.as_str() == auth_profile_id),
            "snapshot should contain auth profile {auth_profile_id}"
        );
    }
    assert_eq!(snapshot.runtime_profiles.len(), 45);
    assert!(
        snapshot
            .runtime_profiles
            .iter()
            .any(|profile| profile.id.as_str() == "runtime-codex-safe")
    );
    for runtime_profile_id in [
        "runtime-openai-safe",
        "runtime-openai-chatgpt-safe",
        "runtime-openai-chatgpt-allow",
        "runtime-openai-chatgpt-deny",
        "runtime-anthropic-safe",
        "runtime-local-ollama",
        "runtime-local-lm-studio",
        "runtime-local-llama-cpp",
        "runtime-local-vllm",
        "runtime-local-tgi",
        "runtime-local-custom",
        "runtime-openrouter-safe",
        "runtime-codex-acp-safe",
        "runtime-claude-acp-safe",
        "runtime-cursor-safe",
        "runtime-opencode-safe",
        "runtime-copilot-acp-safe",
    ] {
        assert!(
            snapshot
                .runtime_profiles
                .iter()
                .any(|profile| profile.id.as_str() == runtime_profile_id),
            "snapshot should contain runtime profile {runtime_profile_id}"
        );
    }
    assert_eq!(snapshot.runtime_extensions.len(), 1);
}

#[test]
fn daemon_agent_runtime_get_is_initialized_but_not_principal_scoped() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: None,
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(151),
            method: METHOD_DAEMON_AGENT_RUNTIME_GET.to_string(),
            params: Some(serde_json::to_value(GetAgentRuntimeQuery {}).expect("params")),
        },
    )
    .expect("daemon.agent.runtime.get should not require principal id");

    let snapshot: AgentRuntimeSnapshot =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        snapshot.selection.runtime_profile_id.as_str(),
        "runtime-codex-safe"
    );
}

#[test]
fn daemon_agent_runtime_profile_select_is_init_only_not_session_attached() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(16),
            method: METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT.to_string(),
            params: Some(
                serde_json::to_value(DaemonAgentRuntimeSelectProfileParams {
                    runtime_profile_id: crate::RuntimeProfileId::new("runtime-codex-allow")
                        .expect("runtime profile id"),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.agent.runtime.profile.select should succeed");

    let snapshot: AgentRuntimeSnapshot =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        snapshot.selection.runtime_profile_id.as_str(),
        "runtime-codex-allow"
    );
}

#[test]
fn daemon_agent_runtime_profile_patch_updates_selected_profile_without_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(160),
            method: METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH.to_string(),
            params: Some(
                serde_json::to_value(crate::DaemonAgentRuntimePatchProfileParams {
                    runtime_profile_id: crate::RuntimeProfileId::new("runtime-codex-safe")
                        .expect("runtime profile id"),
                    patch: crate::RuntimeProfilePatch {
                        policy_mode: Some(crate::RuntimePolicyMode::Allow),
                        ..Default::default()
                    },
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.agent.runtime.profile.patch should succeed");

    let snapshot: AgentRuntimeSnapshot =
        serde_json::from_value(response).expect("response should deserialize");
    let selected_profile = snapshot
        .runtime_profiles
        .iter()
        .find(|profile| profile.id.as_str() == "runtime-codex-safe")
        .expect("runtime profile should exist");

    assert_eq!(
        snapshot.selection.runtime_profile_id.as_str(),
        "runtime-codex-safe"
    );
    assert_eq!(
        selected_profile.policy_mode,
        crate::RuntimePolicyMode::Allow
    );
}

#[test]
fn daemon_agent_runtime_extension_set_updates_snapshot_without_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(161),
            method: METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET.to_string(),
            params: Some(
                serde_json::to_value(crate::DaemonAgentRuntimeSetExtensionEnabledParams {
                    extension_id: crate::RuntimeExtensionId::new("local-shell-tools")
                        .expect("extension id"),
                    enabled: false,
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.agent.runtime.extension.set should succeed");

    let snapshot: AgentRuntimeSnapshot =
        serde_json::from_value(response).expect("response should deserialize");
    let extension = snapshot
        .runtime_extensions
        .iter()
        .find(|extension| extension.descriptor.id.as_str() == "local-shell-tools")
        .expect("runtime extension should exist");

    assert!(!extension.enabled);
}

#[test]
fn daemon_agent_runtime_auth_login_unknown_profile_returns_invalid_params() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(162),
            method: METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN.to_string(),
            params: Some(
                serde_json::to_value(crate::DaemonAgentRuntimeAuthLoginParams {
                    auth_profile_id: crate::AuthProfileId::new("auth-missing")
                        .expect("auth profile id"),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.agent.runtime.auth.login should reject unknown profiles");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("auth profile does not exist"));
}

#[test]
fn daemon_agent_runtime_auth_logout_unknown_profile_returns_invalid_params() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(163),
            method: METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT.to_string(),
            params: Some(
                serde_json::to_value(crate::DaemonAgentRuntimeAuthLogoutParams {
                    auth_profile_id: crate::AuthProfileId::new("auth-missing")
                        .expect("auth profile id"),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.agent.runtime.auth.logout should reject unknown profiles");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("auth profile does not exist"));
}
