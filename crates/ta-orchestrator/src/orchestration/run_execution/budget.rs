use std::collections::{BTreeMap, BTreeSet};

use ta_policy::{
    BudgetDecision, BudgetExceeded as PolicyBudgetExceeded, BudgetMetric as PolicyBudgetMetric,
    BudgetScope as PolicyBudgetScope, BudgetUsage,
};
use ta_protocol::wire::{
    AgentStreamFrame, ApprovalActor, ApprovalDecision, ApprovalResolution,
    ApprovalResolutionReason, BudgetBreach, BudgetEvent, BudgetExceededEvent,
    BudgetMetric as WireBudgetMetric, BudgetScope as WireBudgetScope, BudgetSnapshot,
};
use ta_store::{CommitRunTransition, EventRecord, SessionApprovalQuery};

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn enforce_budget_before_dispatch(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        self.enforce_budget(session_id, run_id, current_time_ms(), generation)
    }

    pub(super) fn enforce_budget_after_stream(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        self.enforce_budget(session_id, run_id, current_time_ms(), generation)
    }

    fn enforce_budget(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        now_ms: u64,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let policy = self.runtime.budget_policy();
        let projection = self.budget_projection(session_id, run_id, now_ms)?;
        let decision = match policy.decide_run(projection.run_usage) {
            BudgetDecision::WithinBudget => {
                policy.decide_parent_aggregate(projection.aggregate_usage)
            }
            exceeded => exceeded,
        };
        let BudgetDecision::Exceeded(exceeded) = decision else {
            return Ok(());
        };

        self.fail_run_for_budget(session_id, run_id, projection, exceeded, now_ms, generation)?;
        Err(RunExecutionError::BudgetExceeded(
            exceeded.redacted_reason().to_string(),
        ))
    }

    fn fail_run_for_budget(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        projection: BudgetProjection,
        exceeded: PolicyBudgetExceeded,
        now_ms: u64,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let commit =
            |store: &mut S| -> Result<(RunProjection, Vec<EventRecord>), RunExecutionError> {
                let Some(existing_run) = store.run(run_id)? else {
                    return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
                };
                if existing_run.session_id != *session_id {
                    return Err(RunExecutionError::RunSessionMismatch(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                if existing_run.status != RunStatus::Running {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        existing_run.id.as_str().to_string(),
                    ));
                }

                let mut events = Vec::new();
                events.push(DaemonEvent::Budget(BudgetEvent::Exceeded {
                    event: budget_exceeded_event(&projection, exceeded),
                }));
                events.extend(
                    store
                        .approvals_for_session(&SessionApprovalQuery {
                            session_id: session_id.clone(),
                            run_id: Some(existing_run.id.clone()),
                            approval_id: None,
                        })?
                        .into_iter()
                        .map(|approval| {
                            let mut resolution = ApprovalResolution::new(
                                approval.id,
                                approval.run_id,
                                ApprovalDecision::Rejected,
                                ApprovalResolutionReason::BudgetExceeded,
                                daemon_budget_actor(),
                                Some("budget_exceeded".to_string()),
                            );
                            if let Some(tool_call_id) = approval.tool_call_id {
                                resolution = resolution.with_tool_call_id(tool_call_id);
                            }
                            DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                        }),
                );

                let run = RunProjection {
                    status: RunStatus::BudgetExceeded,
                    ..existing_run
                };
                events.push(DaemonEvent::Run(
                    crate::RunEvent::terminal(
                        run.id.clone(),
                        RunStatus::BudgetExceeded,
                        crate::RunStatusReason::new(exceeded.redacted_reason())
                            .expect("budget reason"),
                        None,
                        recipe_id_for_run(&run),
                        None,
                    )
                    .expect("budget exceeded is terminal"),
                ));
                let committed = store.commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: run.clone(),
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events,
                    occurred_at_ms: now_ms,
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?;
                Ok((committed.run, committed.events))
            };
        let ((run, events), cancelled_handle) = self
            .runtime
            .with_terminal_live_generation_lease_and_take_handle(
                run_id,
                session_id,
                generation,
                || {
                    let mut store = self.store.lock().expect("app store should not be poisoned");
                    commit(&mut *store)
                },
            )?;
        if let Some(handle) = cancelled_handle {
            handle
                .cancel()
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        }

        let mut records = events;
        records.extend(self.advance_ready_queue(session_id, &run.id, RunStatus::BudgetExceeded)?);
        self.publish_records(&records);
        Ok(())
    }

    fn budget_projection(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        now_ms: u64,
    ) -> Result<BudgetProjection, RunExecutionError> {
        let (run, runs, events) = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let Some(run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if run.session_id != *session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    run.id.as_str().to_string(),
                ));
            }
            let runs = store
                .runs()?
                .into_iter()
                .filter(|candidate| candidate.session_id == *session_id)
                .collect::<Vec<_>>();
            let events = store.events_for_session(session_id)?;
            (run, runs, events)
        };

        let runs_by_id = runs
            .iter()
            .map(|run| (run.id.clone(), run.clone()))
            .collect::<BTreeMap<_, _>>();
        let root_run_id = root_run_id(&run, &runs_by_id);
        let aggregate_run_ids = aggregate_run_ids(&root_run_id, &runs_by_id);
        let usage_by_run = usage_by_run(&events);

        let run_usage = usage_for_run(&run, usage_by_run.get(&run.id), now_ms);
        let aggregate_usage = aggregate_usage(
            &root_run_id,
            &aggregate_run_ids,
            &runs_by_id,
            &usage_by_run,
            now_ms,
        );
        let parent_run_id = parent_run_id(&run);

        Ok(BudgetProjection {
            run_id: run.id,
            parent_run_id,
            run_usage,
            aggregate_usage,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BudgetProjection {
    run_id: RunId,
    parent_run_id: Option<RunId>,
    run_usage: BudgetUsage,
    aggregate_usage: BudgetUsage,
}

fn daemon_budget_actor() -> ApprovalActor {
    ApprovalActor::new("taugentic-daemon").expect("daemon actor id should be valid")
}

fn budget_exceeded_event(
    projection: &BudgetProjection,
    exceeded: PolicyBudgetExceeded,
) -> BudgetExceededEvent {
    let usage = match exceeded.scope {
        PolicyBudgetScope::Run => projection.run_usage,
        PolicyBudgetScope::ParentAggregate => projection.aggregate_usage,
    };
    BudgetExceededEvent {
        run_id: projection.run_id.clone(),
        parent_run_id: projection.parent_run_id.clone(),
        breach: BudgetBreach {
            scope: wire_scope(exceeded.scope),
            metric: wire_metric(exceeded.metric),
            limit: exceeded.limit,
            actual: exceeded.actual,
        },
        snapshot: BudgetSnapshot {
            run_id: projection.run_id.clone(),
            parent_run_id: projection.parent_run_id.clone(),
            scope: wire_scope(exceeded.scope),
            total_tokens: usage.total_tokens,
            wall_clock_ms: usage.wall_clock_ms,
            tool_calls: usage.tool_calls,
        },
    }
}

fn wire_scope(scope: PolicyBudgetScope) -> WireBudgetScope {
    match scope {
        PolicyBudgetScope::Run => WireBudgetScope::Run,
        PolicyBudgetScope::ParentAggregate => WireBudgetScope::ParentAggregate,
    }
}

fn wire_metric(metric: PolicyBudgetMetric) -> WireBudgetMetric {
    match metric {
        PolicyBudgetMetric::Tokens => WireBudgetMetric::Tokens,
        PolicyBudgetMetric::WallClockMs => WireBudgetMetric::WallClockMs,
        PolicyBudgetMetric::ToolCalls => WireBudgetMetric::ToolCalls,
    }
}

fn root_run_id(run: &RunProjection, runs: &BTreeMap<RunId, RunProjection>) -> RunId {
    let mut current = run;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current.id.clone()) {
            return current.id.clone();
        }
        let Some(parent_id) = parent_run_id(current) else {
            return current.id.clone();
        };
        let Some(parent) = runs.get(&parent_id) else {
            return parent_id;
        };
        current = parent;
    }
}

fn parent_run_id(run: &RunProjection) -> Option<RunId> {
    match &run.source {
        RunSource::NativeSubagent { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. }
        | RunSource::RouteSwitchedContinuation { parent_run_id, .. } => Some(parent_run_id.clone()),
        RunSource::ScheduledWork { .. } | RunSource::User { .. } | RunSource::FreshSpawn { .. } => {
            None
        }
    }
}

fn aggregate_run_ids(root: &RunId, runs: &BTreeMap<RunId, RunProjection>) -> BTreeSet<RunId> {
    let mut ids = BTreeSet::from([root.clone()]);
    loop {
        let before = ids.len();
        for run in runs.values() {
            if parent_run_id(run).is_some_and(|parent| ids.contains(&parent)) {
                ids.insert(run.id.clone());
            }
        }
        if ids.len() == before {
            return ids;
        }
    }
}

fn usage_by_run(events: &[EventRecord]) -> BTreeMap<RunId, BudgetUsage> {
    let mut usage = BTreeMap::<RunId, BudgetUsage>::new();
    for record in events {
        let DaemonEvent::AgentStream(event) = &record.payload else {
            continue;
        };
        let entry = usage.entry(event.run_id.clone()).or_default();
        match &event.emission.frame {
            AgentStreamFrame::TokenUsageUpdated {
                total_tokens: Some(total_tokens),
                ..
            } => {
                entry.total_tokens = *total_tokens;
            }
            AgentStreamFrame::ToolCallStarted { .. } => {
                entry.tool_calls = entry.tool_calls.saturating_add(1);
            }
            _ => {}
        }
    }
    usage
}

fn usage_for_run(
    run: &RunProjection,
    event_usage: Option<&BudgetUsage>,
    now_ms: u64,
) -> BudgetUsage {
    let mut usage = event_usage.copied().unwrap_or_default();
    usage.wall_clock_ms = elapsed_ms(run, now_ms);
    usage
}

fn aggregate_usage(
    root_run_id: &RunId,
    aggregate_run_ids: &BTreeSet<RunId>,
    runs_by_id: &BTreeMap<RunId, RunProjection>,
    usage_by_run: &BTreeMap<RunId, BudgetUsage>,
    now_ms: u64,
) -> BudgetUsage {
    let mut usage = BudgetUsage::default();
    for run_id in aggregate_run_ids {
        if let Some(run_usage) = usage_by_run.get(run_id) {
            usage.total_tokens = usage.total_tokens.saturating_add(run_usage.total_tokens);
            usage.tool_calls = usage.tool_calls.saturating_add(run_usage.tool_calls);
        }
    }
    usage.wall_clock_ms = runs_by_id
        .get(root_run_id)
        .map(|run| elapsed_ms(run, now_ms))
        .unwrap_or_default();
    usage
}

fn elapsed_ms(run: &RunProjection, now_ms: u64) -> u64 {
    let Some(started_at_ms) = run.started_at_ms else {
        return 0;
    };
    run.ended_at_ms
        .unwrap_or(now_ms)
        .saturating_sub(started_at_ms)
}
