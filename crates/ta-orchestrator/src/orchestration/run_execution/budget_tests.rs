use std::time::Duration;

use ta_policy::{BudgetLimits, BudgetPolicy};
use ta_protocol::wire::{
    AgentStreamFrame, BudgetEvent, BudgetMetric, BudgetScope, DaemonEvent, RunStatus,
    StreamEmission,
};
use ta_store::EventLogRepository;
use taugentic_agent::{ExecutionSink, NativeChildRunRequest};

use super::test_support::*;
use super::*;

#[test]
fn budget_allows_streaming_when_usage_stays_within_limits() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, "Budget baseline");
    let running = ensure_running_run(&app, &execution, &session.id, "Stay in budget");
    set_budget_policy(
        &execution,
        BudgetLimits {
            max_tokens: Some(10),
            max_tool_calls: Some(1),
            ..BudgetLimits::unbounded()
        },
        BudgetLimits::unbounded(),
    );
    let sink = provider_sink(&execution, &session.id, &running.id);

    sink.push_stream(emission(AgentStreamFrame::ToolCallStarted {
        tool_name: "shell".to_string(),
        input: "{}".to_string(),
    }))
    .expect("tool call at limit should remain running");
    sink.push_stream(emission(AgentStreamFrame::TokenUsageUpdated {
        total_tokens: Some(10),
        model_context_window: Some(100),
    }))
    .expect("token usage at limit should remain running");

    assert_eq!(load_run_status(&execution, &running.id), RunStatus::Running);
    assert!(
        !events_for(&execution, &session.id)
            .iter()
            .any(|record| { matches!(record.payload, DaemonEvent::Budget(_)) })
    );
}

#[test]
fn token_budget_breach_fails_running_run_mid_stream() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, "Token budget");
    let running = ensure_running_run(&app, &execution, &session.id, "Spend tokens");
    attach_noop_handle(&execution, &running.id);
    set_budget_policy(
        &execution,
        BudgetLimits {
            max_tokens: Some(10),
            ..BudgetLimits::unbounded()
        },
        BudgetLimits::unbounded(),
    );

    let error = provider_sink(&execution, &session.id, &running.id)
        .push_stream(emission(AgentStreamFrame::TokenUsageUpdated {
            total_tokens: Some(11),
            model_context_window: Some(100),
        }))
        .expect_err("token budget breach should fail fast");

    assert!(error.to_string().contains("budget exceeded"));
    assert_budget_exceeded(
        &execution,
        &session.id,
        &running.id,
        BudgetScope::Run,
        BudgetMetric::Tokens,
    );
}

#[test]
fn tool_call_budget_breach_fails_running_run_mid_stream() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, "Tool budget");
    let running = ensure_running_run(&app, &execution, &session.id, "Use tools");
    attach_noop_handle(&execution, &running.id);
    set_budget_policy(
        &execution,
        BudgetLimits {
            max_tool_calls: Some(0),
            ..BudgetLimits::unbounded()
        },
        BudgetLimits::unbounded(),
    );

    provider_sink(&execution, &session.id, &running.id)
        .push_stream(emission(AgentStreamFrame::ToolCallStarted {
            tool_name: "shell".to_string(),
            input: "{}".to_string(),
        }))
        .expect_err("tool-call budget breach should fail fast");

    assert_budget_exceeded(
        &execution,
        &session.id,
        &running.id,
        BudgetScope::Run,
        BudgetMetric::ToolCalls,
    );
}

#[test]
fn wall_clock_budget_breach_fails_running_run_mid_stream() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, "Wall budget");
    let running = ensure_running_run(&app, &execution, &session.id, "Run too long");
    attach_noop_handle(&execution, &running.id);
    set_budget_policy(
        &execution,
        BudgetLimits {
            max_wall_clock_ms: Some(0),
            ..BudgetLimits::unbounded()
        },
        BudgetLimits::unbounded(),
    );
    std::thread::sleep(Duration::from_millis(2));

    provider_sink(&execution, &session.id, &running.id)
        .push_stream(emission(AgentStreamFrame::AssistantTurnStarted))
        .expect_err("wall-clock budget breach should fail fast");

    assert_budget_exceeded(
        &execution,
        &session.id,
        &running.id,
        BudgetScope::Run,
        BudgetMetric::WallClockMs,
    );
}

#[test]
fn parent_aggregate_budget_prevents_child_dispatch() {
    let repo = init_dispatch_repo();
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Inherited budget");
    let parent = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Parent budget owner",
        "runtime-openai-allow",
    );
    provider_sink(&execution, &session.id, &parent.id)
        .push_stream(emission(AgentStreamFrame::TokenUsageUpdated {
            total_tokens: Some(6),
            model_context_window: Some(100),
        }))
        .expect("parent usage should seed before budget is enforced");
    set_budget_policy(
        &execution,
        BudgetLimits::unbounded(),
        BudgetLimits {
            max_tokens: Some(5),
            ..BudgetLimits::unbounded()
        },
    );

    let child = execution
        .start_native_child_run(
            session.id.clone(),
            NativeChildRunRequest::new(
                parent.id.clone(),
                ta_protocol::wire::AgentStreamTurnId::new("turn-budget-parent").expect("turn id"),
                "Child inherits parent budget",
                None,
                None,
                None,
            )
            .expect("child request")
            .with_workspace_scope(crate::WorkspaceMode::WorktreeWrite),
        )
        .expect("child dispatch should fail through a typed terminal status");

    assert_eq!(child.status, RunStatus::BudgetExceeded);
    assert_budget_exceeded(
        &execution,
        &session.id,
        &child.run_id,
        BudgetScope::ParentAggregate,
        BudgetMetric::Tokens,
    );
}

fn set_budget_policy(
    execution: &RunExecutionService,
    per_run: BudgetLimits,
    parent_aggregate: BudgetLimits,
) {
    execution
        .runtime
        .set_budget_policy_for_tests(BudgetPolicy::new(per_run, parent_aggregate));
}

fn emission(frame: AgentStreamFrame) -> StreamEmission {
    StreamEmission {
        turn_id: None,
        item_id: None,
        fragment_sequence: None,
        frame,
    }
}

fn load_run_status(execution: &RunExecutionService, run_id: &RunId) -> RunStatus {
    execution
        .load_run_projection(run_id)
        .expect("run should load")
        .status
}

fn events_for(execution: &RunExecutionService, session_id: &crate::SessionId) -> Vec<EventRecord> {
    execution
        .store
        .lock()
        .expect("store should not poison")
        .events_for_session(session_id)
        .expect("events should load")
}

fn assert_budget_exceeded(
    execution: &RunExecutionService,
    session_id: &crate::SessionId,
    run_id: &RunId,
    scope: BudgetScope,
    metric: BudgetMetric,
) {
    assert_eq!(
        load_run_status(execution, run_id),
        RunStatus::BudgetExceeded
    );
    assert!(
        !execution.is_live_run_running(run_id, session_id),
        "budget terminal status should release the live run"
    );
    assert!(events_for(execution, session_id).iter().any(|record| {
        matches!(
            &record.payload,
            DaemonEvent::Budget(BudgetEvent::Exceeded { event })
                if event.run_id == *run_id
                    && event.breach.scope == scope
                    && event.breach.metric == metric
        )
    }));
}
