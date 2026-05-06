use ta_protocol::wire::{
    DelegateRequest, OutputContractKind, RunEvent, RunId, RunStatus, WorkspaceMode,
    WorktreeCleanupPolicy,
};

#[test]
fn delegate_request_roundtrip_without_recipe_id() {
    let json = serde_json::json!({
        "objective": "Review this patch",
        "outputContract": "review"
    });

    let decoded: DelegateRequest =
        serde_json::from_value(json.clone()).expect("delegate request should deserialize");
    let encoded = serde_json::to_value(&decoded).expect("delegate request should serialize");

    assert_eq!(
        decoded,
        DelegateRequest {
            objective: "Review this patch".to_string(),
            output_contract: Some(OutputContractKind::Review),
            model_id: None,
            sandbox_profile: None,
            recipe_id: None,
            workspace_scope: WorkspaceMode::WorktreeWrite,
            cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
        }
    );
    assert_eq!(
        encoded,
        serde_json::json!({
            "objective": "Review this patch",
            "outputContract": "review",
            "workspaceScope": "worktreeWrite",
            "cleanupPolicy": "deleteOnSuccess"
        })
    );
}

#[test]
fn delegate_request_roundtrip_with_recipe_id() {
    let request = DelegateRequest {
        objective: "Create the patch".to_string(),
        output_contract: Some(OutputContractKind::Patch),
        model_id: None,
        sandbox_profile: None,
        recipe_id: Some("patch-agent".to_string()),
        workspace_scope: WorkspaceMode::WorktreeWrite,
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
        planned_write_files: Vec::new(),
    };

    let json = serde_json::to_value(&request).expect("delegate request should serialize");
    let decoded: DelegateRequest =
        serde_json::from_value(json.clone()).expect("delegate request should deserialize");

    assert_eq!(decoded, request);
    assert_eq!(json["recipeId"], "patch-agent");
}

#[test]
fn lineage_event_roundtrip_without_recipe_id() {
    let json = serde_json::json!({
        "runId": "run-1",
        "status": "running",
        "detail": "Execution started"
    });

    let decoded: RunEvent = serde_json::from_value(json.clone()).expect("run event deserialize");
    let encoded = serde_json::to_value(&decoded).expect("run event should serialize");

    assert_eq!(
        decoded,
        RunEvent {
            run_id: RunId::new("run-1").expect("run id"),
            status: RunStatus::Running,
            detail: "Execution started".to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        }
    );
    assert_eq!(encoded, json);
}

#[test]
fn lineage_event_roundtrip_with_recipe_id() {
    let event = RunEvent {
        run_id: RunId::new("run-1").expect("run id"),
        status: RunStatus::Running,
        detail: "Execution started".to_string(),
        output_contract: None,
        recipe_id: Some("review-agent".to_string()),
        result: None,
    };

    let json = serde_json::to_value(&event).expect("run event should serialize");
    let decoded: RunEvent = serde_json::from_value(json.clone()).expect("run event roundtrip");

    assert_eq!(decoded, event);
    assert_eq!(json["recipeId"], "review-agent");
}

#[test]
fn delegate_request_serializes_camel_case_recipe_id() {
    let request = DelegateRequest {
        objective: "Plan next steps".to_string(),
        output_contract: None,
        model_id: None,
        sandbox_profile: None,
        recipe_id: Some("plan-agent".to_string()),
        workspace_scope: WorkspaceMode::WorktreeWrite,
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
        planned_write_files: Vec::new(),
    };

    let json = serde_json::to_value(&request).expect("delegate request should serialize");

    assert_eq!(json["recipeId"], "plan-agent");
    assert!(json.get("recipe_id").is_none());
}

#[test]
fn delegate_request_omits_recipe_id_when_none() {
    let request = DelegateRequest {
        objective: "Debug failure".to_string(),
        output_contract: None,
        model_id: None,
        sandbox_profile: None,
        recipe_id: None,
        workspace_scope: WorkspaceMode::WorktreeWrite,
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
        planned_write_files: Vec::new(),
    };

    let json = serde_json::to_value(&request).expect("delegate request should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "objective": "Debug failure",
            "workspaceScope": "worktreeWrite",
            "cleanupPolicy": "deleteOnSuccess"
        })
    );
}
