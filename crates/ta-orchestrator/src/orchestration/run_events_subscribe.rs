use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use ta_store::{PersistenceStore, RunEventRangeQuery, StoreError, event_run_id};

use super::app::{AppService, AppServiceError};
use crate::{
    DaemonEventKind, PublicDaemonEvent, RunEventDelta, RunEventStreamError, RunHarnessKind,
    RunStatus, SubscribeRunEventsRequest,
};

pub(super) const RUN_EVENT_REPLAY_BATCH_LIMIT: usize = 500;
const RUN_EVENT_REPLAY_TOTAL_LIMIT: usize = 10_000;
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const RUN_EVENT_SUBSCRIBER_QUEUE_DEPTH: usize = 256;

pub type RunEventStreamResult = Result<RunEventDelta, RunEventStreamError>;

#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct RunEventSubscription {
    pub replay: Vec<RunEventDelta>,
    pub latest_event_seq: Option<u64>,
    pub receiver: mpsc::Receiver<RunEventStreamResult>,
    #[allow(dead_code)]
    pub overflowed: Arc<AtomicBool>,
    pub live: bool,
    #[allow(dead_code)]
    live_cleanup: Option<crate::host::event_hub::RuntimeEventSubscriptionCleanup>,
}

struct RunEventReplay {
    events: Vec<RunEventDelta>,
    latest_event_seq: Option<u64>,
    history_gap: bool,
}

pub(super) fn subscribe_run_events<S>(
    service: &AppService<S>,
    session_id: &crate::SessionId,
    request: &SubscribeRunEventsRequest,
) -> Result<RunEventSubscription, AppServiceError>
where
    S: PersistenceStore + Send + 'static,
{
    subscribe_run_events_inner(service, session_id, request, || {})
}

fn subscribe_run_events_inner<S, BeforeLiveSubscribe>(
    service: &AppService<S>,
    session_id: &crate::SessionId,
    request: &SubscribeRunEventsRequest,
    before_live_subscribe: BeforeLiveSubscribe,
) -> Result<RunEventSubscription, AppServiceError>
where
    S: PersistenceStore + Send + 'static,
    BeforeLiveSubscribe: FnOnce(),
{
    if request.session_id != *session_id {
        return Err(AppServiceError::RunSessionMismatch(
            request.run_id.as_str().to_string(),
        ));
    }

    let (run_status, latest_persisted_sequence) = {
        let store = service
            .store
            .lock()
            .expect("app store should not be poisoned");
        let Some(run) = store.run(&request.run_id)? else {
            return Err(AppServiceError::RunNotFound(
                request.run_id.as_str().to_string(),
            ));
        };
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        if run.harness != RunHarnessKind::Native {
            return Err(AppServiceError::RunNotNativeHarness(
                run.id.as_str().to_string(),
            ));
        }
        (run.status, run.last_event_seq)
    };

    let should_subscribe_live = matches!(
        run_status,
        RunStatus::Running | RunStatus::WaitingForApproval
    ) && service
        .run_execution
        .is_live_run_running(&request.run_id, session_id);

    let live_subscription = if should_subscribe_live {
        before_live_subscribe();
        Some(service.runtime.subscribe_events(
            session_id,
            &run_event_subscription_kinds(),
            latest_persisted_sequence,
            None,
        ))
    } else {
        None
    };
    let replay_boundary = live_subscription
        .as_ref()
        .and_then(|subscription| {
            subscription
                .latest_cursor
                .as_ref()
                .map(|cursor| cursor.sequence)
        })
        .or(latest_persisted_sequence);

    let replay = {
        let store = service
            .store
            .lock()
            .expect("app store should not be poisoned");
        replay_run_events_until(
            &*store,
            session_id,
            &request.run_id,
            request.after_seq,
            replay_boundary,
        )?
    };

    if replay.history_gap {
        return Ok(RunEventSubscription {
            replay: replay.events,
            latest_event_seq: replay.latest_event_seq,
            receiver: terminal_run_event_receiver(Some(RunEventStreamError::HistoryGap)),
            overflowed: Arc::new(AtomicBool::new(false)),
            live: false,
            live_cleanup: None,
        });
    }

    let Some(live_subscription) = live_subscription else {
        return Ok(RunEventSubscription {
            replay: replay.events,
            latest_event_seq: replay.latest_event_seq,
            receiver: terminal_run_event_receiver(None),
            overflowed: Arc::new(AtomicBool::new(false)),
            live: false,
            live_cleanup: None,
        });
    };

    let (receiver, overflowed, live_cleanup) =
        spawn_run_event_delta_bridge(request.run_id.clone(), replay_boundary, live_subscription);
    Ok(RunEventSubscription {
        replay: replay.events,
        latest_event_seq: replay.latest_event_seq,
        receiver,
        overflowed,
        live: true,
        live_cleanup: Some(live_cleanup),
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn run_event_subscription_kinds() -> [DaemonEventKind; 6] {
    [
        DaemonEventKind::Run,
        DaemonEventKind::Approval,
        DaemonEventKind::Artifact,
        DaemonEventKind::ContextReceipt,
        DaemonEventKind::AgentStream,
        DaemonEventKind::Budget,
    ]
}

fn replay_run_events_until(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    run_id: &crate::RunId,
    after_seq: Option<u64>,
    replay_boundary: Option<u64>,
) -> Result<RunEventReplay, StoreError> {
    let mut events = Vec::new();
    let mut after_sequence = after_seq;
    let mut latest_replayed_seq = after_seq.unwrap_or(0);

    loop {
        if replay_boundary.is_some_and(|boundary| latest_replayed_seq >= boundary) {
            break;
        }
        if events.len() >= RUN_EVENT_REPLAY_TOTAL_LIMIT {
            let probe_record = store
                .read_run_events(&RunEventRangeQuery {
                    session_id: session_id.clone(),
                    run_id: run_id.clone(),
                    after_sequence: Some(latest_replayed_seq),
                    limit: 1,
                })?
                .records
                .into_iter()
                .next();
            let Some(probe_record) = probe_record else {
                return Ok(RunEventReplay {
                    events,
                    latest_event_seq: Some(latest_replayed_seq),
                    history_gap: false,
                });
            };
            if replay_boundary.is_some_and(|boundary| probe_record.sequence > boundary) {
                return Ok(RunEventReplay {
                    events,
                    latest_event_seq: replay_boundary,
                    history_gap: false,
                });
            }
            return Ok(RunEventReplay {
                events,
                latest_event_seq: replay_boundary,
                history_gap: true,
            });
        }

        let remaining = RUN_EVENT_REPLAY_TOTAL_LIMIT - events.len();
        let range = store.read_run_events(&RunEventRangeQuery {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            after_sequence,
            limit: remaining.min(RUN_EVENT_REPLAY_BATCH_LIMIT),
        })?;
        if range.records.is_empty() {
            return Ok(RunEventReplay {
                events,
                latest_event_seq: replay_boundary.or(range.latest_sequence),
                history_gap: false,
            });
        }

        let mut reached_boundary = false;
        for record in range.records {
            if replay_boundary.is_some_and(|boundary| record.sequence > boundary) {
                reached_boundary = true;
                break;
            }
            latest_replayed_seq = record.sequence;
            after_sequence = Some(record.sequence);
            events.push(RunEventDelta {
                seq: record.sequence,
                event: PublicDaemonEvent::from(record.payload),
            });
        }
        if reached_boundary {
            break;
        }
    }

    Ok(RunEventReplay {
        events,
        latest_event_seq: replay_boundary,
        history_gap: false,
    })
}

fn terminal_run_event_receiver(
    terminal_error: Option<RunEventStreamError>,
) -> mpsc::Receiver<RunEventStreamResult> {
    let (sender, receiver) = mpsc::sync_channel(RUN_EVENT_SUBSCRIBER_QUEUE_DEPTH);
    if let Some(error) = terminal_error {
        let _ = sender.send(Err(error));
    }
    drop(sender);
    receiver
}

#[cfg_attr(not(test), allow(dead_code))]
fn spawn_run_event_delta_bridge(
    run_id: crate::RunId,
    replay_boundary: Option<u64>,
    live_subscription: crate::host::event_hub::RuntimeEventSubscription,
) -> (
    mpsc::Receiver<RunEventStreamResult>,
    Arc<AtomicBool>,
    crate::host::event_hub::RuntimeEventSubscriptionCleanup,
) {
    let (sender, receiver) = mpsc::sync_channel(RUN_EVENT_SUBSCRIBER_QUEUE_DEPTH);
    let overflowed = Arc::clone(&live_subscription.overflowed);
    let bridge_overflowed = Arc::clone(&overflowed);
    let crate::host::event_hub::RuntimeEventSubscription {
        receiver: live_receiver,
        cleanup,
        ..
    } = live_subscription;
    let bridge_cleanup = cleanup.clone();
    let spawn_result = thread::Builder::new()
        .name(format!("run-event-splice-{}", run_id.as_str()))
        .spawn(move || {
            let _cleanup = bridge_cleanup;
            while !bridge_overflowed.load(Ordering::SeqCst) {
                let event = match live_receiver.recv() {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if replay_boundary.is_some_and(|boundary| event.sequence <= boundary) {
                    continue;
                }
                if event_run_id(&event.event) != Some(&run_id) {
                    continue;
                }
                let delta = RunEventDelta {
                    seq: event.sequence,
                    event: PublicDaemonEvent::from(event.event),
                };
                match sender.try_send(Ok(delta)) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        bridge_overflowed.store(true, Ordering::SeqCst);
                        break;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
            if bridge_overflowed.load(Ordering::SeqCst) {
                let _ = sender.send(Err(RunEventStreamError::Lagged));
            }
        });
    if spawn_result.is_err() {
        overflowed.store(true, Ordering::SeqCst);
    }
    (receiver, overflowed, cleanup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppDeferredMutationResult, DaemonEvent, OpenSessionRequest, RunId, RunSource, RunSummary,
        RuntimeProfileId, SessionId,
    };
    use ta_store::{
        EventRecord, ProjectionRepository, RunProjection, test_support::StoreSeedRepository,
    };

    const TEST_OWNER_PRINCIPAL_ID: &str = "principal-test-owner";
    const TEST_CLIENT_NAME: &str = "app-tests";

    fn open_session(service: &AppService) -> crate::OpenSessionResult {
        service
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &OpenSessionRequest {
                    title: "Live splice module".to_string(),
                },
            )
            .expect("session should open")
    }

    fn ensure_running_run(
        service: &AppService,
        session_id: &SessionId,
        objective: &str,
    ) -> AppDeferredMutationResult<RunSummary> {
        service
            .seed_running_run_for_tests(session_id, objective)
            .expect("seeded run should start")
    }

    fn last_event_seq(service: &AppService, run_id: &RunId) -> u64 {
        service
            .store
            .lock()
            .expect("app store should not be poisoned")
            .run(run_id)
            .expect("run lookup should succeed")
            .expect("run should exist")
            .last_event_seq
            .expect("seed event sequence")
    }

    fn seed_native_run_projection(
        service: &AppService,
        session_id: &SessionId,
        run_id: &RunId,
        objective: &str,
    ) {
        service
            .store
            .lock()
            .expect("app store should not be poisoned")
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: objective.to_string(),
                status: RunStatus::Running,
                harness: RunHarnessKind::Native,
                source: RunSource::default(),
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            })
            .expect("run projection should seed");
    }

    fn run_event_record(
        session_id: &SessionId,
        run_id: &RunId,
        sequence: u64,
        detail: &str,
    ) -> EventRecord {
        EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms: sequence * 10,
            payload: DaemonEvent::Run(crate::RunEvent {
                run_id: run_id.clone(),
                status: RunStatus::Running,
                detail: detail.to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
        }
    }

    fn append_and_publish_run_event(
        service: &AppService,
        session_id: &SessionId,
        run_id: &RunId,
        sequence: u64,
        detail: &str,
    ) {
        let record = run_event_record(session_id, run_id, sequence, detail);
        {
            let mut store = service
                .store
                .lock()
                .expect("app store should not be poisoned");
            store
                .append_event(record.clone())
                .expect("run event should append");
            let mut run = store
                .run(run_id)
                .expect("run lookup should succeed")
                .expect("run should exist");
            run.last_event_seq = Some(sequence);
            store.save_run(run).expect("run projection should update");
        }
        service.runtime.publish_record(&record);
    }

    #[test]
    fn subscribe_run_events_uses_hub_cursor_for_subscribe_boundary() {
        let service = AppService::bootstrap().expect("app service should boot");
        let session = open_session(&service);
        let run = ensure_running_run(&service, &session.id, "Replay subscribe race");
        let run_id = run.body.id.clone();
        let after_seq = last_event_seq(&service, &run_id);
        let race_seq = after_seq + 1;

        let subscription = subscribe_run_events_inner(
            &service,
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run_id.clone(),
                after_seq: Some(after_seq),
            },
            || {
                append_and_publish_run_event(
                    &service,
                    &session.id,
                    &run_id,
                    race_seq,
                    "persisted before hub subscribe",
                );
            },
        )
        .expect("live run should subscribe");

        assert!(subscription.live);
        assert_eq!(
            subscription
                .replay
                .iter()
                .map(|event| event.seq)
                .collect::<Vec<_>>(),
            vec![race_seq]
        );
        assert_eq!(subscription.latest_event_seq, Some(race_seq));
        assert!(matches!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn subscribe_run_events_total_replay_limit_ignores_later_other_run_event() {
        let service = AppService::bootstrap().expect("app service should boot");
        let session = open_session(&service);
        let run = ensure_running_run(&service, &session.id, "Replay exact cap");
        let run_id = run.body.id.clone();
        let other_run_id = RunId::new("run-replay-cap-other").expect("run id");
        let after_seq = last_event_seq(&service, &run_id);

        seed_native_run_projection(
            &service,
            &session.id,
            &other_run_id,
            "Same-session unrelated run",
        );
        for offset in 1..=RUN_EVENT_REPLAY_TOTAL_LIMIT as u64 {
            append_and_publish_run_event(
                &service,
                &session.id,
                &run_id,
                after_seq + offset,
                "exact cap replay",
            );
        }
        append_and_publish_run_event(
            &service,
            &session.id,
            &other_run_id,
            after_seq + RUN_EVENT_REPLAY_TOTAL_LIMIT as u64 + 1,
            "same-session other run after cap",
        );

        let subscription = subscribe_run_events(
            &service,
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id,
                after_seq: Some(after_seq),
            },
        )
        .expect("live run should subscribe without history gap");

        assert!(subscription.live);
        assert_eq!(subscription.replay.len(), RUN_EVENT_REPLAY_TOTAL_LIMIT);
        assert_eq!(
            subscription.replay.last().map(|event| event.seq),
            Some(after_seq + RUN_EVENT_REPLAY_TOTAL_LIMIT as u64)
        );
        assert_eq!(
            subscription.latest_event_seq,
            Some(after_seq + RUN_EVENT_REPLAY_TOTAL_LIMIT as u64)
        );
        assert!(matches!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn subscribe_run_events_live_splice_cap_probe_ignores_run_event_after_replay_boundary() {
        let service = AppService::bootstrap().expect("app service should boot");
        let session = open_session(&service);
        let run = ensure_running_run(&service, &session.id, "Replay cap live boundary");
        let run_id = run.body.id.clone();
        let other_run_id = RunId::new("run-replay-boundary-owner").expect("run id");
        let after_seq = last_event_seq(&service, &run_id);

        for offset in 1..=RUN_EVENT_REPLAY_TOTAL_LIMIT as u64 {
            append_and_publish_run_event(
                &service,
                &session.id,
                &run_id,
                after_seq + offset,
                "exact cap before live boundary",
            );
        }
        let cap_seq = after_seq + RUN_EVENT_REPLAY_TOTAL_LIMIT as u64;
        let replay_boundary = cap_seq + 1;
        let live_seq = replay_boundary + 1;
        let live_record = run_event_record(
            &session.id,
            &run_id,
            live_seq,
            "persisted live event after replay boundary",
        );
        // Model the race where a live event is persisted after the subscribe boundary snapshot
        // but before the replay cap probe reads the store.
        {
            let mut store = service
                .store
                .lock()
                .expect("app store should not be poisoned");
            store
                .append_event(live_record.clone())
                .expect("post-boundary run event should append");
        }

        let subscription = subscribe_run_events_inner(
            &service,
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run_id.clone(),
                after_seq: Some(after_seq),
            },
            || {
                service.runtime.publish_record(&run_event_record(
                    &session.id,
                    &other_run_id,
                    replay_boundary,
                    "live boundary from same-session unrelated run",
                ));
            },
        )
        .expect("post-boundary probe event should not create history gap");

        assert!(subscription.live);
        assert_eq!(subscription.replay.len(), RUN_EVENT_REPLAY_TOTAL_LIMIT);
        assert_eq!(
            subscription.replay.last().map(|event| event.seq),
            Some(cap_seq)
        );
        assert_eq!(subscription.latest_event_seq, Some(replay_boundary));

        service.runtime.publish_record(&live_record);
        assert_eq!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(200))
                .expect("post-boundary live event should arrive")
                .expect("post-boundary live item should be ok")
                .seq,
            live_seq
        );
    }

    #[test]
    fn subscribe_run_events_total_replay_limit_returns_history_gap() {
        let service = AppService::bootstrap().expect("app service should boot");
        let session = open_session(&service);
        let run = ensure_running_run(&service, &session.id, "Replay total cap");
        let run_id = run.body.id.clone();
        let after_seq = last_event_seq(&service, &run_id);

        for offset in 1..=(RUN_EVENT_REPLAY_TOTAL_LIMIT as u64 + 1) {
            append_and_publish_run_event(
                &service,
                &session.id,
                &run_id,
                after_seq + offset,
                "total cap replay",
            );
        }

        let subscription = subscribe_run_events(
            &service,
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id,
                after_seq: Some(after_seq),
            },
        )
        .expect("live run should return history gap subscription");

        assert!(!subscription.live);
        assert_eq!(subscription.replay.len(), RUN_EVENT_REPLAY_TOTAL_LIMIT);
        assert_eq!(
            subscription.replay.last().map(|event| event.seq),
            Some(after_seq + RUN_EVENT_REPLAY_TOTAL_LIMIT as u64)
        );
        assert_eq!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(200))
                .expect("stream should report history gap"),
            Err(RunEventStreamError::HistoryGap)
        );
        assert!(matches!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
