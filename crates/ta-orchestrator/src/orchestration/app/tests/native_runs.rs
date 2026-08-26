use super::*;

#[test]
fn list_native_runs_filters_children_and_paginates() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Native runs".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    seed_run_projection(
        &service,
        native_run_projection("run-parent-a", &session.id, RunStatus::Completed, 100),
    );
    seed_run_projection(
        &service,
        native_run_projection("run-parent-b", &session.id, RunStatus::Running, 300),
    );
    seed_run_projection(
        &service,
        RunProjection {
            harness: RunHarnessKind::Acp,
            ..native_run_projection("run-external", &session.id, RunStatus::Running, 400)
        },
    );
    seed_run_projection(
        &service,
        RunProjection {
            source: RunSource::NativeSubagent {
                route: ta_store::default_test_run_source().route().clone(),
                parent_run_id: RunId::new("run-parent-b").expect("parent run id"),
                parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("turn id"),
                output_contract: None,
                model_id: None,
                recipe_id: None,
                workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
                cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
                planned_write_files: Vec::new(),
            },
            ..native_run_projection("run-child", &session.id, RunStatus::Completed, 500)
        },
    );

    let first = service
        .list_native_runs(
            &session.id,
            &ListNativeRunsRequest {
                filter: None,
                limit: 1,
                cursor: None,
            },
        )
        .expect("first page");
    let second = service
        .list_native_runs(
            &session.id,
            &ListNativeRunsRequest {
                filter: None,
                limit: 1,
                cursor: first.next_cursor.clone(),
            },
        )
        .expect("second page");
    let children = service
        .list_native_runs(
            &session.id,
            &ListNativeRunsRequest {
                filter: Some(RunListFilter {
                    harness: None,
                    status: Some(vec![RunStatus::Completed]),
                    parent_run_id: Some(RunId::new("run-parent-b").expect("parent run id")),
                }),
                limit: 10,
                cursor: None,
            },
        )
        .expect("children");

    assert_eq!(first.runs[0].id.as_str(), "run-parent-b");
    assert_eq!(second.runs[0].id.as_str(), "run-parent-a");
    assert_eq!(second.next_cursor, None);
    assert_eq!(children.runs.len(), 1);
    assert_eq!(children.runs[0].id.as_str(), "run-child");
    assert_eq!(
        children.runs[0]
            .parent_run_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("run-parent-b")
    );
}

#[test]
fn list_native_runs_rejects_zero_and_over_max_limit() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Native run limits".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");

    let zero = service.list_native_runs(
        &session.id,
        &ListNativeRunsRequest {
            filter: None,
            limit: 0,
            cursor: None,
        },
    );
    let over_max = service.list_native_runs(
        &session.id,
        &ListNativeRunsRequest {
            filter: None,
            limit: NATIVE_RUN_LIST_MAX_LIMIT + 1,
            cursor: None,
        },
    );

    assert!(matches!(
        zero,
        Err(AppServiceError::InvalidNativeRunListLimit {
            max: NATIVE_RUN_LIST_MAX_LIMIT
        })
    ));
    let Err(over_max) = over_max else {
        panic!("over max limit should fail");
    };
    assert!(matches!(
        over_max,
        AppServiceError::InvalidNativeRunListLimit {
            max: NATIVE_RUN_LIST_MAX_LIMIT
        }
    ));
    assert_eq!(
        over_max.to_string(),
        format!(
            "native run list limit must be between 1 and {}",
            NATIVE_RUN_LIST_MAX_LIMIT
        )
    );
}

#[test]
fn list_native_runs_allows_max_limit() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Native max limit".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");

    let page = service
        .list_native_runs(
            &session.id,
            &ListNativeRunsRequest {
                filter: None,
                limit: NATIVE_RUN_LIST_MAX_LIMIT,
                cursor: None,
            },
        )
        .expect("max limit should be accepted");

    assert!(page.runs.is_empty());
}
