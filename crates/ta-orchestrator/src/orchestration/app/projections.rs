use std::collections::HashMap;

use ta_store::{PersistenceStore, SessionEventPageQuery, StoreError, native_run_parent_id};

use crate::{
    AuthProfileExhaustion, ContextReceipt, DaemonEvent, MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT,
    OutputContractKind, RunDetail, RunListEntry, RunSource, RunStatus, RunSummary,
    SessionOverviewLaneStatus, SessionSummary,
};

pub(super) fn project_latest_run_for_session(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    session_runs: Option<&HashMap<crate::RunId, RunSummary>>,
    recent_events: &[ta_store::EventRecord],
) -> Result<Option<RunSummary>, StoreError> {
    let Some(session_runs) = session_runs else {
        return Ok(None);
    };

    if let Some(latest_run) = project_latest_run_from_events(session_runs, recent_events) {
        return Ok(Some(latest_run));
    }

    let latest_run_event = store.session_event_page(&SessionEventPageQuery {
        session_id: session_id.clone(),
        before_sequence: None,
        limit: 1,
        kinds: vec![ta_protocol::wire::DaemonEventKind::Run],
    })?;
    Ok(project_latest_run_from_events(
        session_runs,
        &latest_run_event.records,
    ))
}

pub(super) fn session_overview_recent_activity_kinds() -> Vec<crate::DaemonEventKind> {
    vec![
        crate::DaemonEventKind::Session,
        crate::DaemonEventKind::Run,
        crate::DaemonEventKind::Approval,
        crate::DaemonEventKind::Artifact,
        crate::DaemonEventKind::Budget,
    ]
}

fn project_latest_run_from_events(
    session_runs: &HashMap<crate::RunId, RunSummary>,
    events: &[ta_store::EventRecord],
) -> Option<RunSummary> {
    events.iter().find_map(|record| match &record.payload {
        DaemonEvent::Run(event) => session_runs.get(event.run_id()).cloned(),
        DaemonEvent::Approval(_)
        | DaemonEvent::Artifact(_)
        | DaemonEvent::ContextReceipt(_)
        | DaemonEvent::Session(_)
        | DaemonEvent::AgentStream(_)
        | DaemonEvent::RunReconciledOnStartup(_)
        | DaemonEvent::TokenUsageRecorded(_)
        | DaemonEvent::Conflict(_)
        | DaemonEvent::Budget(_) => None,
    })
}

pub(super) fn index_run_summaries_by_session(
    runs: Vec<ta_store::RunProjection>,
) -> HashMap<crate::SessionId, HashMap<crate::RunId, RunSummary>> {
    let mut runs_by_session = HashMap::new();
    for run in runs {
        let run_id = run.id.clone();
        let run_summary = project_run_summary(&run);
        runs_by_session
            .entry(run.session_id)
            .or_insert_with(HashMap::new)
            .insert(run_id, run_summary);
    }
    runs_by_session
}

pub(super) fn clamp_session_overview_recent_activity_limit(limit: u32) -> usize {
    usize::try_from(limit.min(MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT))
        .expect("u32 recent activity limit should fit in usize")
}

pub(super) fn project_run_summary(run: &ta_store::RunProjection) -> RunSummary {
    RunSummary {
        id: run.id.clone(),
        runtime_profile_id: run.runtime_profile_id.clone(),
        objective: run.objective.clone(),
        status: run.status,
    }
}

pub(super) fn project_run_detail(
    run: &ta_store::RunProjection,
    quarantine_receipt: Option<ContextReceipt>,
    token_usage: Option<crate::TokenUsageTotals>,
    auth_profile_exhaustion: Option<AuthProfileExhaustion>,
) -> RunDetail {
    let parent_run_id = native_run_parent_id(run);
    let (output_contract, recipe_id) = native_run_contract_fields(run);
    RunDetail {
        summary: project_run_summary(run),
        result: run.result.clone(),
        contract_violation: run.contract_violation.clone(),
        quarantine_receipt,
        output_contract,
        recipe_id,
        parent_run_id,
        execution_context: run.execution_context.clone(),
        workspace_info: run.workspace_info.clone(),
        claimed_files: run.claimed_files.clone(),
        conflict_summary: run.conflict_summary.clone(),
        token_usage,
        auth_profile_exhaustion,
    }
}

pub(super) fn project_run_list_entry(run: ta_store::RunProjection) -> RunListEntry {
    let relationship = native_run_relationship(&run);
    let (output_contract, recipe_id) = native_run_contract_fields(&run);
    let objective_preview = Some(trim_run_objective_preview(&run.objective));
    RunListEntry {
        id: run.id,
        relationship,
        output_contract,
        recipe_id,
        harness: run.harness,
        status: run.status,
        started_at_ms: run.started_at_ms,
        ended_at_ms: run.ended_at_ms,
        last_event_seq: run.last_event_seq,
        objective_preview,
        workspace_info: run.workspace_info,
        claimed_files: run.claimed_files,
        conflict_summary: run.conflict_summary,
    }
}

fn native_run_relationship(run: &ta_store::RunProjection) -> crate::NativeRunRelationship {
    match &run.source {
        RunSource::ScheduledWork { .. } | RunSource::User { .. } => {
            crate::NativeRunRelationship::Root
        }
        RunSource::NativeSubagent { parent_run_id, .. } => {
            crate::NativeRunRelationship::NativeSubagent {
                parent_run_id: parent_run_id.clone(),
            }
        }
        RunSource::FreshSpawn { parent_run_id, .. } => crate::NativeRunRelationship::FreshSpawn {
            parent_run_id: parent_run_id.clone(),
        },
        RunSource::Forked {
            parent_run_id,
            parent_event_seq,
            ..
        } => crate::NativeRunRelationship::Fork {
            parent_run_id: parent_run_id.clone(),
            parent_event_seq: *parent_event_seq,
        },
        RunSource::AccountSwitchedContinuation {
            parent_run_id,
            parent_event_seq,
            ..
        } => crate::NativeRunRelationship::AccountSwitchedContinuation {
            parent_run_id: parent_run_id.clone(),
            parent_event_seq: *parent_event_seq,
        },
    }
}

fn native_run_contract_fields(
    run: &ta_store::RunProjection,
) -> (Option<OutputContractKind>, Option<String>) {
    match &run.source {
        RunSource::NativeSubagent {
            output_contract,
            recipe_id,
            ..
        }
        | RunSource::FreshSpawn {
            output_contract,
            recipe_id,
            ..
        }
        | RunSource::User {
            output_contract,
            recipe_id,
            ..
        } => (*output_contract, recipe_id.clone()),
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::AccountSwitchedContinuation { .. } => (None, None),
    }
}

fn trim_run_objective_preview(objective: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;
    let trimmed = objective.trim();
    if trimmed.chars().count() <= MAX_PREVIEW_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_PREVIEW_CHARS).collect()
}

pub(super) fn project_session_overview_lane_status(
    latest_run: Option<&RunSummary>,
    pending_approval_count: u32,
) -> SessionOverviewLaneStatus {
    if pending_approval_count > 0
        || latest_run.is_some_and(|run| run.status == RunStatus::WaitingForApproval)
    {
        return SessionOverviewLaneStatus::WaitingForApproval;
    }

    match latest_run.map(|run| run.status) {
        Some(RunStatus::Queued | RunStatus::Running) => SessionOverviewLaneStatus::Active,
        Some(RunStatus::Failed | RunStatus::BudgetExceeded) => SessionOverviewLaneStatus::Failed,
        Some(RunStatus::Completed) => SessionOverviewLaneStatus::Completed,
        Some(RunStatus::Cancelled) => SessionOverviewLaneStatus::Cancelled,
        Some(RunStatus::WaitingForApproval) | None => SessionOverviewLaneStatus::Idle,
    }
}

pub(super) fn summarize_event_preview(event: &DaemonEvent) -> String {
    match event {
        DaemonEvent::Session(event) => {
            format!("Session {}", summarize_session_status(event.status))
        }
        DaemonEvent::Run(ta_protocol::wire::RunEvent::Status(event)) => format!(
            "Run {}: {}",
            summarize_run_status(event.status()),
            event.reason().map_or("", |reason| reason.as_str()).trim()
        ),
        DaemonEvent::Approval(crate::ApprovalEvent::Requested { request }) => {
            format!("Approval requested: {}", request.reason.trim())
        }
        DaemonEvent::Approval(crate::ApprovalEvent::Resolved { resolution }) => format!(
            "Approval {}",
            match resolution.decision {
                crate::ApprovalDecision::Approved => "approved",
                crate::ApprovalDecision::Rejected => "rejected",
            }
        ),
        DaemonEvent::Artifact(event) => format!(
            "Artifact {} for run {}",
            summarize_artifact_kind(event.artifact.kind),
            event.artifact.run_id.as_str()
        ),
        DaemonEvent::ContextReceipt(event) => match event {
            crate::ContextReceiptEvent::Created { receipt } => {
                format!("Receipt returned: {}", receipt.id)
            }
            crate::ContextReceiptEvent::Promoted { receipt } => {
                format!("Receipt promoted: {}", receipt.id)
            }
            crate::ContextReceiptEvent::Quarantined { receipt } => {
                format!("Receipt quarantined: {}", receipt.id)
            }
        },
        DaemonEvent::AgentStream(event) => summarize_agent_stream_preview(event),
        DaemonEvent::RunReconciledOnStartup(event) => {
            format!(
                "Run reconciled after daemon restart: {}",
                event.run_id.as_str()
            )
        }
        DaemonEvent::TokenUsageRecorded(event) => format!(
            "Token usage recorded for {}: {} prompt / {} completion",
            event.run_id.as_str(),
            event.prompt_tokens,
            event.completion_tokens
        ),
        DaemonEvent::Conflict(event) => summarize_conflict_preview(event),
        DaemonEvent::Budget(event) => summarize_budget_preview(event),
    }
}

fn summarize_conflict_preview(event: &crate::ConflictEvent) -> String {
    match event {
        crate::ConflictEvent::Warning { run_id, warning } => format!(
            "Conflict warning for {}: {} overlapping file claim(s)",
            run_id.as_str(),
            warning.conflicts.len()
        ),
    }
}

fn summarize_budget_preview(event: &crate::BudgetEvent) -> String {
    match event {
        crate::BudgetEvent::Exceeded { event } => {
            format!("Budget exceeded for {}", event.run_id.as_str())
        }
    }
}

fn summarize_agent_stream_preview(event: &crate::AgentStreamEvent) -> String {
    match &event.emission.frame {
        crate::AgentStreamFrame::AssistantTurnStarted => "Assistant turn started".to_string(),
        crate::AgentStreamFrame::AssistantMessageDelta { .. } => {
            "Assistant message delta".to_string()
        }
        crate::AgentStreamFrame::AssistantTurnCompleted => "Assistant turn completed".to_string(),
        crate::AgentStreamFrame::ToolCallStarted { tool_name, .. } => {
            format!("Tool call started: {}", tool_name.trim())
        }
        crate::AgentStreamFrame::ToolCallProgressed { .. } => "Tool call progressed".to_string(),
        crate::AgentStreamFrame::ToolCallCompleted { outcome } => format!(
            "Tool call {}",
            match outcome {
                crate::AgentToolCallOutcome::Completed => "completed",
                crate::AgentToolCallOutcome::Failed => "failed",
                crate::AgentToolCallOutcome::Cancelled => "cancelled",
            }
        ),
        crate::AgentStreamFrame::PendingStateChanged { state } => format!(
            "Pending state: {}",
            match state {
                crate::RuntimeLanePendingState::Queued => "queued",
                crate::RuntimeLanePendingState::WaitingForApproval => "waiting for approval",
                crate::RuntimeLanePendingState::WaitingForInput => "waiting for input",
            }
        ),
        crate::AgentStreamFrame::TokenUsageUpdated {
            total_tokens,
            model_context_window,
        } => format!(
            "Token usage total={} context_window={}",
            total_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            model_context_window
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
    }
}

fn summarize_session_status(status: crate::SessionStatus) -> &'static str {
    match status {
        crate::SessionStatus::Idle => "idle",
        crate::SessionStatus::Running => "running",
        crate::SessionStatus::Paused => "paused",
        crate::SessionStatus::Failed => "failed",
        crate::SessionStatus::Completed => "completed",
    }
}

fn summarize_run_status(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::WaitingForApproval => "waiting for approval",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::BudgetExceeded => "budget exceeded",
        RunStatus::Cancelled => "cancelled",
    }
}

fn summarize_artifact_kind(kind: crate::ArtifactKind) -> &'static str {
    match kind {
        crate::ArtifactKind::Transcript => "transcript",
        crate::ArtifactKind::Patch => "patch",
        crate::ArtifactKind::FileSnapshot => "file snapshot",
        crate::ArtifactKind::CommandLog => "command log",
        crate::ArtifactKind::Image => "image",
    }
}

pub(super) fn project_session_summary(session: ta_store::SessionProjection) -> SessionSummary {
    SessionSummary {
        id: session.id,
        title: session.title,
        status: session.status,
        next_run_selection: session.next_run_selection,
    }
}
