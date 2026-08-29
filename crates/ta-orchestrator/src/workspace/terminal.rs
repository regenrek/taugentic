use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ta_exec::{LocalPtyEngine, PtyRequest, PtySession, PtySize};
use ta_protocol::wire::{
    ProjectId, TERMINAL_OUTPUT_CHUNK_MAX_BYTES, TERMINAL_SNAPSHOT_MAX_BYTES, TerminalAttachResult,
    TerminalEventParams, TerminalSessionId, TerminalSessionStatus, TerminalSessionSummary,
    TerminalStreamEvent, Workspace, WorkspaceId,
};
use uuid::Uuid;

use crate::orchestration::AppServiceError;

const TERMINAL_SUBSCRIBER_QUEUE_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct TerminalRuntime {
    inner: Arc<Mutex<TerminalRuntimeInner>>,
}

impl std::fmt::Debug for TerminalRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalRuntime")
            .finish_non_exhaustive()
    }
}

struct TerminalRuntimeInner {
    next_subscriber_id: u64,
    terminals: HashMap<TerminalSessionId, TerminalRecord>,
}

struct TerminalRecord {
    owner_principal_id: String,
    summary: TerminalSessionSummary,
    pty: Arc<PtySession>,
    chunks: VecDeque<TerminalOutputChunk>,
    stored_bytes: usize,
    snapshot_truncated: bool,
    latest_sequence: u64,
    subscribers: Vec<TerminalSubscriber>,
}

#[derive(Clone)]
struct TerminalOutputChunk {
    sequence: u64,
    bytes: Arc<[u8]>,
}

struct TerminalSubscriber {
    id: u64,
    connection_id: usize,
    sender: SyncSender<TerminalRuntimeEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalRuntimeEvent {
    Output { sequence: u64, bytes: Arc<[u8]> },
    Exited,
}

pub(crate) struct TerminalRuntimeSubscription {
    pub(crate) result: TerminalAttachResult,
    pub(crate) receiver: Receiver<TerminalRuntimeEvent>,
    _cleanup: TerminalSubscriptionCleanup,
}

struct TerminalSubscriptionCleanup {
    runtime: Weak<Mutex<TerminalRuntimeInner>>,
    terminal_id: TerminalSessionId,
    subscriber_id: u64,
}

impl Drop for TerminalSubscriptionCleanup {
    fn drop(&mut self) {
        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        let Ok(mut runtime) = runtime.lock() else {
            return;
        };
        let Some(record) = runtime.terminals.get_mut(&self.terminal_id) else {
            return;
        };
        record
            .subscribers
            .retain(|subscriber| subscriber.id != self.subscriber_id);
    }
}

impl Default for TerminalRuntime {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TerminalRuntimeInner {
                next_subscriber_id: 1,
                terminals: HashMap::new(),
            })),
        }
    }
}

impl TerminalRuntime {
    pub(crate) fn spawn(
        &self,
        owner_principal_id: String,
        project_id: ProjectId,
        workspace: &Workspace,
        rows: u16,
        cols: u16,
        sandbox_profile: ta_exec::SandboxProfile,
    ) -> Result<TerminalSessionSummary, AppServiceError> {
        let id = TerminalSessionId::new(format!("terminal-{}", Uuid::new_v4().simple()))
            .expect("generated terminal id should be valid");
        let request =
            PtyRequest::default_shell(workspace.root_realpath.as_path(), PtySize::new(rows, cols))
                .env("TERM", "xterm-256color")
                .env("COLORTERM", "truecolor")
                .sandbox_profile(sandbox_profile);
        let (pty, reader) = LocalPtyEngine
            .spawn(request)
            .map_err(|error| AppServiceError::TerminalOperationFailed(error.to_string()))?;
        let summary = TerminalSessionSummary {
            id: id.clone(),
            project_id,
            workspace_id: workspace.id.clone(),
            status: TerminalSessionStatus::Running,
            rows,
            cols,
        };
        self.inner
            .lock()
            .expect("terminal runtime lock poisoned")
            .terminals
            .insert(
                id.clone(),
                TerminalRecord {
                    owner_principal_id,
                    summary: summary.clone(),
                    pty,
                    chunks: VecDeque::new(),
                    stored_bytes: 0,
                    snapshot_truncated: false,
                    latest_sequence: 0,
                    subscribers: Vec::new(),
                },
            );
        self.spawn_reader(id, reader)?;
        Ok(summary)
    }

    pub(crate) fn list(
        &self,
        owner_principal_id: &str,
        project_id: &ProjectId,
        workspace_id: &WorkspaceId,
    ) -> Vec<TerminalSessionSummary> {
        let runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let mut terminals = runtime
            .terminals
            .values()
            .filter(|record| {
                record.owner_principal_id == owner_principal_id
                    && record.summary.project_id == *project_id
                    && record.summary.workspace_id == *workspace_id
            })
            .map(|record| record.summary.clone())
            .collect::<Vec<_>>();
        terminals.sort_by(|left, right| left.id.cmp(&right.id));
        terminals
    }

    pub(crate) fn attach(
        &self,
        owner_principal_id: &str,
        terminal_id: &TerminalSessionId,
        connection_id: usize,
    ) -> Result<TerminalRuntimeSubscription, AppServiceError> {
        let (sender, receiver) = mpsc::sync_channel(TERMINAL_SUBSCRIBER_QUEUE_CAPACITY);
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let subscriber_id = runtime.next_subscriber_id;
        runtime.next_subscriber_id =
            runtime.next_subscriber_id.checked_add(1).ok_or_else(|| {
                AppServiceError::TerminalOperationFailed("subscriber id exhausted".to_string())
            })?;
        let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
        let mut snapshot = Vec::with_capacity(record.stored_bytes);
        for chunk in &record.chunks {
            snapshot.extend_from_slice(&chunk.bytes);
        }
        let result = TerminalAttachResult {
            terminal: record.summary.clone(),
            snapshot_base64: BASE64.encode(snapshot),
            snapshot_truncated: record.snapshot_truncated,
            latest_sequence: record.latest_sequence,
        };
        record
            .subscribers
            .retain(|subscriber| subscriber.connection_id != connection_id);
        record.subscribers.push(TerminalSubscriber {
            id: subscriber_id,
            connection_id,
            sender,
        });
        Ok(TerminalRuntimeSubscription {
            result,
            receiver,
            _cleanup: TerminalSubscriptionCleanup {
                runtime: Arc::downgrade(&self.inner),
                terminal_id: terminal_id.clone(),
                subscriber_id,
            },
        })
    }

    pub(crate) fn detach(
        &self,
        owner_principal_id: &str,
        terminal_id: &TerminalSessionId,
        connection_id: usize,
    ) -> Result<bool, AppServiceError> {
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
        let before = record.subscribers.len();
        record
            .subscribers
            .retain(|subscriber| subscriber.connection_id != connection_id);
        Ok(record.subscribers.len() != before)
    }

    pub(crate) fn input(
        &self,
        owner_principal_id: &str,
        terminal_id: &TerminalSessionId,
        bytes: &[u8],
    ) -> Result<(), AppServiceError> {
        let pty = {
            let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
            let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
            ensure_running(record)?;
            Arc::clone(&record.pty)
        };
        pty.write_input(bytes)
            .map_err(|error| AppServiceError::TerminalOperationFailed(error.to_string()))
    }

    pub(crate) fn resize(
        &self,
        owner_principal_id: &str,
        terminal_id: &TerminalSessionId,
        rows: u16,
        cols: u16,
    ) -> Result<TerminalSessionSummary, AppServiceError> {
        let pty = {
            let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
            let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
            ensure_running(record)?;
            Arc::clone(&record.pty)
        };
        pty.resize(PtySize::new(rows, cols))
            .map_err(|error| AppServiceError::TerminalOperationFailed(error.to_string()))?;
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
        record.summary.rows = rows;
        record.summary.cols = cols;
        Ok(record.summary.clone())
    }

    pub(crate) fn close(
        &self,
        owner_principal_id: &str,
        terminal_id: &TerminalSessionId,
    ) -> Result<TerminalSessionSummary, AppServiceError> {
        let pty = {
            let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
            let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
            Arc::clone(&record.pty)
        };
        pty.close()
            .map_err(|error| AppServiceError::TerminalOperationFailed(error.to_string()))?;
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let record = owned_terminal_mut(&mut runtime, owner_principal_id, terminal_id)?;
        mark_exited(record);
        Ok(record.summary.clone())
    }

    fn spawn_reader(
        &self,
        terminal_id: TerminalSessionId,
        mut reader: Box<dyn Read + Send>,
    ) -> Result<(), AppServiceError> {
        let runtime = self.clone();
        thread::Builder::new()
            .name(format!("terminal-reader-{}", terminal_id.as_str()))
            .spawn(move || {
                let mut buffer = vec![0_u8; TERMINAL_OUTPUT_CHUNK_MAX_BYTES];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => {
                            runtime.exit_from_reader(&terminal_id);
                            return;
                        }
                        Ok(read) => runtime.publish_output(&terminal_id, &buffer[..read]),
                    }
                }
            })
            .map(|_| ())
            .map_err(|error| AppServiceError::TerminalOperationFailed(error.to_string()))
    }

    fn publish_output(&self, terminal_id: &TerminalSessionId, bytes: &[u8]) {
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let Some(record) = runtime.terminals.get_mut(terminal_id) else {
            return;
        };
        record.latest_sequence = record.latest_sequence.saturating_add(1);
        let chunk = TerminalOutputChunk {
            sequence: record.latest_sequence,
            bytes: Arc::from(bytes),
        };
        record.stored_bytes = record.stored_bytes.saturating_add(chunk.bytes.len());
        record.chunks.push_back(chunk.clone());
        while record.stored_bytes > TERMINAL_SNAPSHOT_MAX_BYTES {
            let Some(removed) = record.chunks.pop_front() else {
                break;
            };
            record.stored_bytes = record.stored_bytes.saturating_sub(removed.bytes.len());
            record.snapshot_truncated = true;
        }
        let event = TerminalRuntimeEvent::Output {
            sequence: chunk.sequence,
            bytes: chunk.bytes,
        };
        record.subscribers.retain(
            |subscriber| match subscriber.sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            },
        );
    }

    fn exit_from_reader(&self, terminal_id: &TerminalSessionId) {
        let mut runtime = self.inner.lock().expect("terminal runtime lock poisoned");
        let Some(record) = runtime.terminals.get_mut(terminal_id) else {
            return;
        };
        mark_exited(record);
    }
}

fn owned_terminal_mut<'a>(
    runtime: &'a mut TerminalRuntimeInner,
    owner_principal_id: &str,
    terminal_id: &TerminalSessionId,
) -> Result<&'a mut TerminalRecord, AppServiceError> {
    let record = runtime
        .terminals
        .get_mut(terminal_id)
        .ok_or_else(|| AppServiceError::TerminalNotFound(terminal_id.as_str().to_string()))?;
    if record.owner_principal_id != owner_principal_id {
        return Err(AppServiceError::TerminalNotFound(
            terminal_id.as_str().to_string(),
        ));
    }
    Ok(record)
}

fn ensure_running(record: &TerminalRecord) -> Result<(), AppServiceError> {
    if record.summary.status == TerminalSessionStatus::Running {
        Ok(())
    } else {
        Err(AppServiceError::TerminalNotRunning(
            record.summary.id.as_str().to_string(),
        ))
    }
}

fn mark_exited(record: &mut TerminalRecord) {
    if record.summary.status == TerminalSessionStatus::Exited {
        return;
    }
    record.summary.status = TerminalSessionStatus::Exited;
    record.subscribers.retain(|subscriber| {
        subscriber
            .sender
            .try_send(TerminalRuntimeEvent::Exited)
            .is_ok()
    });
}

pub(crate) fn protocol_event(
    terminal_id: TerminalSessionId,
    event: TerminalRuntimeEvent,
) -> TerminalEventParams {
    TerminalEventParams {
        terminal_id,
        event: match event {
            TerminalRuntimeEvent::Output { sequence, bytes } => TerminalStreamEvent::Output {
                sequence,
                data_base64: BASE64.encode(bytes),
            },
            TerminalRuntimeEvent::Exited => TerminalStreamEvent::Exited,
        },
    }
}
