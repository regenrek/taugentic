use super::*;

#[test]
fn workflow_status_reports_unloaded_by_default() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state();
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(90),
            method: METHOD_WORKFLOW_STATUS.to_string(),
            params: None,
        },
    )
    .expect("workflow status should succeed");

    let status: WorkflowStatusResult = serde_json::from_value(response).expect("workflow status");
    assert!(status.loaded.is_none());
}

#[test]
fn workflow_load_status_reload_and_validate_roundtrip() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state();
    let session = test_session();
    let file = tempfile::NamedTempFile::new().expect("workflow file");
    std::fs::write(file.path(), workflow_yaml("first")).expect("workflow write");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(91),
            method: METHOD_WORKFLOW_LOAD.to_string(),
            params: Some(
                serde_json::to_value(WorkflowLoadParams {
                    path: file.path().display().to_string(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("workflow load should succeed");
    let loaded: WorkflowStatusResult = serde_json::from_value(response).expect("load status");
    assert_eq!(loaded.loaded.expect("loaded").name, "first");

    std::fs::write(file.path(), workflow_yaml("second")).expect("workflow rewrite");
    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(92),
            method: METHOD_WORKFLOW_RELOAD.to_string(),
            params: None,
        },
    )
    .expect("workflow reload should succeed");
    let reloaded: WorkflowStatusResult = serde_json::from_value(response).expect("reload status");
    assert_eq!(reloaded.loaded.expect("loaded").name, "second");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(93),
            method: METHOD_WORKFLOW_VALIDATE.to_string(),
            params: Some(
                serde_json::to_value(WorkflowValidateParams {
                    path: None,
                    contents: Some(workflow_yaml("dry-run")),
                })
                .expect("params"),
            ),
        },
    )
    .expect("workflow validate should succeed");
    let report: WorkflowValidationReport =
        serde_json::from_value(response).expect("validation report");
    assert!(report.valid);
}

#[test]
fn workflow_reload_failure_keeps_last_known_good() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state();
    let session = test_session();
    let file = tempfile::NamedTempFile::new().expect("workflow file");
    std::fs::write(file.path(), workflow_yaml("first")).expect("workflow write");

    handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(94),
            method: METHOD_WORKFLOW_LOAD.to_string(),
            params: Some(
                serde_json::to_value(WorkflowLoadParams {
                    path: file.path().display().to_string(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("workflow load should succeed");
    std::fs::write(file.path(), "kind: nope").expect("workflow rewrite");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(95),
            method: METHOD_WORKFLOW_RELOAD.to_string(),
            params: None,
        },
    )
    .expect("workflow reload should succeed");
    let status: WorkflowStatusResult = serde_json::from_value(response).expect("reload status");

    assert_eq!(status.loaded.expect("loaded").name, "first");
    assert!(matches!(
        status.last_reload,
        Some(WorkflowReloadOutcome::Failed { .. })
    ));
}

fn initialized_session_state() -> Arc<Mutex<DaemonRpcSessionState>> {
    Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }))
}

fn workflow_yaml(name: &str) -> String {
    format!(
        r#"
kind: taugentic.workflow/v1
name: {name}
source:
  kind: github_issues
  repo: regenrek/taugentic
  active_states: ["ready"]
  terminal_states: ["done"]
orchestrator:
  max_concurrent_missions: 2
  max_capsules_per_mission: 3
  retry:
    initial_ms: 1000
    max_ms: 10000
policy:
  approvals:
    file_write: ask
    process: ask
    network: allowlist
  network_allowlist: [github.com]
runtime_profiles:
  implementer:
    provider: codex
    model: gpt-5.6-sol
outputs:
  required: [tests, patch_or_blocker]
budgets:
  per_capsule: {{}}
  per_orchestrator: {{}}
  per_workflow: {{}}
"#
    )
}
