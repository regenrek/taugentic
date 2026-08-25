use super::*;

#[test]
fn list_artifacts_filters_by_session_run_and_artifact() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session_a = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session A".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_b = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session B".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run_a = ensure_running_run(&service, &session_a.id, "one");
    let run_b = ensure_running_run(&service, &session_b.id, "two");
    let artifact_a = ArtifactId::new("artifact-a").expect("artifact id");

    service
        .record_artifact(ArtifactRecord {
            id: artifact_a.clone(),
            session_id: session_a.id.clone(),
            run_id: run_a.body.id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-a/patch.diff".to_string(),
        })
        .expect("artifact should record");
    service
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-b").expect("artifact id"),
            session_id: session_b.id.clone(),
            run_id: run_b.body.id,
            kind: ArtifactKind::Transcript,
            storage_path: "artifacts/run-b/transcript.md".to_string(),
        })
        .expect("artifact should record");

    let artifacts = service
        .list_artifacts(
            &session_a.id,
            &ListArtifactsQuery {
                run_id: Some(run_a.body.id.clone()),
                artifact_id: Some(artifact_a),
            },
        )
        .expect("artifacts");

    assert_eq!(artifacts.items.len(), 1);
    assert_eq!(artifacts.items[0].run_id, run_a.body.id);
    assert_eq!(artifacts.items[0].kind, ArtifactKind::Patch);
    assert_eq!(
        artifacts.items[0].storage_path,
        "artifacts/run-a/patch.diff"
    );
    assert!(artifacts.latest_cursor.is_some());
}

#[test]
fn record_artifact_returns_trimmed_summary_and_deferred_records() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Artifacts".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = ensure_running_run(&service, &session.id, "record");

    let recorded = service
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-a").expect("artifact id"),
            session_id: session.id.clone(),
            run_id: started.body.id,
            kind: ArtifactKind::Patch,
            storage_path: " artifacts/run-a/patch.diff ".to_string(),
        })
        .expect("artifact should record");

    assert_eq!(recorded.body.storage_path, "artifacts/run-a/patch.diff");
    assert_eq!(recorded.deferred_records.len(), 2);
    assert!(matches!(
        &recorded.deferred_records[0].payload,
        DaemonEvent::Artifact(ArtifactEvent { artifact })
            if artifact.id == recorded.body.id
                && artifact.storage_path == "artifacts/run-a/patch.diff"
    ));
    assert!(matches!(
        &recorded.deferred_records[1].payload,
        DaemonEvent::ContextReceipt(ContextReceiptEvent::Created { receipt })
            if receipt.kind == ReceiptKind::Patch
                && receipt.provenance.artifact_id.as_ref() == Some(&recorded.body.id)
    ));
}

#[test]
fn record_artifact_auto_creates_native_receipts_with_kind_mapping() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Receipts".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = ensure_running_run(&service, &session.id, "receipt mapping");
    let cases = [
        (ArtifactKind::Patch, ReceiptKind::Patch, "patch.diff"),
        (
            ArtifactKind::FileSnapshot,
            ReceiptKind::Evidence,
            "snapshot.txt",
        ),
        (
            ArtifactKind::Transcript,
            ReceiptKind::Artifact,
            "transcript.md",
        ),
        (
            ArtifactKind::CommandLog,
            ReceiptKind::Artifact,
            "command.log",
        ),
    ];

    for (index, (artifact_kind, receipt_kind, name)) in cases.iter().enumerate() {
        let artifact_id = ArtifactId::new(format!("artifact-map-{index}")).expect("artifact id");
        service
            .record_artifact(ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session.id.clone(),
                run_id: started.body.id.clone(),
                kind: *artifact_kind,
                storage_path: format!("artifacts/run-a/{name}"),
            })
            .expect("artifact should record");
        let receipts = service
            .list_receipts(
                &session.id,
                &ListReceiptsRequest {
                    session_id: session.id.clone(),
                    run_id: Some(started.body.id.clone()),
                    parent_run_id: None,
                    state: Some(ReceiptState::Returned),
                    kind: Some(*receipt_kind),
                    limit: None,
                },
            )
            .expect("receipts should list");
        assert!(receipts.receipts.iter().any(|receipt| {
            receipt.provenance.artifact_id.as_ref() == Some(&artifact_id)
                && receipt.kind == *receipt_kind
        }));
    }
}

#[test]
fn record_artifact_skips_receipt_for_external_harness() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "External".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = ensure_running_run(&service, &session.id, "external artifact");
    {
        let mut store = service.store.lock().expect("store lock");
        let mut run = store
            .run(&started.body.id)
            .expect("run lookup")
            .expect("run should exist");
        run.harness = RunHarnessKind::Acp;
        store.save_run(run).expect("run should save");
    }

    let recorded = service
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-external").expect("artifact id"),
            session_id: session.id.clone(),
            run_id: started.body.id,
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/external/patch.diff".to_string(),
        })
        .expect("external artifact should record");
    let receipts = service
        .list_receipts(
            &session.id,
            &ListReceiptsRequest {
                session_id: session.id.clone(),
                run_id: None,
                parent_run_id: None,
                state: None,
                kind: None,
                limit: None,
            },
        )
        .expect("receipts should list");

    assert_eq!(recorded.deferred_records.len(), 1);
    assert!(receipts.receipts.is_empty());
}

#[test]
fn receipt_rpc_app_methods_filter_and_transition() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Receipt RPC".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let parent_run_id = RunId::new("run-parent").expect("run id");
    let started = ensure_running_run(&service, &session.id, "subagent artifact");
    {
        let mut store = service.store.lock().expect("store lock");
        let mut run = store
            .run(&started.body.id)
            .expect("run lookup")
            .expect("run should exist");
        run.source = RunSource::NativeSubagent {
            parent_run_id: parent_run_id.clone(),
            parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("turn id"),
            output_contract: None,
            model_id: None,
            recipe_id: None,
            workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
            cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
        };
        store.save_run(run).expect("run should save");
    }
    service
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-transition").expect("artifact id"),
            session_id: session.id.clone(),
            run_id: started.body.id.clone(),
            kind: ArtifactKind::FileSnapshot,
            storage_path: "artifacts/run-a/snapshot.txt".to_string(),
        })
        .expect("artifact should record");
    let listed = service
        .list_receipts(
            &session.id,
            &ListReceiptsRequest {
                session_id: session.id.clone(),
                run_id: Some(started.body.id),
                parent_run_id: Some(parent_run_id),
                state: Some(ReceiptState::Returned),
                kind: Some(ReceiptKind::Evidence),
                limit: Some(10),
            },
        )
        .expect("receipts should list");
    assert_eq!(listed.receipts.len(), 1);
    let receipt_id = listed.receipts[0].id.clone();

    let quarantined = service
        .quarantine_receipt(
            &session.id,
            &QuarantineReceiptRequest {
                session_id: session.id.clone(),
                receipt_id: receipt_id.clone(),
            },
        )
        .expect("receipt should quarantine");
    assert_eq!(quarantined.body.state, ReceiptState::Quarantined);
    let promoted = service
        .promote_receipt(
            &session.id,
            &PromoteReceiptRequest {
                session_id: session.id.clone(),
                receipt_id: receipt_id.clone(),
            },
        )
        .expect("quarantined receipt can promote");
    assert_eq!(promoted.body.state, ReceiptState::Promoted);
    let error = service
        .quarantine_receipt(
            &session.id,
            &QuarantineReceiptRequest {
                session_id: session.id.clone(),
                receipt_id,
            },
        )
        .expect_err("promoted receipt must not quarantine");
    assert!(matches!(
        error,
        AppServiceError::ReceiptTransitionViolation { .. }
    ));
}

#[test]
fn get_artifact_returns_summary_only_for_selected_session() {
    let service = AppService::bootstrap().expect("app service should boot");
    let selected_session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let other_session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Other".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let selected_run = ensure_running_run(&service, &selected_session.id, "selected");
    let selected_artifact = ArtifactId::new("artifact-1").expect("artifact id");

    service
        .record_artifact(ArtifactRecord {
            id: selected_artifact.clone(),
            session_id: selected_session.id.clone(),
            run_id: selected_run.body.id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        })
        .expect("artifact should record");

    let selected = service
        .get_artifact(
            &selected_session.id,
            &GetArtifactQuery {
                artifact_id: selected_artifact.clone(),
            },
        )
        .expect("artifact lookup should work");
    let listed = service
        .list_artifacts(
            &selected_session.id,
            &ListArtifactsQuery {
                run_id: None,
                artifact_id: Some(selected_artifact.clone()),
            },
        )
        .expect("artifact list should work");
    let other = service
        .get_artifact(
            &other_session.id,
            &GetArtifactQuery {
                artifact_id: selected_artifact,
            },
        )
        .expect("artifact lookup should work");

    assert_eq!(
        selected
            .as_ref()
            .expect("artifact should exist")
            .storage_path,
        "artifacts/run-1/patch.diff"
    );
    assert_eq!(listed.items.len(), 1);
    assert_eq!(Some(listed.items[0].clone()), selected);
    assert!(listed.latest_cursor.is_some());
    assert_eq!(other, None);
}
