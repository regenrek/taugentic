use std::collections::{BTreeMap, BTreeSet};

use ta_store::{EventRecord, PersistenceStore, RunProjection, native_run_parent_id};

use crate::{
    AgentStreamFrame, ApprovalEvent, BudgetEvent, ContextReceiptEvent, DaemonEvent,
    GetRunTimelineQuery, OutputContractKind, RUN_TIMELINE_EVENT_DEFAULT_LIMIT,
    RUN_TIMELINE_EVENT_MAX_LIMIT, RunHarnessKind, RunId, RunSource, RunTimeline, RunTimelineEvent,
    RunTimelineEventKind, RunTimelineEventPayload, RunTimelineRun,
};

use super::{AppService, AppServiceError};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn run_timeline(
        &self,
        session_id: &crate::SessionId,
        request: &GetRunTimelineQuery,
    ) -> Result<RunTimeline, AppServiceError> {
        if request.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                request.root_run_id.as_str().to_string(),
            ));
        }
        let limit = request.limit.unwrap_or(RUN_TIMELINE_EVENT_DEFAULT_LIMIT);
        if limit == 0 || limit > RUN_TIMELINE_EVENT_MAX_LIMIT {
            return Err(AppServiceError::InvalidRunTimelineLimit {
                max: RUN_TIMELINE_EVENT_MAX_LIMIT,
            });
        }

        let store = self.store.lock().expect("app store should not be poisoned");
        let Some(root) = store.run(&request.root_run_id)? else {
            return Err(AppServiceError::RunNotFound(
                request.root_run_id.as_str().to_string(),
            ));
        };
        if root.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                root.id.as_str().to_string(),
            ));
        }
        if root.harness != RunHarnessKind::Native {
            return Err(AppServiceError::RunNotNativeHarness(
                root.id.as_str().to_string(),
            ));
        }

        let session_runs = store
            .runs()?
            .into_iter()
            .filter(|run| run.session_id == *session_id && run.harness == RunHarnessKind::Native)
            .collect::<Vec<_>>();
        let descendants = collect_descendants(&request.root_run_id, &session_runs);
        let events = collect_timeline_events(
            store.events_for_session(session_id)?,
            &descendants,
            request.after_seq,
            limit as usize,
        );

        Ok(RunTimeline {
            session_id: session_id.clone(),
            root_run_id: request.root_run_id.clone(),
            runs: project_timeline_runs(session_runs, &descendants),
            latest_event_seq: events.latest_event_seq,
            events: events.items,
        })
    }
}

struct TimelineEventPage {
    items: Vec<RunTimelineEvent>,
    latest_event_seq: Option<u64>,
}

fn collect_descendants(root_run_id: &RunId, runs: &[RunProjection]) -> BTreeMap<RunId, u32> {
    let mut children_by_parent: BTreeMap<RunId, Vec<RunId>> = BTreeMap::new();
    for run in runs {
        if let Some(parent_run_id) = native_run_parent_id(run) {
            children_by_parent
                .entry(parent_run_id)
                .or_default()
                .push(run.id.clone());
        }
    }

    let mut depths = BTreeMap::new();
    let mut pending = vec![(root_run_id.clone(), 0)];
    while let Some((run_id, depth)) = pending.pop() {
        if depths.insert(run_id.clone(), depth).is_some() {
            continue;
        }
        if let Some(children) = children_by_parent.get(&run_id) {
            for child_id in children.iter().rev() {
                pending.push((child_id.clone(), depth + 1));
            }
        }
    }
    depths
}

fn project_timeline_runs(
    runs: Vec<RunProjection>,
    depths_by_run_id: &BTreeMap<RunId, u32>,
) -> Vec<RunTimelineRun> {
    let mut timeline_runs = runs
        .into_iter()
        .filter_map(|run| {
            let depth = *depths_by_run_id.get(&run.id)?;
            Some(RunTimelineRun {
                run_id: run.id.clone(),
                parent_run_id: native_run_parent_id(&run),
                depth,
                status: run.status,
                recipe_id: run_recipe_id(&run),
                output_contract: run_output_contract(&run),
                started_at_ms: run.started_at_ms,
                ended_at_ms: run.ended_at_ms,
                workspace_info: run.workspace_info,
                claimed_files: run.claimed_files,
            })
        })
        .collect::<Vec<_>>();
    timeline_runs.sort_by(|left, right| {
        left.started_at_ms
            .unwrap_or(0)
            .cmp(&right.started_at_ms.unwrap_or(0))
            .then_with(|| left.run_id.as_str().cmp(right.run_id.as_str()))
    });
    timeline_runs
}

fn collect_timeline_events(
    records: Vec<EventRecord>,
    depths_by_run_id: &BTreeMap<RunId, u32>,
    after_seq: Option<u64>,
    limit: usize,
) -> TimelineEventPage {
    let lineage_ids = depths_by_run_id.keys().cloned().collect::<BTreeSet<_>>();
    let mut latest_event_seq = None;
    let mut items = Vec::with_capacity(limit.min(1024));

    for record in records {
        let Some(event) = timeline_event_from_record(record, &lineage_ids) else {
            continue;
        };
        latest_event_seq = Some(event.seq);
        if after_seq.is_some_and(|after_seq| event.seq <= after_seq) || items.len() >= limit {
            continue;
        }
        items.push(event);
    }

    TimelineEventPage {
        items,
        latest_event_seq,
    }
}

fn timeline_event_from_record(
    record: EventRecord,
    lineage_ids: &BTreeSet<RunId>,
) -> Option<RunTimelineEvent> {
    let (run_id, kind, status, label, payload) = match record.payload {
        DaemonEvent::Run(ta_protocol::wire::RunEvent::Status(event))
            if lineage_ids.contains(event.run_id()) =>
        {
            (
                event.run_id().clone(),
                RunTimelineEventKind::RunStatus,
                Some(event.status()),
                event.reason().map_or_else(
                    || "run status changed".to_string(),
                    |reason| reason.as_str().to_string(),
                ),
                RunTimelineEventPayload::Run {
                    detail: event.reason().map_or_else(
                        || "run status changed".to_string(),
                        |reason| reason.as_str().to_string(),
                    ),
                    auth_profile_exhaustion: event.auth_profile_exhaustion(),
                },
            )
        }
        DaemonEvent::Approval(ApprovalEvent::Requested { request })
            if lineage_ids.contains(&request.run_id) =>
        {
            (
                request.run_id,
                RunTimelineEventKind::ApprovalRequested,
                None,
                "approval requested".to_string(),
                RunTimelineEventPayload::ApprovalRequested {
                    approval_id: request.id,
                    scope: request.scope,
                },
            )
        }
        DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
            if lineage_ids.contains(&resolution.run_id) =>
        {
            (
                resolution.run_id,
                RunTimelineEventKind::ApprovalResolved,
                None,
                format!("approval {:?}", resolution.decision).to_lowercase(),
                RunTimelineEventPayload::ApprovalResolved {
                    approval_id: resolution.approval_id,
                    decision: resolution.decision,
                },
            )
        }
        DaemonEvent::Conflict(crate::ConflictEvent::Warning { run_id, warning })
            if conflict_touches_lineage(&run_id, &warning, lineage_ids) =>
        {
            (
                run_id,
                RunTimelineEventKind::ClaimConflict,
                None,
                "claim conflict warning".to_string(),
                RunTimelineEventPayload::Conflict { warning },
            )
        }
        DaemonEvent::Budget(BudgetEvent::Exceeded { event })
            if lineage_ids.contains(&event.run_id)
                || event
                    .parent_run_id
                    .as_ref()
                    .is_some_and(|run_id| lineage_ids.contains(run_id)) =>
        {
            (
                event.run_id,
                RunTimelineEventKind::BudgetExceeded,
                Some(crate::RunStatus::BudgetExceeded),
                format!("budget exceeded: {:?}", event.breach.metric).to_lowercase(),
                RunTimelineEventPayload::BudgetExceeded {
                    scope: event.breach.scope,
                    metric: event.breach.metric,
                    limit: event.breach.limit,
                    actual: event.breach.actual,
                },
            )
        }
        DaemonEvent::AgentStream(event) if lineage_ids.contains(&event.run_id) => {
            agent_stream_timeline_event(event)?
        }
        DaemonEvent::TokenUsageRecorded(event) if lineage_ids.contains(&event.run_id) => (
            event.run_id.clone(),
            RunTimelineEventKind::TokenUsage,
            None,
            format!(
                "token usage recorded: {} prompt / {} completion",
                event.prompt_tokens, event.completion_tokens
            ),
            RunTimelineEventPayload::TokenUsage { usage: event },
        ),
        DaemonEvent::Artifact(event) if lineage_ids.contains(&event.artifact.run_id) => (
            event.artifact.run_id,
            RunTimelineEventKind::Artifact,
            None,
            format!("artifact {:?}", event.artifact.kind).to_lowercase(),
            RunTimelineEventPayload::Artifact {
                artifact_id: event.artifact.id.as_str().to_string(),
                artifact_kind: event.artifact.kind,
            },
        ),
        DaemonEvent::ContextReceipt(event) => {
            let receipt = match event {
                ContextReceiptEvent::Created { receipt }
                | ContextReceiptEvent::Promoted { receipt }
                | ContextReceiptEvent::Quarantined { receipt }
                    if lineage_ids.contains(&receipt.run_id) =>
                {
                    receipt
                }
                _ => return None,
            };
            (
                receipt.run_id,
                RunTimelineEventKind::Receipt,
                None,
                format!("receipt {:?}", receipt.state).to_lowercase(),
                RunTimelineEventPayload::Receipt {
                    receipt_id: receipt.id,
                    receipt_kind: receipt.kind,
                    receipt_state: receipt.state,
                },
            )
        }
        DaemonEvent::Session(_) | DaemonEvent::RunReconciledOnStartup(_) => return None,
        _ => return None,
    };

    Some(RunTimelineEvent {
        seq: record.sequence,
        occurred_at_ms: record.occurred_at_ms,
        run_id,
        kind,
        status,
        label,
        payload,
    })
}

fn agent_stream_timeline_event(
    event: crate::AgentStreamEvent,
) -> Option<(
    RunId,
    RunTimelineEventKind,
    Option<crate::RunStatus>,
    String,
    RunTimelineEventPayload,
)> {
    match event.emission.frame {
        AgentStreamFrame::TokenUsageUpdated { .. } => Some(agent_stream_event(
            event.run_id,
            "token usage updated",
            "tokenUsageUpdated",
        )),
        AgentStreamFrame::ToolCallStarted { tool_name, .. } => Some((
            event.run_id,
            RunTimelineEventKind::ToolCall,
            None,
            format!("tool started: {tool_name}"),
            RunTimelineEventPayload::ToolCall {
                tool_name: Some(tool_name),
                outcome: None,
            },
        )),
        AgentStreamFrame::ToolCallCompleted { outcome } => Some((
            event.run_id,
            RunTimelineEventKind::ToolCall,
            None,
            format!("tool {:?}", outcome).to_lowercase(),
            RunTimelineEventPayload::ToolCall {
                tool_name: None,
                outcome: Some(outcome),
            },
        )),
        AgentStreamFrame::AssistantTurnStarted => Some(agent_stream_event(
            event.run_id,
            "assistant turn started",
            "assistantTurnStarted",
        )),
        AgentStreamFrame::AssistantTurnCompleted => Some(agent_stream_event(
            event.run_id,
            "assistant turn completed",
            "assistantTurnCompleted",
        )),
        AgentStreamFrame::PendingStateChanged { state } => Some(agent_stream_event(
            event.run_id,
            &format!("pending {:?}", state).to_lowercase(),
            "pendingStateChanged",
        )),
        AgentStreamFrame::AssistantMessageDelta { .. }
        | AgentStreamFrame::ToolCallProgressed { .. } => None,
    }
}

fn agent_stream_event(
    run_id: RunId,
    label: &str,
    frame_kind: &str,
) -> (
    RunId,
    RunTimelineEventKind,
    Option<crate::RunStatus>,
    String,
    RunTimelineEventPayload,
) {
    (
        run_id,
        RunTimelineEventKind::AgentStream,
        None,
        label.to_string(),
        RunTimelineEventPayload::AgentStream {
            frame_kind: frame_kind.to_string(),
        },
    )
}

fn conflict_touches_lineage(
    event_run_id: &RunId,
    warning: &ta_protocol::wire::ConflictWarning,
    lineage_ids: &BTreeSet<RunId>,
) -> bool {
    lineage_ids.contains(event_run_id)
        || lineage_ids.contains(&warning.requesting_capsule)
        || warning
            .conflicts
            .iter()
            .any(|conflict| lineage_ids.contains(&conflict.holding_capsule))
}

fn run_recipe_id(run: &RunProjection) -> Option<String> {
    match &run.source {
        RunSource::NativeSubagent { recipe_id, .. }
        | RunSource::FreshSpawn { recipe_id, .. }
        | RunSource::User { recipe_id, .. } => recipe_id.clone(),
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::RouteSwitchedContinuation { .. } => None,
    }
}

fn run_output_contract(run: &RunProjection) -> Option<OutputContractKind> {
    match &run.source {
        RunSource::NativeSubagent {
            output_contract, ..
        }
        | RunSource::FreshSpawn {
            output_contract, ..
        }
        | RunSource::User {
            output_contract, ..
        } => *output_contract,
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::RouteSwitchedContinuation { .. } => None,
    }
}
