use ta_work_source::{WorkItem, WorkItemKey, WorkItemStatus, WorkSource};

use super::*;

#[test]
fn daemon_work_item_list_returns_store_items() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state(None);
    let session = test_session();
    let item = work_item("1");
    state
        .app
        .seed_work_item_for_tests(item.clone())
        .expect("work item seed");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(70),
            method: METHOD_DAEMON_WORK_ITEM_LIST.to_string(),
            params: Some(serde_json::to_value(WorkItemListQuery {}).expect("params")),
        },
    )
    .expect("work item list should succeed");

    let listed: WorkItemListResult = serde_json::from_value(response).expect("list response");
    assert_eq!(listed.items, vec![item]);
}

#[test]
fn daemon_work_item_dismiss_updates_store_item() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state(None);
    let session = test_session();
    let item = work_item("2");
    state
        .app
        .seed_work_item_for_tests(item.clone())
        .expect("work item seed");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(71),
            method: METHOD_DAEMON_WORK_ITEM_DISMISS.to_string(),
            params: Some(
                serde_json::to_value(WorkItemDismissParams {
                    key: item.key.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("work item dismiss should succeed");

    let dismissed: WorkItemDismissResult =
        serde_json::from_value(response).expect("dismiss response");
    assert_eq!(
        dismissed.item.map(|item| item.status),
        Some(WorkItemStatus::Dismissed)
    );
}

#[test]
fn daemon_work_item_trigger_reuses_run_start_path() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = initialized_session_state(Some(opened.id.clone()));
    let session = test_session();
    let item = work_item("3");
    state
        .app
        .seed_work_item_for_tests(item.clone())
        .expect("work item seed");
    load_test_workflow(&state);

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(72),
            method: METHOD_DAEMON_WORK_ITEM_TRIGGER.to_string(),
            params: Some(
                serde_json::to_value(WorkItemTriggerParams {
                    key: item.key.clone(),
                    recipe_id: Some("debug-agent".to_string()),
                })
                .expect("params"),
            ),
        },
    )
    .expect("work item trigger should succeed");

    let triggered: WorkItemTriggerResult =
        serde_json::from_value(response).expect("trigger response");
    let runs = state.app.list_runs(&opened.id).expect("runs should list");
    assert_eq!(triggered.item.status, WorkItemStatus::Triggered);
    assert_eq!(
        triggered.item.triggered_run_id.as_deref(),
        Some(triggered.run.id.as_str())
    );
    assert!(runs.iter().any(|run| run.id == triggered.run.id));
    assert!(
        triggered
            .run
            .objective
            .contains("Work on background item 3")
    );
}

#[test]
fn daemon_work_item_trigger_requires_loaded_workflow() {
    with_test_config_home("work-item-trigger-requires-workflow", || {
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let opened = state
            .app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let session_state = initialized_session_state(Some(opened.id.clone()));
        let session = test_session();
        let item = work_item("workflow-required");
        state
            .app
            .seed_work_item_for_tests(item.clone())
            .expect("work item seed");

        let error = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(74),
                method: METHOD_DAEMON_WORK_ITEM_TRIGGER.to_string(),
                params: Some(
                    serde_json::to_value(WorkItemTriggerParams {
                        key: item.key.clone(),
                        recipe_id: Some("debug-agent".to_string()),
                    })
                    .expect("params"),
                ),
            },
        )
        .expect_err("work item trigger should require workflow");

        assert!(error.message.contains("background workflow is not loaded"));
    });
}

#[test]
fn daemon_work_item_refresh_reports_daemon_side_queue() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = initialized_session_state(None);
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(73),
            method: METHOD_DAEMON_WORK_ITEM_REFRESH.to_string(),
            params: Some(serde_json::to_value(WorkItemRefreshParams {}).expect("params")),
        },
    )
    .expect("work item refresh should succeed");

    let refreshed: WorkItemListResult = serde_json::from_value(response).expect("refresh response");
    assert_eq!(
        refreshed.sync.state,
        crate::WorkSourceSyncState::RefreshQueued
    );
}

fn initialized_session_state(session_id: Option<SessionId>) -> Arc<Mutex<DaemonRpcSessionState>> {
    Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: session_id,
    }))
}

fn load_test_workflow<S>(state: &crate::host::bootstrap::BootstrapState<S>)
where
    S: ta_store::PersistenceStore + Send + 'static,
{
    let file = tempfile::NamedTempFile::new().expect("workflow file");
    std::fs::write(file.path(), test_workflow_yaml()).expect("workflow write");
    state
        .app
        .load_workflow(&crate::WorkflowLoadParams {
            path: file.path().display().to_string(),
        })
        .expect("workflow load")
        .loaded
        .expect("workflow should be loaded");
}

fn test_workflow_yaml() -> &'static str {
    r#"
kind: taugentic.workflow/v1
name: test-background
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
  per_capsule: {}
  per_orchestrator: {}
  per_workflow: {}
"#
}

fn work_item(number: &str) -> WorkItem {
    WorkItem {
        key: WorkItemKey::github("regenrek", "taugentic", number).expect("work item key"),
        source: WorkSource::GitHub {
            repo_owner: "regenrek".to_string(),
            repo_name: "taugentic".to_string(),
        },
        external_id: number.to_string(),
        title: format!("GitHub issue #{number}"),
        body: "Implement the background item".to_string(),
        labels: vec!["ready".to_string()],
        url: format!("https://github.com/regenrek/taugentic/issues/{number}"),
        fetched_at_ms: 100,
        status: WorkItemStatus::Available,
        triggered_run_id: None,
    }
}
