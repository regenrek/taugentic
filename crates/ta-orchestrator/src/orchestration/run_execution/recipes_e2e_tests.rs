use super::test_support::{
    app_and_execution_with_runtime, open_session, set_default_test_workspace_root,
    validated_runtime_selection,
};
use super::*;
use crate::{ListReceiptsRequest, ReceiptState};
use ta_protocol::wire::{
    AgentRuntimeModelId, AgentStreamTurnId, CapsuleResult, DebugResult, GetRunQuery,
    ListNativeRunsRequest, OutputContractKind, PatchResult, PlanResult, PlanStep, ReceiptKind,
    ReviewResult, ReviewVerdict, RunEvent, RunListFilter, TestResult, ValidationError,
    WorkspaceMode, WorktreeCleanupPolicy,
};
use ta_store::{
    CommitRepository, CommitRunTransition, EventLogRepository, InMemoryStore, ProjectionRepository,
};
use taugentic_agent::NativeChildRunRequest;

const BUILTIN_RECIPES: [RecipeCase; 5] = [
    RecipeCase::new("debug-agent", OutputContractKind::Debug),
    RecipeCase::new("patch-agent", OutputContractKind::Patch),
    RecipeCase::new("review-agent", OutputContractKind::Review),
    RecipeCase::new("test-agent", OutputContractKind::Test),
    RecipeCase::new("plan-agent", OutputContractKind::Plan),
];

#[derive(Clone, Copy)]
struct RecipeCase {
    id: &'static str,
    contract: OutputContractKind,
}

impl RecipeCase {
    const fn new(id: &'static str, contract: OutputContractKind) -> Self {
        Self { id, contract }
    }
}

#[test]
fn all_builtin_recipes_complete_with_valid_capsule_results_and_promote_receipts() {
    for case in BUILTIN_RECIPES {
        let (app, execution, session, parent) = native_parent(&format!("Recipe {}", case.id));
        let child = delegate_recipe(&execution, &session.id, &parent.id, case.id, None, None)
            .expect("recipe delegate should start a native child run");
        let stored_child = run(&execution, &child.run_id);
        let result = fake_codex_capsule_result(case.contract);

        assert_native_subagent_source(
            &stored_child,
            &parent.id,
            case.id,
            Some(case.contract),
            Some(DEFAULT_RECIPE_MODEL),
        );
        mark_child_running_for_capsule_completion(&execution, &session.id, &child.run_id);

        let completed = execution
            .complete_run_with_result(
                session.id.clone(),
                &child.run_id,
                "normal end".to_string(),
                Some(result.clone()),
            )
            .expect("valid recipe result should complete");

        assert_eq!(completed.run.status, RunStatus::Completed);
        assert_run_event(
            &execution,
            &session.id,
            &child.run_id,
            RunStatus::Completed,
            Some(case.contract),
            Some(case.id),
            Some(&result),
        );
        assert_receipt(
            &app,
            &session.id,
            &child.run_id,
            &parent.id,
            ReceiptState::Promoted,
            receipt_kind(case.contract),
        );
    }
}

#[test]
fn recipe_delegate_prose_completion_fails_with_structured_error_and_quarantines() {
    let (app, execution, session, parent) = native_parent("Recipe drift");
    let child = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "debug-agent",
        None,
        None,
    )
    .expect("debug recipe delegate should start");
    let prose_result = serde_json::from_str::<CapsuleResult>("debugging is complete").ok();
    mark_child_running_for_capsule_completion(&execution, &session.id, &child.run_id);

    let error = execution
        .complete_run_with_result(
            session.id.clone(),
            &child.run_id,
            "normal end".to_string(),
            prose_result,
        )
        .expect_err("prose completion should fail daemon validation");

    assert!(matches!(
        error,
        RunExecutionError::OutputContractViolation(ValidationError::Custom(message))
            if message.contains("requires a matching CapsuleResult")
    ));
    assert_eq!(run(&execution, &child.run_id).status, RunStatus::Failed);
    assert_run_event(
        &execution,
        &session.id,
        &child.run_id,
        RunStatus::Failed,
        Some(OutputContractKind::Debug),
        Some("debug-agent"),
        None,
    );
    assert_receipt(
        &app,
        &session.id,
        &child.run_id,
        &parent.id,
        ReceiptState::Quarantined,
        ReceiptKind::Summary,
    );
}

#[test]
fn recipe_delegate_conflicting_contract_returns_typed_error_before_child_start() {
    let (_app, execution, session, parent) = native_parent("Recipe conflict");
    let run_count_before = run_count(&execution);

    let error = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "debug-agent",
        Some(OutputContractKind::Patch),
        None,
    )
    .expect_err("conflicting recipe contract should fail");

    assert!(matches!(
        error,
        RunExecutionError::RecipeContractConflict {
            recipe_id,
            recipe_contract: OutputContractKind::Debug,
            request_contract: OutputContractKind::Patch,
        } if recipe_id == "debug-agent"
    ));
    assert_eq!(run_count(&execution), run_count_before);
}

#[test]
fn recipe_delegate_unknown_recipe_returns_typed_error_before_child_start() {
    let (_app, execution, session, parent) = native_parent("Unknown recipe");
    let run_count_before = run_count(&execution);

    let error = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "bogus-recipe",
        None,
        None,
    )
    .expect_err("unknown recipe should fail");

    assert!(matches!(
        error,
        RunExecutionError::UnknownRecipeId(recipe_id) if recipe_id == "bogus-recipe"
    ));
    assert_eq!(run_count(&execution), run_count_before);
}

#[test]
fn recipe_delegate_applies_default_model_and_allows_model_override() {
    let (_app, execution, session, parent) = native_parent("Default recipe model");
    let default_child = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "debug-agent",
        None,
        None,
    )
    .expect("defaulted recipe delegate should start");
    assert_native_subagent_source(
        &run(&execution, &default_child.run_id),
        &parent.id,
        "debug-agent",
        Some(OutputContractKind::Debug),
        Some(DEFAULT_RECIPE_MODEL),
    );

    let (_app, execution, session, parent) = native_parent("Override recipe model");
    let override_child = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "debug-agent",
        None,
        Some(model_id("override")),
    )
    .expect("model override recipe delegate should start");
    assert_native_subagent_source(
        &run(&execution, &override_child.run_id),
        &parent.id,
        "debug-agent",
        Some(OutputContractKind::Debug),
        Some("override"),
    );
}

#[test]
fn recipe_delegate_lineage_events_carry_recipe_id() {
    let (_app, execution, session, parent) = native_parent("Recipe lineage");
    let child = delegate_recipe(
        &execution,
        &session.id,
        &parent.id,
        "debug-agent",
        None,
        None,
    )
    .expect("debug recipe delegate should start");

    assert_native_subagent_source(
        &run(&execution, &child.run_id),
        &parent.id,
        "debug-agent",
        Some(OutputContractKind::Debug),
        Some(DEFAULT_RECIPE_MODEL),
    );
    assert_run_event(
        &execution,
        &session.id,
        &child.run_id,
        RunStatus::Queued,
        None,
        Some("debug-agent"),
        None,
    );
}

#[test]
fn delegate_without_recipe_id_preserves_legacy_flow() {
    let (_app, execution, session, parent) = native_parent("Legacy delegate");
    let child = execution
        .start_native_child_run(
            session.id.clone(),
            NativeChildRunRequest::new(
                parent.id.clone(),
                turn_id("turn-legacy"),
                "Legacy child objective",
                None,
                Some(model_id("legacy-model")),
                None,
            )
            .expect("legacy child request"),
        )
        .expect("legacy delegate should start");
    let stored_child = run(&execution, &child.run_id);

    assert_eq!(stored_child.objective, "Legacy child objective");
    assert_legacy_source(&stored_child, &parent.id, Some("legacy-model"));
    mark_child_running_for_capsule_completion(&execution, &session.id, &child.run_id);

    let completed = execution
        .complete_run_with_result(
            session.id.clone(),
            &child.run_id,
            "normal end".to_string(),
            None,
        )
        .expect("legacy completion without contract should complete");

    assert_eq!(completed.run.status, RunStatus::Completed);
    assert_run_event(
        &execution,
        &session.id,
        &child.run_id,
        RunStatus::Completed,
        None,
        None,
        None,
    );
}

#[test]
fn worktree_parallel_delegation_records_conflicts_receipts_and_cleanup() {
    let repo = clean_git_repo_fixture();
    let runtime = runtime_for_clean_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Parallel worktree delegation");
    let selection = validated_runtime_selection(&app, "runtime-openai-safe");
    let parent = execution
        .seed_running_run_for_tests(
            session.id.clone(),
            "Parent native run".to_string(),
            selection,
        )
        .expect("parent should seed")
        .run;
    const OVERLAP_FILE: &str = "apps/desktop/package.json";

    let first = start_worktree_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-worktree-a",
        "Patch package metadata A",
        WorktreeCleanupPolicy::DeleteOnSuccess,
        OVERLAP_FILE,
    );
    let second = start_worktree_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-worktree-b",
        "Patch package metadata B",
        WorktreeCleanupPolicy::Keep,
        OVERLAP_FILE,
    );
    assert_eq!(first.status, RunStatus::WaitingForApproval);
    assert_eq!(second.status, RunStatus::WaitingForApproval);

    mark_child_running_for_capsule_completion(&execution, &session.id, &first.run_id);
    mark_child_running_for_capsule_completion(&execution, &session.id, &second.run_id);
    let first_run = run(&execution, &first.run_id);
    let second_run = run(&execution, &second.run_id);
    assert_ne!(
        first_run.execution_context.effective_cwd,
        second_run.execution_context.effective_cwd
    );

    let first_worktree = std::path::PathBuf::from(
        first_run
            .workspace_info
            .as_ref()
            .expect("first workspace info")
            .path
            .clone(),
    );
    let second_worktree = std::path::PathBuf::from(
        second_run
            .workspace_info
            .as_ref()
            .expect("second workspace info")
            .path
            .clone(),
    );
    assert!(first_worktree.exists());
    assert!(second_worktree.exists());
    assert_ne!(
        first_run.workspace_info.as_ref().map(|info| &info.branch),
        second_run.workspace_info.as_ref().map(|info| &info.branch)
    );
    assert_eq!(first_run.claimed_files, vec![OVERLAP_FILE.to_string()]);
    assert_eq!(second_run.claimed_files, vec![OVERLAP_FILE.to_string()]);
    assert_eq!(
        second_run
            .conflict_summary
            .as_ref()
            .map(|summary| (summary.warning_count, summary.files.clone())),
        Some((1, vec![OVERLAP_FILE.to_string()]))
    );
    assert_conflict_event(&execution, &session.id, &second.run_id, &first.run_id);

    let listed = app
        .list_native_runs(
            &session.id,
            &ListNativeRunsRequest {
                filter: Some(RunListFilter {
                    harness: None,
                    status: None,
                    parent_run_id: Some(parent.id.clone()),
                }),
                limit: 50,
                cursor: None,
            },
        )
        .expect("native runs should list");
    let listed_second = listed
        .runs
        .iter()
        .find(|run| run.id == second.run_id)
        .expect("second child should be in native run list");
    assert_eq!(
        listed_second
            .conflict_summary
            .as_ref()
            .map(|summary| summary.warning_count),
        Some(1)
    );
    let second_detail = app
        .get_run(
            &session.id,
            &GetRunQuery {
                run_id: second.run_id.clone(),
            },
        )
        .expect("run detail query should succeed")
        .expect("run detail should exist");
    assert!(second_detail.workspace_info.is_some());
    assert_eq!(second_detail.claimed_files, vec![OVERLAP_FILE.to_string()]);
    assert_eq!(
        second_detail
            .conflict_summary
            .as_ref()
            .map(|summary| summary.warning_count),
        Some(1)
    );

    let first_result = patch_result("receipt-patch-a", OVERLAP_FILE);
    let second_result = patch_result("receipt-patch-b", OVERLAP_FILE);
    let first_completed = execution
        .complete_run_with_result(
            session.id.clone(),
            &first.run_id,
            "first patch complete".to_string(),
            Some(first_result.clone()),
        )
        .expect("first child should complete");
    let second_completed = execution
        .complete_run_with_result(
            session.id.clone(),
            &second.run_id,
            "second patch complete".to_string(),
            Some(second_result.clone()),
        )
        .expect("second child should complete");

    assert_eq!(first_completed.run.status, RunStatus::Completed);
    assert_eq!(second_completed.run.status, RunStatus::Completed);
    assert!(!first_worktree.exists());
    assert!(second_worktree.exists());
    assert_receipt(
        &app,
        &session.id,
        &first.run_id,
        &parent.id,
        ReceiptState::Promoted,
        ReceiptKind::Patch,
    );
    assert_receipt(
        &app,
        &session.id,
        &second.run_id,
        &parent.id,
        ReceiptState::Promoted,
        ReceiptKind::Patch,
    );

    let third = start_worktree_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-worktree-c",
        "Patch package metadata C",
        WorktreeCleanupPolicy::DeleteOnSuccess,
        OVERLAP_FILE,
    );
    mark_child_running_for_capsule_completion(&execution, &session.id, &third.run_id);
    assert!(run(&execution, &third.run_id).conflict_summary.is_none());
    execution
        .complete_run_with_result(
            session.id.clone(),
            &third.run_id,
            "third patch complete".to_string(),
            Some(patch_result("receipt-patch-c", OVERLAP_FILE)),
        )
        .expect("third child should complete");
}

const DEFAULT_RECIPE_MODEL: &str = "claude-4.6-sonnet-medium-thinking";

fn native_parent(
    title: &str,
) -> (
    crate::orchestration::AppService<InMemoryStore>,
    RunExecutionService<InMemoryStore>,
    crate::SessionSummary,
    RunSummary,
) {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, title);
    let selection = validated_runtime_selection(&app, "runtime-openai-safe");
    let parent = execution
        .seed_running_run_for_tests(session.id.clone(), format!("{title} parent"), selection)
        .expect("parent should seed")
        .run;
    (app, execution, session, parent)
}

fn delegate_recipe(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &crate::SessionId,
    parent_run_id: &RunId,
    recipe_id: &str,
    output_contract: Option<OutputContractKind>,
    model_id: Option<AgentRuntimeModelId>,
) -> Result<taugentic_agent::NativeChildRunResult, RunExecutionError> {
    execution.start_native_child_run(
        session_id.clone(),
        NativeChildRunRequest::new(
            parent_run_id.clone(),
            turn_id(&format!("turn-{recipe_id}")),
            format!("Execute {recipe_id} recipe objective"),
            output_contract,
            model_id,
            Some(recipe_id.to_string()),
        )
        .expect("recipe child request"),
    )
}

fn start_worktree_child(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &crate::SessionId,
    parent_run_id: &RunId,
    turn_id_value: &str,
    objective: &str,
    cleanup_policy: WorktreeCleanupPolicy,
    planned_file: &str,
) -> taugentic_agent::NativeChildRunResult {
    execution
        .start_native_child_run(
            session_id.clone(),
            NativeChildRunRequest::new(
                parent_run_id.clone(),
                turn_id(turn_id_value),
                objective,
                Some(OutputContractKind::Patch),
                None,
                None,
            )
            .expect("worktree child request")
            .with_workspace_scope(WorkspaceMode::WorktreeWrite)
            .with_cleanup_policy(cleanup_policy)
            .with_planned_write_files(vec![planned_file.to_string()]),
        )
        .expect("worktree child should start")
}

fn patch_result(receipt_id: &str, touched_file: &str) -> CapsuleResult {
    CapsuleResult::Patch(PatchResult {
        patch_receipt_ids: vec![receipt_id.to_string()],
        touched_files: vec![touched_file.to_string()],
        tests_run_receipt_ids: vec![format!("{receipt_id}-tests")],
        passing: true,
        blockers: Vec::new(),
    })
}

fn clean_git_repo_fixture() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp repo");
    dispatch_git(repo.path(), ["init"]);
    dispatch_git(repo.path(), ["config", "user.email", "agent@example.test"]);
    dispatch_git(repo.path(), ["config", "user.name", "Agent Test"]);
    std::fs::write(repo.path().join(".gitignore"), "target/\n").expect("gitignore");
    let desktop_dir = repo.path().join("apps/desktop");
    std::fs::create_dir_all(&desktop_dir).expect("desktop dir");
    std::fs::write(desktop_dir.join("package.json"), "{\"name\":\"fixture\"}\n")
        .expect("package fixture");
    dispatch_git(repo.path(), ["add", "."]);
    dispatch_git(repo.path(), ["commit", "-m", "initial"]);
    repo
}

fn dispatch_git<const N: usize>(repo: &std::path::Path, args: [&str; N]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn runtime_for_clean_repo(repo: &std::path::Path) -> crate::RuntimeService {
    crate::RuntimeService::from_host_platform_with_paths(
        ta_host_platform::detect_current_platform(),
        crate::RuntimeExecutionPaths {
            artifact_root: repo.join("target/daemon-artifacts"),
        },
    )
}

fn assert_conflict_event(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &crate::SessionId,
    run_id: &RunId,
    holding_run_id: &RunId,
) {
    let events = execution
        .store
        .lock()
        .expect("store lock")
        .events_for_session(session_id)
        .expect("events should load");
    assert!(
        events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Conflict(crate::ConflictEvent::Warning { run_id: event_run_id, warning })
                    if event_run_id == run_id
                        && warning
                            .conflicts
                            .iter()
                            .any(|conflict| &conflict.holding_capsule == holding_run_id)
            )
        }),
        "missing conflict warning for {run_id:?} held by {holding_run_id:?}"
    );
}

fn fake_codex_capsule_result(contract: OutputContractKind) -> CapsuleResult {
    let json = serde_json::to_string(&capsule_result(contract))
        .expect("fake codex capsule result should serialize");
    serde_json::from_str(&json).expect("fake codex capsule result should deserialize")
}

fn capsule_result(contract: OutputContractKind) -> CapsuleResult {
    match contract {
        OutputContractKind::Debug => CapsuleResult::Debug(DebugResult {
            reproduced: false,
            root_cause: None,
            evidence_receipt_ids: Vec::new(),
            patch_receipt_id: None,
            confidence: 0.75,
            blockers: Vec::new(),
        }),
        OutputContractKind::Patch => CapsuleResult::Patch(PatchResult {
            patch_receipt_ids: vec!["receipt-patch".to_string()],
            touched_files: vec!["crates/example.rs".to_string()],
            tests_run_receipt_ids: vec!["receipt-tests".to_string()],
            passing: true,
            blockers: Vec::new(),
        }),
        OutputContractKind::Review => CapsuleResult::Review(ReviewResult {
            verdict: ReviewVerdict::Approve,
            findings: Vec::new(),
            risks: Vec::new(),
            touched_files: vec!["crates/example.rs".to_string()],
        }),
        OutputContractKind::Test => CapsuleResult::Test(TestResult {
            total: 2,
            passed: 2,
            failed: 0,
            skipped: 0,
            failed_test_names: Vec::new(),
            log_receipt_ids: vec!["receipt-test-log".to_string()],
        }),
        OutputContractKind::Plan => CapsuleResult::Plan(PlanResult {
            steps: vec![PlanStep {
                title: "Verify recipe flow".to_string(),
                description: Some("Assert structured completion and receipts".to_string()),
                estimated_minutes: Some(5),
                depends_on: Vec::new(),
            }],
            estimated_total_minutes: Some(5),
            risks: Vec::new(),
        }),
        OutputContractKind::Custom => CapsuleResult::Custom(serde_json::json!({ "ok": true })),
    }
}

fn receipt_kind(contract: OutputContractKind) -> ReceiptKind {
    match contract {
        OutputContractKind::Debug | OutputContractKind::Plan => ReceiptKind::Summary,
        OutputContractKind::Patch => ReceiptKind::Patch,
        OutputContractKind::Review => ReceiptKind::ReviewFinding,
        OutputContractKind::Test => ReceiptKind::TestOutput,
        OutputContractKind::Custom => ReceiptKind::Artifact,
    }
}

fn mark_child_running_for_capsule_completion(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &crate::SessionId,
    run_id: &RunId,
) {
    let existing_run = run(execution, run_id);
    let recipe_id = recipe_id_for_run(&existing_run);
    {
        let mut store = execution.store.lock().expect("store lock");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: ta_store::RunProjection {
                    status: RunStatus::Running,
                    ..existing_run
                },
                events: vec![DaemonEvent::Run(RunEvent {
                    run_id: run_id.clone(),
                    status: RunStatus::Running,
                    detail: "Seeded live recipe child for capsule completion".to_string(),
                    output_contract: None,
                    recipe_id,
                    result: None,
                })],
                occurred_at_ms: current_time_ms(),
            })
            .expect("child running transition should commit");
    }
    execution
        .runtime
        .claim_live_run(run_id.clone(), session_id.clone());
}

fn assert_native_subagent_source(
    run: &ta_store::RunProjection,
    expected_parent_run_id: &RunId,
    expected_recipe_id: &str,
    expected_contract: Option<OutputContractKind>,
    expected_model_id: Option<&str>,
) {
    let RunSource::NativeSubagent {
        parent_run_id,
        output_contract,
        model_id,
        recipe_id,
        ..
    } = &run.source
    else {
        panic!("expected native subagent source");
    };

    assert_eq!(parent_run_id, expected_parent_run_id);
    assert_eq!(*output_contract, expected_contract);
    assert_eq!(model_id.as_ref().map(|id| id.as_str()), expected_model_id);
    assert_eq!(recipe_id.as_deref(), Some(expected_recipe_id));
}

fn assert_legacy_source(
    run: &ta_store::RunProjection,
    expected_parent_run_id: &RunId,
    expected_model_id: Option<&str>,
) {
    let RunSource::NativeSubagent {
        parent_run_id,
        output_contract,
        model_id,
        recipe_id,
        ..
    } = &run.source
    else {
        panic!("expected native subagent source");
    };

    assert_eq!(parent_run_id, expected_parent_run_id);
    assert_eq!(*output_contract, None);
    assert_eq!(model_id.as_ref().map(|id| id.as_str()), expected_model_id);
    assert_eq!(recipe_id, &None);
}

fn assert_run_event(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &crate::SessionId,
    run_id: &RunId,
    status: RunStatus,
    output_contract: Option<OutputContractKind>,
    recipe_id: Option<&str>,
    result: Option<&CapsuleResult>,
) {
    let events = execution
        .store
        .lock()
        .expect("store lock")
        .events_for_session(session_id)
        .expect("events should load");
    assert!(
        events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(RunEvent {
                    run_id: event_run_id,
                    status: event_status,
                    output_contract: event_output_contract,
                    recipe_id: event_recipe_id,
                    result: event_result,
                    ..
                }) if event_run_id == run_id
                    && *event_status == status
                    && *event_output_contract == output_contract
                    && event_recipe_id.as_deref() == recipe_id
                    && event_result.as_ref() == result
            )
        }),
        "missing run event for {run_id:?} status {status:?} recipe {recipe_id:?}"
    );
}

fn assert_receipt(
    app: &crate::orchestration::AppService<InMemoryStore>,
    session_id: &crate::SessionId,
    run_id: &RunId,
    parent_run_id: &RunId,
    state: ReceiptState,
    kind: ReceiptKind,
) {
    let receipts = app
        .list_receipts(
            session_id,
            &ListReceiptsRequest {
                session_id: session_id.clone(),
                run_id: Some(run_id.clone()),
                parent_run_id: Some(parent_run_id.clone()),
                state: Some(state),
                kind: Some(kind),
                limit: None,
            },
        )
        .expect("receipts should list");

    assert_eq!(receipts.receipts.len(), 1);
    let receipt = &receipts.receipts[0];
    assert_eq!(receipt.parent_run_id.as_ref(), Some(parent_run_id));
    assert_eq!(receipt.state, state);
    assert_eq!(receipt.kind, kind);
    assert!(receipt.provenance.agent_turn_id.is_some());
    assert!(
        receipt
            .provenance
            .stream_cursor
            .as_deref()
            .is_some_and(|cursor| cursor.contains(run_id.as_str()))
    );
}

fn run(execution: &RunExecutionService<InMemoryStore>, run_id: &RunId) -> ta_store::RunProjection {
    execution
        .store
        .lock()
        .expect("store lock")
        .run(run_id)
        .expect("run lookup")
        .expect("run should exist")
}

fn run_count(execution: &RunExecutionService<InMemoryStore>) -> usize {
    execution
        .store
        .lock()
        .expect("store lock")
        .runs()
        .expect("runs should list")
        .len()
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

fn turn_id(value: &str) -> AgentStreamTurnId {
    AgentStreamTurnId::new(value).expect("turn id")
}
