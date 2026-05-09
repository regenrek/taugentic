use super::*;

#[test]
fn daemon_context_receipt_methods_dispatch_and_scope_by_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let selected_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let other_session = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "other".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
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
    let selected_run = ensure_running_run(&state, &selected_session.id, "selected");
    let other_run = ensure_running_run(&state, &other_session.id, "other");
    for suffix in ["promote", "quarantine"] {
        state
            .app
            .record_artifact(ArtifactRecord {
                id: ArtifactId::new(format!("artifact-{suffix}")).expect("artifact id"),
                session_id: selected_session.id.clone(),
                run_id: selected_run.body.id.clone(),
                kind: ArtifactKind::Patch,
                storage_path: format!("artifacts/{suffix}/patch.diff"),
            })
            .expect("artifact should record");
    }
    state
        .app
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-other").expect("artifact id"),
            session_id: other_session.id.clone(),
            run_id: other_run.body.id,
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/other/patch.diff".to_string(),
        })
        .expect("other artifact should record");

    let list_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(81),
            method: METHOD_DAEMON_CONTEXT_RECEIPTS_LIST.to_string(),
            params: Some(
                serde_json::to_value(ListReceiptsRequest {
                    session_id: selected_session.id.clone(),
                    run_id: Some(selected_run.body.id),
                    parent_run_id: None,
                    state: Some(ReceiptState::Returned),
                    kind: Some(ReceiptKind::Patch),
                    limit: Some(10),
                })
                .expect("params"),
            ),
        },
    )
    .expect("receipt list should succeed");
    let listed: ListReceiptsResult =
        serde_json::from_value(list_response).expect("list response should deserialize");
    assert_eq!(listed.receipts.len(), 2);

    let promote_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(82),
            method: METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE.to_string(),
            params: Some(
                serde_json::to_value(PromoteReceiptRequest {
                    session_id: selected_session.id.clone(),
                    receipt_id: listed.receipts[0].id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("receipt promote should succeed");
    let promoted: ContextReceipt =
        serde_json::from_value(promote_response).expect("promote response should deserialize");
    assert_eq!(promoted.state, ReceiptState::Promoted);

    let illegal_transition = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(85),
            method: METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE.to_string(),
            params: Some(
                serde_json::to_value(QuarantineReceiptRequest {
                    session_id: selected_session.id.clone(),
                    receipt_id: listed.receipts[0].id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("promoted receipt must not quarantine");
    assert_eq!(illegal_transition.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(
        illegal_transition
            .message
            .contains("cannot quarantine promoted receipt")
    );

    let quarantine_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(83),
            method: METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE.to_string(),
            params: Some(
                serde_json::to_value(QuarantineReceiptRequest {
                    session_id: selected_session.id.clone(),
                    receipt_id: listed.receipts[1].id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("receipt quarantine should succeed");
    let quarantined: ContextReceipt = serde_json::from_value(quarantine_response)
        .expect("quarantine response should deserialize");
    assert_eq!(quarantined.state, ReceiptState::Quarantined);

    let other_receipt = state
        .app
        .list_receipts(
            &other_session.id,
            &ListReceiptsRequest {
                session_id: other_session.id.clone(),
                run_id: None,
                parent_run_id: None,
                state: None,
                kind: None,
                limit: None,
            },
        )
        .expect("other receipts")
        .receipts
        .into_iter()
        .next()
        .expect("other receipt");
    let rejected = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(84),
            method: METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE.to_string(),
            params: Some(
                serde_json::to_value(PromoteReceiptRequest {
                    session_id: selected_session.id.clone(),
                    receipt_id: other_receipt.id,
                })
                .expect("params"),
            ),
        },
    );

    assert!(rejected.is_err());
}
