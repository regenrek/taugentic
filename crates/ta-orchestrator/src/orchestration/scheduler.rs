use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use ta_protocol::wire::{DaemonEvent, RunId, RunStatus, SessionId};
use ta_store::{EventLogRepository, PersistenceStore, ProjectionRepository, StoreError};
use thiserror::Error;

pub(crate) const MAX_QUEUED_RUNS_PER_SESSION: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunScheduleDisposition {
    StartNow,
    Queued { position: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunSchedulingPolicy {
    QueueIfBusy,
    ParallelIfBusy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SchedulerRehydratePlan {
    pub demote_to_queued: Vec<(SessionId, RunId)>,
    pub promote_from_queue: Vec<(SessionId, RunId)>,
}

#[derive(Debug, Error)]
pub(crate) enum RunSchedulerError {
    #[error("run queue is full for session: {0}")]
    QueueFull(String),
}

#[derive(Debug, Clone)]
pub(crate) struct RunScheduler {
    inner: Arc<Mutex<BTreeMap<SessionId, SessionRunSchedule>>>,
    max_queued_runs_per_session: usize,
}

#[derive(Debug, Clone, Default)]
struct SessionRunSchedule {
    active_run_ids: BTreeSet<RunId>,
    queued_run_ids: VecDeque<QueuedRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueuedRun {
    run_id: RunId,
    policy: RunSchedulingPolicy,
}

impl RunScheduler {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BTreeMap::new())),
            max_queued_runs_per_session: MAX_QUEUED_RUNS_PER_SESSION,
        }
    }

    pub(crate) fn schedule_start(
        &self,
        session_id: &SessionId,
        run_id: RunId,
    ) -> Result<RunScheduleDisposition, RunSchedulerError> {
        self.schedule_start_with_policy(session_id, run_id, RunSchedulingPolicy::QueueIfBusy)
    }

    pub(crate) fn schedule_start_with_policy(
        &self,
        session_id: &SessionId,
        run_id: RunId,
        policy: RunSchedulingPolicy,
    ) -> Result<RunScheduleDisposition, RunSchedulerError> {
        let mut inner = self
            .inner
            .lock()
            .expect("run scheduler should not be poisoned");
        let schedule = inner.entry(session_id.clone()).or_default();
        if schedule.can_start_now(policy) {
            schedule.active_run_ids.insert(run_id);
            return Ok(RunScheduleDisposition::StartNow);
        }
        if schedule.queued_run_ids.len() >= self.max_queued_runs_per_session {
            return Err(RunSchedulerError::QueueFull(
                session_id.as_str().to_string(),
            ));
        }
        schedule
            .queued_run_ids
            .push_back(QueuedRun { run_id, policy });
        Ok(RunScheduleDisposition::Queued {
            position: schedule.queued_run_ids.len(),
        })
    }

    pub(crate) fn finish_run(&self, session_id: &SessionId, run_id: &RunId) -> Option<RunId> {
        let mut inner = self
            .inner
            .lock()
            .expect("run scheduler should not be poisoned");
        let schedule = inner.get_mut(session_id)?;
        if schedule.active_run_ids.remove(run_id) {
            if schedule.active_run_ids.is_empty()
                && let Some(next) = schedule.queued_run_ids.pop_front()
            {
                schedule.active_run_ids.insert(next.run_id.clone());
                return Some(next.run_id);
            }
            return None;
        }
        let queued_index = schedule
            .queued_run_ids
            .iter()
            .position(|queued_run| queued_run.run_id == *run_id)?;
        schedule.queued_run_ids.remove(queued_index);
        None
    }

    pub(crate) fn rehydrate_from_store<S>(
        &self,
        store: &S,
    ) -> Result<SchedulerRehydratePlan, StoreError>
    where
        S: PersistenceStore + EventLogRepository + ProjectionRepository + Send,
    {
        let runs = store.runs()?;
        let events = store.events()?;
        let first_run_sequence = first_run_event_sequence(&events);
        let mut grouped = BTreeMap::<SessionId, Vec<_>>::new();
        for run in runs.into_iter().filter(|run| {
            matches!(
                run.status,
                RunStatus::Queued | RunStatus::WaitingForApproval | RunStatus::Running
            )
        }) {
            grouped.entry(run.session_id.clone()).or_default().push(run);
        }

        let mut next_state = BTreeMap::new();
        let mut demote_to_queued = Vec::new();
        let mut promote_from_queue = Vec::new();

        for (session_id, mut session_runs) in grouped {
            session_runs.sort_by_key(|run| {
                (
                    first_run_sequence.get(&run.id).copied().unwrap_or(u64::MAX),
                    run.id.as_str().to_string(),
                )
            });

            let mut schedule = SessionRunSchedule::default();
            for run in session_runs {
                match run.status {
                    RunStatus::WaitingForApproval | RunStatus::Running => {
                        let policy = scheduling_policy_for_run(&run);
                        if schedule.can_start_now(policy) {
                            schedule.active_run_ids.insert(run.id.clone());
                        } else {
                            demote_to_queued.push((session_id.clone(), run.id.clone()));
                            schedule.queued_run_ids.push_back(QueuedRun {
                                run_id: run.id.clone(),
                                policy,
                            });
                        }
                    }
                    RunStatus::Queued => {
                        schedule.queued_run_ids.push_back(QueuedRun {
                            policy: scheduling_policy_for_run(&run),
                            run_id: run.id.clone(),
                        });
                    }
                    _ => {}
                }
            }

            if schedule.active_run_ids.is_empty()
                && let Some(next_run_id) = schedule.queued_run_ids.pop_front()
            {
                schedule.active_run_ids.insert(next_run_id.run_id.clone());
                promote_from_queue.push((session_id.clone(), next_run_id.run_id));
            }

            if !schedule.active_run_ids.is_empty() || !schedule.queued_run_ids.is_empty() {
                next_state.insert(session_id, schedule);
            }
        }

        *self
            .inner
            .lock()
            .expect("run scheduler should not be poisoned") = next_state;

        Ok(SchedulerRehydratePlan {
            demote_to_queued,
            promote_from_queue,
        })
    }
}

impl SessionRunSchedule {
    fn can_start_now(&self, policy: RunSchedulingPolicy) -> bool {
        self.queued_run_ids.is_empty()
            && (self.active_run_ids.is_empty()
                || matches!(policy, RunSchedulingPolicy::ParallelIfBusy))
    }
}

fn scheduling_policy_for_run(run: &ta_store::RunProjection) -> RunSchedulingPolicy {
    match &run.source {
        ta_protocol::wire::RunSource::NativeSubagent {
            workspace_scope: ta_protocol::wire::WorkspaceMode::WorktreeWrite,
            ..
        } => RunSchedulingPolicy::ParallelIfBusy,
        _ => RunSchedulingPolicy::QueueIfBusy,
    }
}

fn first_run_event_sequence(events: &[ta_store::EventRecord]) -> HashMap<RunId, u64> {
    let mut sequences = HashMap::new();
    for record in events {
        let DaemonEvent::Run(run_event) = &record.payload else {
            continue;
        };
        sequences
            .entry(run_event.run_id.clone())
            .or_insert(record.sequence);
    }
    sequences
}
