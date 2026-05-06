use super::*;

#[test]
fn daemon_artifact_list_returns_session_scoped_artifacts() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let selected_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "selected".to_string(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(selected_session.id.clone()),
    }));
    let session = test_session();
    let other_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "other".to_string(),
            },
        )
        .expect("session should open");
    let selected_run = ensure_running_run(&state, &selected_session.id, "selected");
    let other_run = ensure_running_run(&state, &other_session.id, "other");
    let selected_artifact = state
        .app
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new(format!("artifact-{}", uuid::Uuid::new_v4().simple()))
                .expect("artifact id"),
            session_id: selected_session.id.clone(),
            run_id: selected_run.body.id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        })
        .expect("selected artifact");
    state
        .app
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new(format!("artifact-{}", uuid::Uuid::new_v4().simple()))
                .expect("artifact id"),
            session_id: other_session.id.clone(),
            run_id: other_run.body.id,
            kind: ArtifactKind::Transcript,
            storage_path: "artifacts/run-2/transcript.md".to_string(),
        })
        .expect("other artifact");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(35),
            method: METHOD_DAEMON_ARTIFACT_LIST.to_string(),
            params: Some(
                serde_json::to_value(ListArtifactsQuery {
                    run_id: None,
                    artifact_id: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.artifact.list should succeed");

    let artifacts: ArtifactSnapshotResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(artifacts.items.len(), 1);
    assert_eq!(artifacts.items[0].run_id, selected_run.body.id);
    assert_eq!(artifacts.items[0].id, selected_artifact.body.id);
    assert_eq!(
        artifacts.items[0].storage_path,
        "artifacts/run-1/patch.diff"
    );
    assert!(artifacts.latest_cursor.is_some());
}

#[test]
fn daemon_artifact_get_returns_only_selected_session_artifact() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let selected_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "selected".to_string(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(selected_session.id.clone()),
    }));
    let session = test_session();
    let _other_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "other".to_string(),
            },
        )
        .expect("session should open");
    let selected_run = ensure_running_run(&state, &selected_session.id, "selected");
    let selected_artifact = state
        .app
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new(format!("artifact-{}", uuid::Uuid::new_v4().simple()))
                .expect("artifact id"),
            session_id: selected_session.id.clone(),
            run_id: selected_run.body.id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        })
        .expect("selected artifact");

    let selected_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(36),
            method: METHOD_DAEMON_ARTIFACT_GET.to_string(),
            params: Some(
                serde_json::to_value(GetArtifactQuery {
                    artifact_id: selected_artifact.body.id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.artifact.get should succeed");

    let other_session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(SessionId::new("session-2").expect("session id")),
    }));
    let other_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &other_session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(37),
            method: METHOD_DAEMON_ARTIFACT_GET.to_string(),
            params: Some(
                serde_json::to_value(GetArtifactQuery {
                    artifact_id: selected_artifact.body.id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.artifact.get should return attached-session scoped none");

    let selected: Option<ArtifactSummary> =
        serde_json::from_value(selected_response).expect("response should deserialize");
    let other: Option<ArtifactSummary> =
        serde_json::from_value(other_response).expect("response should deserialize");
    assert_eq!(
        selected.expect("artifact should exist").run_id,
        selected_run.body.id
    );
    assert_eq!(other, None);
}
