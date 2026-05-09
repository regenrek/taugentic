use super::*;
use crate::{OpenSessionRequest, SessionSummary};
use ta_protocol::wire::{
    CapsuleResult, OutputContractKind, PatchResult, ReceiptProvenance, ValidationError,
};
use ta_store::{CreateReceipt, ReceiptRepository};

#[test]
fn run_detail_maps_running_run_without_completion_fields() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Running detail");
    let run = native_run_projection("run-running-detail", &session.id, RunStatus::Running, 100);
    seed_run_projection(&service, run.clone());

    let detail = get_detail(&service, &session.id, &run.id);

    assert_eq!(detail.summary.id, run.id);
    assert_eq!(detail.summary.status, RunStatus::Running);
    assert_eq!(detail.result, None);
    assert_eq!(detail.contract_violation, None);
    assert_eq!(detail.quarantine_receipt, None);
}

#[test]
fn run_detail_maps_completed_result_and_native_contract_fields() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Completed detail");
    let parent_run_id = RunId::new("run-parent-detail").expect("parent run id");
    let result = patch_result();
    let run = native_child_projection(
        &session.id,
        &parent_run_id,
        NativeChildProjectionInput {
            run_id: "run-completed-detail",
            status: RunStatus::Completed,
            output_contract: Some(OutputContractKind::Patch),
            recipe_id: Some("patch-native-subagent".to_string()),
            result: Some(result.clone()),
            contract_violation: None,
        },
    );
    seed_run_projection(&service, run.clone());

    let detail = get_detail(&service, &session.id, &run.id);

    assert_eq!(detail.summary.status, RunStatus::Completed);
    assert_eq!(detail.result, Some(result));
    assert_eq!(detail.contract_violation, None);
    assert_eq!(detail.output_contract, Some(OutputContractKind::Patch));
    assert_eq!(detail.recipe_id.as_deref(), Some("patch-native-subagent"));
    assert_eq!(detail.parent_run_id, Some(parent_run_id));
}

#[test]
fn run_detail_maps_contract_violation_and_quarantine_receipt() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Quarantine detail");
    let parent_run_id = RunId::new("run-parent-quarantine").expect("parent run id");
    let violation = ValidationError::KindMismatch {
        expected: OutputContractKind::Patch,
        got: OutputContractKind::Debug,
    };
    let run = native_child_projection(
        &session.id,
        &parent_run_id,
        NativeChildProjectionInput {
            run_id: "run-quarantined-detail",
            status: RunStatus::Failed,
            output_contract: Some(OutputContractKind::Patch),
            recipe_id: Some("patch-native-subagent".to_string()),
            result: None,
            contract_violation: Some(violation.clone()),
        },
    );
    seed_run_projection(&service, run.clone());
    seed_quarantine_receipt(&service, &session.id, &run.id, &parent_run_id);

    let detail = get_detail(&service, &session.id, &run.id);
    let receipt = detail
        .quarantine_receipt
        .expect("quarantine receipt should project");

    assert_eq!(detail.summary.status, RunStatus::Failed);
    assert_eq!(detail.result, None);
    assert_eq!(detail.contract_violation, Some(violation));
    assert_eq!(receipt.state, ReceiptState::Quarantined);
    assert_eq!(receipt.parent_run_id, Some(parent_run_id));
    assert_eq!(
        receipt.provenance.stream_cursor.as_deref(),
        Some("run:run-quarantined-detail:event:42")
    );
}

#[test]
fn run_detail_preserves_non_native_parent_and_omits_native_contract_fields() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Fork detail");
    let parent_run_id = RunId::new("run-parent-fork").expect("parent run id");
    let run = RunProjection {
        id: RunId::new("run-fork-detail").expect("run id"),
        session_id: session.id.clone(),
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        objective: "Forked detail".to_string(),
        status: RunStatus::Completed,
        harness: RunHarnessKind::Native,
        source: RunSource::Forked {
            parent_run_id: parent_run_id.clone(),
            parent_event_seq: 7,
        },
        result: None,
        contract_violation: None,
        started_at_ms: Some(100),
        ended_at_ms: Some(200),
        last_event_seq: Some(9),
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
    };
    seed_run_projection(&service, run.clone());

    let detail = get_detail(&service, &session.id, &run.id);

    assert_eq!(detail.parent_run_id, Some(parent_run_id));
    assert_eq!(detail.output_contract, None);
    assert_eq!(detail.recipe_id, None);
}

fn open_test_session(service: &AppService, title: &str) -> SessionSummary {
    service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: title.to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open")
        .session
}

fn get_detail(service: &AppService, session_id: &SessionId, run_id: &RunId) -> crate::RunDetail {
    service
        .get_run(
            session_id,
            &GetRunQuery {
                run_id: run_id.clone(),
            },
        )
        .expect("run detail should load")
        .expect("run detail should exist")
}

struct NativeChildProjectionInput<'a> {
    run_id: &'a str,
    status: RunStatus,
    output_contract: Option<OutputContractKind>,
    recipe_id: Option<String>,
    result: Option<CapsuleResult>,
    contract_violation: Option<ValidationError>,
}

fn native_child_projection(
    session_id: &SessionId,
    parent_run_id: &RunId,
    input: NativeChildProjectionInput<'_>,
) -> RunProjection {
    RunProjection {
        id: RunId::new(input.run_id).expect("run id"),
        session_id: session_id.clone(),
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        objective: format!("Objective {}", input.run_id),
        status: input.status,
        harness: RunHarnessKind::Native,
        source: RunSource::NativeSubagent {
            parent_run_id: parent_run_id.clone(),
            parent_turn_id: AgentStreamTurnId::new("turn-run-detail").expect("turn id"),
            output_contract: input.output_contract,
            model_id: None,
            sandbox_profile: None,
            recipe_id: input.recipe_id,
            workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
            cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
        },
        result: input.result,
        contract_violation: input.contract_violation,
        started_at_ms: Some(100),
        ended_at_ms: Some(200),
        last_event_seq: Some(42),
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
    }
}

fn patch_result() -> CapsuleResult {
    CapsuleResult::Patch(PatchResult {
        patch_receipt_ids: vec!["receipt-patch".to_string()],
        touched_files: vec![
            "crates/ta-orchestrator/src/orchestration/app/tests/run_detail.rs".to_string(),
        ],
        tests_run_receipt_ids: vec!["receipt-tests".to_string()],
        passing: true,
        blockers: Vec::new(),
    })
}

fn seed_quarantine_receipt(
    service: &AppService,
    session_id: &SessionId,
    run_id: &RunId,
    parent_run_id: &RunId,
) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    let receipt = store
        .create(CreateReceipt {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            parent_run_id: Some(parent_run_id.clone()),
            kind: ReceiptKind::Patch,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: Some(AgentStreamTurnId::new("turn-run-detail").expect("turn id")),
                event_seq: Some(42),
                stream_cursor: Some(format!("run:{}:event:42", run_id.as_str())),
            },
            title: Some("Patch result".to_string()),
            summary: Some("Patch CapsuleResult quarantined after daemon validation".to_string()),
        })
        .expect("receipt should create");
    store
        .quarantine(&receipt.id)
        .expect("receipt should quarantine");
}
