use ta_policy::{BudgetLimits, BudgetPolicy};
use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, ApprovalDecision, ApprovalId,
    ApprovalRequest, ApprovalScope, ApprovalTarget, CapsuleResult, DaemonApprovalDecideParams,
    GetRunTimelineQuery, PatchResult, ReviewResult, ReviewVerdict, RunEvent, RunSource, RunStatus,
    RunTimelineEventKind, StreamEmission, WorkspaceMode, WorktreeCleanupPolicy,
};
use ta_provider_llm::client::LlmTokenUsage;
use ta_store::{CommitRepository, CommitRunTransition};
use taugentic_agent::{ExecutionSink, NativeChildRunRequest};

use super::test_support::{
    app_and_execution_with_runtime, approval_actor, attach_noop_handle, attach_recording_handle,
    open_session, provider_sink, select_runtime_profile, set_default_test_workspace_root,
};
use super::*;

const OVERLAP_FILE: &str = "apps/desktop/package.json";

#[test]
fn hierarchical_delegation_smoke_projects_replay_timeline() {
    let repo = clean_git_repo_fixture();
    let runtime = runtime_for_clean_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Hierarchical replay smoke");
    select_runtime_profile(&app, "runtime-openai-safe");
    let parent = execution
        .seed_running_run_for_tests(
            session.id.clone(),
            "Parent hierarchical orchestrator".to_string(),
        )
        .expect("parent should seed")
        .run;

    let scout = start_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-scout",
        "plan-agent",
        None,
    );
    let debug = start_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-debug",
        "debug-agent",
        None,
    );
    let patch = start_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-patch",
        "patch-agent",
        Some(OVERLAP_FILE),
    );
    let review = start_child(
        &execution,
        &session.id,
        &parent.id,
        "turn-review",
        "review-agent",
        Some(OVERLAP_FILE),
    );
    assert_eq!(scout.status, RunStatus::WaitingForApproval);
    assert_eq!(debug.status, RunStatus::WaitingForApproval);
    assert_eq!(patch.status, RunStatus::WaitingForApproval);
    assert_eq!(review.status, RunStatus::WaitingForApproval);

    mark_child_running(&execution, &session.id, &debug.run_id);
    mark_child_running(&execution, &session.id, &patch.run_id);
    mark_child_running(&execution, &session.id, &review.run_id);

    attach_noop_handle(&execution, &debug.run_id);
    execution
        .runtime
        .set_budget_policy_for_tests(BudgetPolicy::new(
            BudgetLimits {
                max_tokens: Some(0),
                ..BudgetLimits::unbounded()
            },
            BudgetLimits::unbounded(),
        ));
    let debug_sink = provider_sink(&execution, &session.id, &debug.run_id);
    debug_sink
        .record_token_usage(LlmTokenUsage {
            prompt_tokens: 1,
            completion_tokens: 0,
            cached_tokens: None,
            reasoning_tokens: None,
            model: "gpt-test".to_string(),
            provider: "openai".to_string(),
        })
        .expect("real token usage should persist");
    debug_sink
        .push_stream(StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::TokenUsageUpdated {
                total_tokens: Some(1),
                model_context_window: Some(100),
            },
        })
        .expect_err("debug child should fail through budget enforcement");

    let recorded_approvals = attach_recording_handle(&execution, &patch.run_id);
    let live_approval = live_tool_approval(&patch.run_id);
    provider_sink(&execution, &session.id, &patch.run_id)
        .request_approval(live_approval.clone())
        .expect("live approval request should persist");
    execution
        .decide_approval(
            session.id.clone(),
            approval_actor(),
            DaemonApprovalDecideParams {
                approval_id: live_approval.id.clone(),
                decision: ApprovalDecision::Approved,
                commentary: Some("allow patch smoke".to_string()),
            },
        )
        .expect("approval decision should resolve live waiter");
    assert_eq!(
        recorded_approvals
            .lock()
            .expect("approval record lock")
            .len(),
        1
    );

    execution
        .complete_run_with_result(
            session.id.clone(),
            &patch.run_id,
            "patch complete".to_string(),
            Some(patch_result()),
        )
        .expect("patch should complete with structured result");
    execution
        .complete_run_with_result(
            session.id.clone(),
            &review.run_id,
            "review complete".to_string(),
            Some(review_result()),
        )
        .expect("review should complete with structured result");

    let timeline = app
        .run_timeline(
            &session.id,
            &GetRunTimelineQuery {
                session_id: session.id.clone(),
                root_run_id: parent.id.clone(),
                after_seq: None,
                limit: Some(200),
            },
        )
        .expect("timeline should project from event log");
    let run_ids = timeline
        .runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        &parent.id,
        &scout.run_id,
        &debug.run_id,
        &patch.run_id,
        &review.run_id,
    ] {
        assert!(
            run_ids.contains(expected),
            "missing timeline run {expected:?}"
        );
    }
    assert!(
        timeline.runs.iter().all(|run| {
            run.run_id == parent.id || run.parent_run_id.as_ref() == Some(&parent.id)
        })
    );
    assert_timeline_kind(&timeline.events, RunTimelineEventKind::ApprovalRequested);
    assert_timeline_kind(&timeline.events, RunTimelineEventKind::ApprovalResolved);
    assert_timeline_kind(&timeline.events, RunTimelineEventKind::ClaimConflict);
    assert_timeline_kind(&timeline.events, RunTimelineEventKind::BudgetExceeded);
    assert_timeline_kind(&timeline.events, RunTimelineEventKind::TokenUsage);
    assert!(timeline.events.iter().any(|event| {
        event.run_id == patch.run_id && event.status == Some(RunStatus::Completed)
    }));
    assert!(timeline.events.iter().any(|event| {
        event.run_id == review.run_id && event.status == Some(RunStatus::Completed)
    }));
}

fn start_child(
    execution: &RunExecutionService,
    session_id: &crate::SessionId,
    parent_run_id: &RunId,
    turn_id: &str,
    recipe_id: &str,
    planned_file: Option<&str>,
) -> taugentic_agent::NativeChildRunResult {
    let mut request = NativeChildRunRequest::new(
        parent_run_id.clone(),
        AgentStreamTurnId::new(turn_id).expect("turn id"),
        format!("Execute {recipe_id} smoke step"),
        None,
        None,
        Some(recipe_id.to_string()),
    )
    .expect("child request")
    .with_workspace_scope(WorkspaceMode::WorktreeWrite)
    .with_cleanup_policy(WorktreeCleanupPolicy::Keep);
    if let Some(file) = planned_file {
        request = request.with_planned_write_files(vec![file.to_string()]);
    }
    execution
        .start_native_child_run(session_id.clone(), request)
        .expect("child run should start through orchestrator")
}

fn mark_child_running(
    execution: &RunExecutionService,
    session_id: &crate::SessionId,
    run_id: &RunId,
) {
    let existing = execution
        .load_run_projection(run_id)
        .expect("child run should load");
    let recipe_id = recipe_id_for_run(&existing);
    {
        let mut store = execution.store.lock().expect("store lock");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: ta_store::RunProjection {
                    status: RunStatus::Running,
                    ..existing
                },
                events: vec![DaemonEvent::Run(RunEvent {
                    run_id: run_id.clone(),
                    status: RunStatus::Running,
                    detail: "Smoke child running through production preflight".to_string(),
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

fn live_tool_approval(run_id: &RunId) -> ApprovalRequest {
    let requested_at_ms = current_time_ms();
    let ttl = ta_policy::ApprovalTtlPolicy::default();
    ApprovalRequest::new(
        ApprovalId::new("approval-patch-shell").expect("approval id"),
        run_id.clone(),
        ApprovalScope::ProcessExec,
        requested_at_ms,
        ttl.expires_at_ms(requested_at_ms),
        ApprovalTarget::ToolCall {
            tool_name: "shell".to_string(),
        },
        "patch smoke shell requires approval",
    )
    .expect("approval request")
    .with_tool_call_id(AgentStreamItemId::new("tool-patch-shell").expect("tool id"))
}

fn patch_result() -> CapsuleResult {
    CapsuleResult::Patch(PatchResult {
        patch_receipt_ids: vec!["receipt-smoke-patch".to_string()],
        touched_files: vec![OVERLAP_FILE.to_string()],
        tests_run_receipt_ids: vec!["receipt-smoke-tests".to_string()],
        passing: true,
        blockers: Vec::new(),
    })
}

fn review_result() -> CapsuleResult {
    CapsuleResult::Review(ReviewResult {
        verdict: ReviewVerdict::Approve,
        findings: Vec::new(),
        risks: Vec::new(),
        touched_files: vec![OVERLAP_FILE.to_string()],
    })
}

fn recipe_id_for_run(run: &ta_store::RunProjection) -> Option<String> {
    match &run.source {
        RunSource::NativeSubagent { recipe_id, .. } | RunSource::User { recipe_id, .. } => {
            recipe_id.clone()
        }
        RunSource::Forked { .. } => None,
    }
}

fn assert_timeline_kind(
    events: &[ta_protocol::wire::RunTimelineEvent],
    kind: RunTimelineEventKind,
) {
    assert!(
        events.iter().any(|event| event.kind == kind),
        "timeline missing {kind:?}"
    );
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
