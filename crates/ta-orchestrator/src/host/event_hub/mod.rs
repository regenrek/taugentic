use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use ta_store::{EventPersistence, EventRecord, event_persistence};

use crate::{DaemonEvent, DaemonEventCursor, DaemonEventEnvelope, DaemonEventKind, SessionId};

const MAX_SUBSCRIBER_QUEUE_DEPTH: usize = 256;
const MAX_EVENT_BACKLOG_DEPTH: usize = 512;

#[derive(Debug)]
pub(crate) struct RuntimeEventSubscription {
    pub cleanup: RuntimeEventSubscriptionCleanup,
    pub latest_cursor: Option<DaemonEventCursor>,
    pub backlog: Vec<DaemonEventEnvelope>,
    pub receiver: mpsc::Receiver<DaemonEventEnvelope>,
    pub overflowed: Arc<AtomicBool>,
    pub has_gap: bool,
}

#[derive(Debug)]
pub(crate) struct NavigationInvalidationSubscription {
    pub cleanup: NavigationInvalidationSubscriptionCleanup,
    pub receiver: mpsc::Receiver<()>,
}

#[derive(Debug, Clone)]
pub(crate) struct NavigationInvalidationSubscriptionCleanup {
    id: u64,
    inner: Weak<Mutex<RuntimeEventHubState>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeEventSubscriptionCleanup {
    id: u64,
    inner: Weak<Mutex<RuntimeEventHubState>>,
}

/// Shared runtime event fan-out.
///
/// Publish stays non-blocking: each subscriber gets a bounded queue, and a slow
/// subscriber is marked overflowed and removed instead of stalling other
/// subscribers. The JSON-RPC host closes that client session and relies on the
/// existing replay/history-gap path for recovery rather than emitting an
/// in-band lag marker.
#[derive(Debug, Clone)]
pub(crate) struct RuntimeEventHub {
    inner: Arc<Mutex<RuntimeEventHubState>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeEventPublisher {
    daemon_instance_id: String,
    event_hub: RuntimeEventHub,
}

#[derive(Debug)]
struct RuntimeEventHubState {
    next_subscriber_id: u64,
    latest_sequence_by_session: BTreeMap<SessionId, u64>,
    backlogs_by_session: BTreeMap<SessionId, SessionEventBacklog>,
    subscribers: Vec<RuntimeEventSubscriber>,
    navigation_principals_by_session: BTreeMap<SessionId, String>,
    navigation_subscribers: Vec<NavigationInvalidationSubscriber>,
}

#[derive(Debug)]
struct RuntimeEventSubscriber {
    id: u64,
    session_id: SessionId,
    kinds: Vec<DaemonEventKind>,
    sender: mpsc::SyncSender<DaemonEventEnvelope>,
    overflowed: Arc<AtomicBool>,
}

#[derive(Debug)]
struct NavigationInvalidationSubscriber {
    id: u64,
    principal_id: String,
    sender: mpsc::SyncSender<()>,
}

#[derive(Debug, Default)]
struct SessionEventBacklog {
    records: VecDeque<DaemonEventEnvelope>,
    required_after_sequence: Option<u64>,
}

impl RuntimeEventHub {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RuntimeEventHubState {
                next_subscriber_id: 1,
                latest_sequence_by_session: BTreeMap::new(),
                backlogs_by_session: BTreeMap::new(),
                subscribers: Vec::new(),
                navigation_principals_by_session: BTreeMap::new(),
                navigation_subscribers: Vec::new(),
            })),
        }
    }

    pub(crate) fn register_navigation_session(&self, session_id: &SessionId, principal_id: &str) {
        self.inner
            .lock()
            .expect("runtime event hub should not be poisoned")
            .navigation_principals_by_session
            .insert(session_id.clone(), principal_id.to_string());
    }

    pub(crate) fn subscribe_navigation(
        &self,
        principal_id: &str,
    ) -> NavigationInvalidationSubscription {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut inner = self
            .inner
            .lock()
            .expect("runtime event hub should not be poisoned");
        let subscriber_id = inner.next_subscriber_id;
        inner.next_subscriber_id = inner
            .next_subscriber_id
            .checked_add(1)
            .expect("subscriber id space exhausted");
        inner
            .navigation_subscribers
            .push(NavigationInvalidationSubscriber {
                id: subscriber_id,
                principal_id: principal_id.to_string(),
                sender,
            });
        NavigationInvalidationSubscription {
            cleanup: NavigationInvalidationSubscriptionCleanup {
                id: subscriber_id,
                inner: Arc::downgrade(&self.inner),
            },
            receiver,
        }
    }

    pub(crate) fn publish_navigation_for_principal(&self, principal_id: &str) {
        let mut inner = self
            .inner
            .lock()
            .expect("runtime event hub should not be poisoned");
        inner.navigation_subscribers.retain(|subscriber| {
            if subscriber.principal_id != principal_id {
                return true;
            }
            match subscriber.sender.try_send(()) {
                Ok(()) | Err(mpsc::TrySendError::Full(())) => true,
                Err(mpsc::TrySendError::Disconnected(())) => false,
            }
        });
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn latest_cursor_for_session(
        &self,
        daemon_instance_id: &str,
        session_id: &SessionId,
    ) -> Option<DaemonEventCursor> {
        self.inner
            .lock()
            .expect("runtime event hub should not be poisoned")
            .latest_sequence_by_session
            .get(session_id)
            .copied()
            .map(|sequence| to_event_cursor(daemon_instance_id, session_id, sequence))
    }

    pub(crate) fn publish(
        &self,
        daemon_instance_id: &str,
        record: &EventRecord,
    ) -> DaemonEventEnvelope {
        let mut inner = self
            .inner
            .lock()
            .expect("runtime event hub should not be poisoned");
        let session_latest = inner
            .latest_sequence_by_session
            .get(&record.session_id)
            .copied()
            .map_or(record.sequence, |latest| latest.max(record.sequence));
        inner
            .latest_sequence_by_session
            .insert(record.session_id.clone(), session_latest);
        let envelope = to_envelope(daemon_instance_id, record);
        inner
            .backlogs_by_session
            .entry(record.session_id.clone())
            .or_default()
            .push(envelope.clone());
        inner.subscribers.retain(|subscriber| {
            if subscriber.session_id != record.session_id {
                return true;
            }
            if !matches_subscription_kind(&envelope.event, &subscriber.kinds) {
                return true;
            }

            match subscriber.sender.try_send(envelope.clone()) {
                Ok(()) => true,
                Err(mpsc::TrySendError::Disconnected(_)) => false,
                Err(mpsc::TrySendError::Full(_)) => {
                    subscriber.overflowed.store(true, Ordering::SeqCst);
                    false
                }
            }
        });
        if matches!(
            record.payload,
            crate::DaemonEvent::Session(_)
                | crate::DaemonEvent::Run(_)
                | crate::DaemonEvent::RunReconciledOnStartup(_)
                | crate::DaemonEvent::Approval(_)
        ) && let Some(principal_id) = inner
            .navigation_principals_by_session
            .get(&record.session_id)
            .cloned()
        {
            inner.navigation_subscribers.retain(|subscriber| {
                if subscriber.principal_id != principal_id {
                    return true;
                }
                match subscriber.sender.try_send(()) {
                    Ok(()) | Err(mpsc::TrySendError::Full(())) => true,
                    Err(mpsc::TrySendError::Disconnected(())) => false,
                }
            });
        }
        envelope
    }

    pub(crate) fn subscribe(
        &self,
        daemon_instance_id: &str,
        session_id: &SessionId,
        kinds: &[DaemonEventKind],
        latest_persisted_sequence: Option<u64>,
        after_cursor: Option<&DaemonEventCursor>,
    ) -> RuntimeEventSubscription {
        let (sender, receiver) = mpsc::sync_channel(MAX_SUBSCRIBER_QUEUE_DEPTH);
        let overflowed = Arc::new(AtomicBool::new(false));
        let mut inner = self
            .inner
            .lock()
            .expect("runtime event hub should not be poisoned");
        let subscriber_id = inner.next_subscriber_id;
        inner.next_subscriber_id = inner
            .next_subscriber_id
            .checked_add(1)
            .expect("subscriber id space exhausted");
        let latest_live_sequence = inner.latest_sequence_by_session.get(session_id).copied();
        let latest_sequence = match (latest_live_sequence, latest_persisted_sequence) {
            (Some(live), Some(persisted)) => Some(live.max(persisted)),
            (Some(live), None) => Some(live),
            (None, Some(persisted)) => Some(persisted),
            (None, None) => None,
        };
        let latest_live_cursor = latest_sequence
            .map(|sequence| to_event_cursor(daemon_instance_id, session_id, sequence));
        let latest_persisted_cursor = latest_persisted_sequence
            .map(|sequence| to_event_cursor(daemon_instance_id, session_id, sequence));
        let backlog = inner.backlogs_by_session.get(session_id);
        let (has_gap, replay_backlog) = match after_cursor {
            None => (false, Vec::new()),
            Some(cursor) if cursor.daemon_instance_id != daemon_instance_id => (true, Vec::new()),
            Some(cursor) if cursor.session_id != *session_id => (true, Vec::new()),
            Some(cursor) => match latest_sequence {
                None if cursor.sequence == 0 => (false, Vec::new()),
                None => (true, Vec::new()),
                Some(current_latest) if cursor.sequence > current_latest => (true, Vec::new()),
                Some(current_latest) if cursor.sequence == current_latest => (false, Vec::new()),
                Some(_) => match backlog {
                    Some(backlog) if backlog.can_resume_from(cursor.sequence) => {
                        (false, backlog.events_after(cursor.sequence, kinds))
                    }
                    _ => (true, Vec::new()),
                },
            },
        };
        let latest_cursor = if has_gap {
            latest_persisted_cursor
        } else {
            latest_live_cursor
        };
        inner.subscribers.push(RuntimeEventSubscriber {
            id: subscriber_id,
            session_id: session_id.clone(),
            kinds: kinds.to_vec(),
            sender,
            overflowed: Arc::clone(&overflowed),
        });
        RuntimeEventSubscription {
            cleanup: RuntimeEventSubscriptionCleanup {
                id: subscriber_id,
                inner: Arc::downgrade(&self.inner),
            },
            latest_cursor,
            backlog: replay_backlog,
            receiver,
            overflowed,
            has_gap,
        }
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count_for_session(&self, session_id: &SessionId) -> usize {
        self.inner
            .lock()
            .expect("runtime event hub should not be poisoned")
            .subscribers
            .iter()
            .filter(|subscriber| subscriber.session_id == *session_id)
            .count()
    }
}

impl Drop for RuntimeEventSubscriptionCleanup {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        inner
            .lock()
            .expect("runtime event hub should not be poisoned")
            .subscribers
            .retain(|subscriber| subscriber.id != self.id);
    }
}

impl Drop for NavigationInvalidationSubscriptionCleanup {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        inner
            .lock()
            .expect("runtime event hub should not be poisoned")
            .navigation_subscribers
            .retain(|subscriber| subscriber.id != self.id);
    }
}

impl RuntimeEventPublisher {
    pub(crate) fn new(daemon_instance_id: String, event_hub: RuntimeEventHub) -> Self {
        Self {
            daemon_instance_id,
            event_hub,
        }
    }

    pub(crate) fn publish(&self, record: &EventRecord) -> DaemonEventEnvelope {
        self.event_hub.publish(&self.daemon_instance_id, record)
    }
}

impl SessionEventBacklog {
    fn push(&mut self, envelope: DaemonEventEnvelope) {
        self.records.push_back(envelope);
        while self.records.len() > MAX_EVENT_BACKLOG_DEPTH {
            if let Some(index) = self.records.iter().position(|record| {
                matches!(
                    event_persistence(&record.event),
                    EventPersistence::Transient
                )
            }) {
                self.records.remove(index);
                continue;
            }

            let dropped = self
                .records
                .pop_front()
                .expect("backlog overflow requires at least one event");
            self.required_after_sequence = Some(
                self.required_after_sequence
                    .map_or(dropped.sequence, |current| current.max(dropped.sequence)),
            );
        }
    }

    fn can_resume_from(&self, after_sequence: u64) -> bool {
        self.required_after_sequence
            .is_none_or(|required| after_sequence >= required)
    }

    fn events_after(
        &self,
        after_sequence: u64,
        kinds: &[DaemonEventKind],
    ) -> Vec<DaemonEventEnvelope> {
        self.records
            .iter()
            .filter(|record| record.sequence > after_sequence)
            .filter(|record| matches_subscription_kind(&record.event, kinds))
            .cloned()
            .collect()
    }
}

pub(crate) fn to_event_cursor(
    daemon_instance_id: &str,
    session_id: &SessionId,
    sequence: u64,
) -> DaemonEventCursor {
    DaemonEventCursor {
        daemon_instance_id: daemon_instance_id.to_string(),
        session_id: session_id.clone(),
        sequence,
    }
}

fn to_envelope(daemon_instance_id: &str, record: &EventRecord) -> DaemonEventEnvelope {
    DaemonEventEnvelope {
        daemon_instance_id: daemon_instance_id.to_string(),
        session_id: record.session_id.clone(),
        sequence: record.sequence,
        occurred_at_ms: record.occurred_at_ms,
        event: record.payload.clone(),
    }
}

fn matches_subscription_kind(event: &DaemonEvent, kinds: &[DaemonEventKind]) -> bool {
    kinds.is_empty() || kinds.contains(&event.kind())
}

#[cfg(test)]
mod tests {
    use ta_host_platform::{HostCapabilities, HostOs, HostPlatform, LocalIpcKind, OsVersion};

    use super::*;
    use crate::{
        AgentStreamEvent, AgentStreamFrame, AgentToolCallOutcome, RunEvent, RunId, RunStatus,
        SessionEvent, SessionStatus, StreamEmission,
    };

    fn fixture_host_platform() -> HostPlatform {
        HostPlatform {
            os: HostOs::Linux,
            version: OsVersion::parse("6.9.0"),
            edition: None,
            linux_distribution: None,
            capabilities: HostCapabilities {
                local_ipc: LocalIpcKind::UnixDomainSocket {
                    runtime_dir: std::path::PathBuf::from("/tmp/taugentic"),
                },
                sandbox: ta_host_platform::SandboxKind::LinuxLandlockBwrap,
                supports_unix_peer_credentials: true,
                supports_launchd_user_services: false,
                supports_systemd_user_services: true,
                supports_windows_service_control: false,
            },
        }
    }

    fn runtime() -> crate::RuntimeService {
        crate::RuntimeService::from_host_platform(fixture_host_platform())
    }

    fn run_record(session_id: &SessionId, sequence: u64) -> EventRecord {
        EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms: 100 + sequence,
            payload: DaemonEvent::Run(RunEvent {
                run_id: RunId::new(format!("run-{sequence}")).expect("run id"),
                status: RunStatus::Running,
                detail: "running".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
        }
    }

    fn transient_agent_stream_record(session_id: &SessionId, sequence: u64) -> EventRecord {
        EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms: 100 + sequence,
            payload: DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: RunId::new("run-agent-stream").expect("run id"),
                emission: StreamEmission {
                    turn_id: None,
                    item_id: None,
                    fragment_sequence: Some(sequence),
                    frame: AgentStreamFrame::AssistantMessageDelta {
                        delta: format!("delta-{sequence}"),
                    },
                },
            }),
        }
    }

    fn durable_agent_stream_record(
        session_id: &SessionId,
        sequence: u64,
        frame: AgentStreamFrame,
    ) -> EventRecord {
        EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms: 100 + sequence,
            payload: DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: RunId::new("run-agent-stream").expect("run id"),
                emission: StreamEmission {
                    turn_id: None,
                    item_id: None,
                    fragment_sequence: None,
                    frame,
                },
            }),
        }
    }

    #[test]
    fn runtime_event_hub_tracks_latest_committed_sequence() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        let envelope = runtime.publish_record(&EventRecord {
            sequence: 7,
            session_id: session_id.clone(),
            occurred_at_ms: 42,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Running,
            }),
        });

        assert_eq!(
            runtime.latest_cursor_for_session(&session_id),
            Some(DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id,
                sequence: 7,
            })
        );
        assert_eq!(envelope.sequence, 7);
        assert_eq!(envelope.occurred_at_ms, 42);
    }

    #[test]
    fn runtime_event_hub_delivers_live_events_to_subscribers() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        let subscription =
            runtime.subscribe_events(&session_id, &[DaemonEventKind::Run], None, None);
        let published = runtime.publish_record(&run_record(&session_id, 1));

        let received = subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("subscriber should receive live event");

        assert_eq!(subscription.latest_cursor, None);
        assert!(subscription.backlog.is_empty());
        assert!(!subscription.has_gap);
        assert_eq!(received, published);
    }

    #[test]
    fn runtime_event_hub_removes_dropped_subscriber_without_publish() {
        let runtime = runtime();
        let session_id = SessionId::new("session-drop").expect("session id");
        let subscription =
            runtime.subscribe_events(&session_id, &[DaemonEventKind::Run], None, None);

        assert_eq!(runtime.subscriber_count_for_session(&session_id), 1);
        drop(subscription);

        assert_eq!(runtime.subscriber_count_for_session(&session_id), 0);
    }

    #[test]
    fn runtime_event_hub_replays_recoverable_backlog_after_cursor() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        runtime.publish_record(&run_record(&session_id, 1));
        let second = runtime.publish_record(&transient_agent_stream_record(&session_id, 2));

        let subscription = runtime.subscribe_events(
            &session_id,
            &[DaemonEventKind::AgentStream],
            Some(1),
            Some(&DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id: session_id.clone(),
                sequence: 1,
            }),
        );

        assert!(!subscription.has_gap);
        assert_eq!(
            subscription.latest_cursor,
            Some(DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id,
                sequence: 2,
            })
        );
        assert_eq!(subscription.backlog, vec![second]);
    }

    #[test]
    fn runtime_event_hub_returns_history_gap_for_persisted_gap_without_live_backlog() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");

        let subscription = runtime.subscribe_events(
            &session_id,
            &[DaemonEventKind::Run],
            Some(3),
            Some(&DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id: session_id.clone(),
                sequence: 2,
            }),
        );

        assert!(subscription.has_gap);
        assert!(subscription.backlog.is_empty());
        assert_eq!(
            subscription.latest_cursor,
            Some(DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id,
                sequence: 3,
            })
        );
    }

    #[test]
    fn runtime_event_hub_caps_history_gap_cursor_to_latest_persisted_sequence() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        runtime.publish_record(&EventRecord {
            sequence: 1,
            session_id: session_id.clone(),
            occurred_at_ms: 101,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Idle,
            }),
        });
        runtime.publish_record(&transient_agent_stream_record(&session_id, 2));

        let subscription = runtime.subscribe_events(
            &session_id,
            &[DaemonEventKind::AgentStream],
            Some(1),
            Some(&DaemonEventCursor {
                daemon_instance_id: "stale-daemon".to_string(),
                session_id: session_id.clone(),
                sequence: 0,
            }),
        );

        assert!(subscription.has_gap);
        assert!(subscription.backlog.is_empty());
        assert_eq!(
            subscription.latest_cursor,
            Some(DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id,
                sequence: 1,
            })
        );
    }

    #[test]
    fn runtime_event_hub_prefers_dropping_transient_agent_stream_frames() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        runtime.publish_record(&durable_agent_stream_record(
            &session_id,
            1,
            AgentStreamFrame::ToolCallStarted {
                tool_name: "shell".to_string(),
                input: "{}".to_string(),
            },
        ));
        for sequence in 2..=(MAX_EVENT_BACKLOG_DEPTH as u64 + 32) {
            runtime.publish_record(&transient_agent_stream_record(&session_id, sequence));
        }
        let final_sequence = MAX_EVENT_BACKLOG_DEPTH as u64 + 33;
        runtime.publish_record(&durable_agent_stream_record(
            &session_id,
            final_sequence,
            AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed,
            },
        ));

        let subscription = runtime.subscribe_events(
            &session_id,
            &[DaemonEventKind::AgentStream],
            Some(1),
            Some(&DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id: session_id.clone(),
                sequence: 1,
            }),
        );

        assert!(!subscription.has_gap);
        assert!(subscription.backlog.iter().any(|record| {
            matches!(
                record.event,
                DaemonEvent::AgentStream(AgentStreamEvent {
                    emission: StreamEmission {
                        frame: AgentStreamFrame::ToolCallCompleted { .. },
                        ..
                    },
                    ..
                })
            )
        }));
        assert!(
            subscription
                .backlog
                .iter()
                .all(|record| record.sequence > 1)
        );
    }

    #[test]
    fn runtime_event_hub_marks_gap_once_non_droppable_backlog_edges_evicted() {
        let runtime = runtime();
        let session_id = SessionId::new("session-1").expect("session id");
        for sequence in 1..=(MAX_EVENT_BACKLOG_DEPTH as u64 + 1) {
            runtime.publish_record(&run_record(&session_id, sequence));
        }

        let subscription = runtime.subscribe_events(
            &session_id,
            &[DaemonEventKind::Run],
            None,
            Some(&DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id: session_id.clone(),
                sequence: 0,
            }),
        );

        assert!(subscription.has_gap);
        assert!(subscription.backlog.is_empty());
        assert_eq!(subscription.latest_cursor, None);
    }

    #[test]
    fn runtime_event_hub_filters_live_events_by_session_when_requested() {
        let runtime = runtime();
        let session_a = SessionId::new("session-a").expect("session id");
        let session_b = SessionId::new("session-b").expect("session id");
        let subscription =
            runtime.subscribe_events(&session_a, &[DaemonEventKind::Run], None, None);

        runtime.publish_record(&run_record(&session_b, 1));
        let published = runtime.publish_record(&run_record(&session_a, 2));

        let received = subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("subscriber should receive only matching session event");

        assert_eq!(received, published);
    }

    #[test]
    fn runtime_event_hub_marks_and_removes_overflowed_subscriber() {
        let runtime = runtime();
        let session_id = SessionId::new("session-overflow").expect("session id");
        let subscription =
            runtime.subscribe_events(&session_id, &[DaemonEventKind::Run], None, None);

        for sequence in 1..=(MAX_SUBSCRIBER_QUEUE_DEPTH as u64 + 1) {
            runtime.publish_record(&run_record(&session_id, sequence));
        }

        assert!(subscription.overflowed.load(Ordering::SeqCst));

        let received = subscription.receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(received.len(), MAX_SUBSCRIBER_QUEUE_DEPTH);
        assert_eq!(received.first().map(|event| event.sequence), Some(1));
        assert_eq!(
            received.last().map(|event| event.sequence),
            Some(MAX_SUBSCRIBER_QUEUE_DEPTH as u64)
        );

        runtime.publish_record(&run_record(
            &session_id,
            MAX_SUBSCRIBER_QUEUE_DEPTH as u64 + 2,
        ));

        assert!(matches!(
            subscription
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn runtime_event_hub_uses_persisted_latest_when_live_hub_is_empty() {
        let runtime = runtime();
        let session_id = SessionId::new("session-a").expect("session id");

        let subscription =
            runtime.subscribe_events(&session_id, &[DaemonEventKind::Run], Some(99), None);

        assert_eq!(
            subscription.latest_cursor,
            Some(DaemonEventCursor {
                daemon_instance_id: runtime.daemon_instance_id(),
                session_id,
                sequence: 99,
            })
        );
        assert_eq!(
            runtime.latest_cursor_for_session(&SessionId::new("session-a").expect("session id")),
            None
        );
    }

    #[test]
    #[should_panic(expected = "subscriber id space exhausted")]
    fn runtime_event_hub_panics_when_subscriber_id_space_is_exhausted() {
        let hub = RuntimeEventHub::new();
        {
            let mut inner = hub
                .inner
                .lock()
                .expect("runtime event hub should not be poisoned");
            inner.next_subscriber_id = u64::MAX;
        }
        let session_id = SessionId::new("session-overflow").expect("session id");

        let _ = hub.subscribe(
            "daemon-test",
            &session_id,
            &[DaemonEventKind::Run],
            None,
            None,
        );
    }
}
