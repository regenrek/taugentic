use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};

use taugentic_agent::ExecutionHandle;

use crate::{RunId, SessionId};

#[derive(Debug, Clone)]
pub(crate) struct ActiveExecutionOwner {
    inner: Arc<ActiveExecutionStore>,
}

#[derive(Debug)]
struct ActiveExecutionStore {
    entries: Mutex<BTreeMap<RunId, ActiveExecution>>,
    handle_changed: Condvar,
    next_generation: Mutex<u64>,
}

pub(super) struct ActiveExecution {
    pub(super) session_id: SessionId,
    pub(super) generation: u64,
    handle: Option<Arc<dyn ExecutionHandle>>,
    voice: Option<Arc<dyn crate::orchestration::voice::VoiceFrameExchange>>,
    cancel_requested: bool,
    cancel_delivered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachHandleDisposition {
    Attached,
    CancelRequested,
}

enum TerminalLeaseTarget {
    Current,
    Exact(u64),
}

impl Clone for ActiveExecution {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            generation: self.generation,
            handle: self.handle.clone(),
            voice: self.voice.clone(),
            cancel_requested: self.cancel_requested,
            cancel_delivered: self.cancel_delivered,
        }
    }
}

impl std::fmt::Debug for ActiveExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveExecution")
            .field("session_id", &self.session_id)
            .field("generation", &self.generation)
            .field("has_handle", &self.handle.is_some())
            .field("cancel_requested", &self.cancel_requested)
            .field("cancel_delivered", &self.cancel_delivered)
            .finish()
    }
}

impl ActiveExecutionOwner {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(ActiveExecutionStore {
                entries: Mutex::new(BTreeMap::new()),
                handle_changed: Condvar::new(),
                next_generation: Mutex::new(0),
            }),
        }
    }

    fn allocate_generation(&self) -> u64 {
        let mut next = self
            .inner
            .next_generation
            .lock()
            .expect("active generation should not be poisoned");
        *next = next
            .checked_add(1)
            .expect("active execution generation overflow");
        *next
    }

    pub(super) fn claim_run(&self, run_id: RunId, session_id: SessionId) -> u64 {
        let generation = self.allocate_generation();
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .insert(
                run_id,
                ActiveExecution {
                    session_id,
                    generation,
                    handle: None,
                    voice: None,
                    cancel_requested: false,
                    cancel_delivered: false,
                },
            );
        generation
    }

    /// Retain exclusive ownership while the replacement route transition is
    /// committed. The old generation and provider handle remain current until
    /// that durable action succeeds; only then is the old handle returned for
    /// cancellation after the lease has been released.
    pub(super) fn replace_run_with_generation_lease<T, E>(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        action: impl FnOnce(u64) -> Result<T, E>,
    ) -> Result<(Result<T, E>, u64, Option<Arc<dyn ExecutionHandle>>), String> {
        let generation = self.allocate_generation();
        let mut entries = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned");
        let Some(execution) = entries.get_mut(run_id) else {
            return Err(format!(
                "active execution missing while replacing {}",
                run_id.as_str()
            ));
        };
        if execution.session_id != *session_id {
            return Err(format!(
                "run {} is not owned by session {}",
                run_id.as_str(),
                session_id.as_str()
            ));
        }
        let result = action(generation);
        let old_handle = if result.is_ok() {
            execution.generation = generation;
            execution.cancel_requested = false;
            execution.cancel_delivered = false;
            execution.handle.take()
        } else {
            None
        };
        drop(entries);
        Ok((result, generation, old_handle))
    }

    /// Runs one durable mutation while the exact provider generation remains
    /// exclusively authorized.  Callers may take the store lock in `action`,
    /// but must never call provider code there: the owner lock is deliberately
    /// retained until the durable mutation has completed.
    pub(super) fn with_generation_lease<T, E>(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        generation: u64,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, String> {
        let entries = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned");
        let Some(execution) = entries.get(run_id) else {
            return Err(format!(
                "active execution missing while leasing {}",
                run_id.as_str()
            ));
        };
        if execution.session_id != *session_id || execution.generation != generation {
            return Err(format!(
                "stale execution generation while leasing {}",
                run_id.as_str()
            ));
        }
        // Keep `entries` alive through action. The only permitted nested lock
        // is the store lock, establishing the sole owner-then-store order.
        let result = action();
        drop(entries);
        Ok(result)
    }

    /// Completes a terminal durable mutation and retires exactly the
    /// generation that authorized it. The owner lock stays held through the
    /// supplied store work, then the matching slot is removed before the
    /// lease is released. This prevents an older callback from releasing a
    /// replacement execution by RunId alone.
    pub(super) fn with_current_terminal_generation_lease_and_take_handle<T, E>(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        action: impl FnOnce(u64) -> Result<T, E>,
    ) -> Result<(Result<T, E>, Option<Arc<dyn ExecutionHandle>>), String> {
        self.with_terminal_target_lease_and_take_handle(
            run_id,
            session_id,
            TerminalLeaseTarget::Current,
            action,
        )
    }

    /// Same terminal lease, but transfers the retired provider handle to the
    /// caller after the owner lock has been released. The caller may then
    /// cancel it without ever executing provider code under the lease.
    pub(super) fn with_terminal_generation_lease_and_take_handle<T, E>(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        generation: u64,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<(Result<T, E>, Option<Arc<dyn ExecutionHandle>>), String> {
        self.with_terminal_target_lease_and_take_handle(
            run_id,
            session_id,
            TerminalLeaseTarget::Exact(generation),
            |_| action(),
        )
    }

    fn with_terminal_target_lease_and_take_handle<T, E>(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        target: TerminalLeaseTarget,
        action: impl FnOnce(u64) -> Result<T, E>,
    ) -> Result<(Result<T, E>, Option<Arc<dyn ExecutionHandle>>), String> {
        let mut entries = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned");
        let Some(execution) = entries.get(run_id) else {
            return Err(format!(
                "active execution missing while leasing terminal {}",
                run_id.as_str()
            ));
        };
        let generation = execution.generation;
        if execution.session_id != *session_id
            || matches!(target, TerminalLeaseTarget::Exact(expected) if generation != expected)
        {
            return Err(format!(
                "stale execution generation while leasing terminal {}",
                run_id.as_str()
            ));
        }
        let result = action(generation);
        let handle = if result.is_ok() {
            entries
                .remove(run_id)
                .and_then(|execution| execution.handle)
        } else {
            None
        };
        drop(entries);
        Ok((result, handle))
    }

    pub(super) fn attach_handle(
        &self,
        run_id: &RunId,
        generation: u64,
        handle: Arc<dyn ExecutionHandle>,
    ) -> Result<AttachHandleDisposition, String> {
        let pending_cancel = {
            let mut inner = self
                .inner
                .entries
                .lock()
                .expect("active run owner should not be poisoned");
            let Some(execution) = inner.get_mut(run_id) else {
                return Err(format!(
                    "active execution missing while attaching handle for {}",
                    run_id.as_str(),
                ));
            };
            if execution.generation != generation {
                return Err(format!(
                    "stale execution generation while attaching handle for {}",
                    run_id.as_str()
                ));
            }
            let pending_cancel = execution.cancel_requested && !execution.cancel_delivered;
            if pending_cancel {
                execution.cancel_delivered = true;
            }
            execution.handle = Some(handle.clone());
            self.inner.handle_changed.notify_all();
            pending_cancel
        };

        if pending_cancel {
            handle
                .cancel()
                .map_err(|error| format!("failed to cancel run {}: {error}", run_id.as_str()))?;
            return Ok(AttachHandleDisposition::CancelRequested);
        }

        Ok(AttachHandleDisposition::Attached)
    }

    pub(crate) fn attach_voice_handle(
        &self,
        run_id: &RunId,
        generation: u64,
        handle: Arc<dyn ExecutionHandle>,
        voice: Arc<dyn crate::orchestration::voice::VoiceFrameExchange>,
    ) -> Result<AttachHandleDisposition, String> {
        let disposition = self.attach_handle(run_id, generation, handle)?;
        let mut entries = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned");
        let execution = entries.get_mut(run_id).ok_or_else(|| {
            format!(
                "active execution missing while attaching voice for {}",
                run_id.as_str()
            )
        })?;
        if execution.generation != generation {
            return Err(format!(
                "stale execution generation while attaching voice for {}",
                run_id.as_str()
            ));
        }
        execution.voice = Some(voice);
        Ok(disposition)
    }

    pub(crate) fn is_voice_run(&self, run_id: &RunId) -> bool {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .is_some_and(|execution| execution.voice.is_some())
    }

    pub(crate) fn exchange_voice_frame(
        &self,
        run_id: &RunId,
        input: [u8; ta_protocol::wire::VOICE_FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<crate::orchestration::voice::VoiceExchange, String> {
        let voice = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .and_then(|execution| execution.voice.clone())
            .ok_or_else(|| format!("active voice execution missing for {}", run_id.as_str()))?;
        voice.exchange(input, playback_completed_frames)
    }

    pub(crate) fn end_voice(
        &self,
        run_id: &RunId,
        reason: ta_protocol::wire::VoiceStreamEndReason,
    ) -> Result<(), String> {
        let voice = self
            .inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .and_then(|execution| execution.voice.clone())
            .ok_or_else(|| format!("active voice execution missing for {}", run_id.as_str()))?;
        voice.end(reason)
    }

    pub(super) fn is_running_owned_by(&self, run_id: &RunId, session_id: &SessionId) -> bool {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .is_some_and(|execution| execution.session_id == *session_id)
    }

    pub(super) fn active_count(&self) -> usize {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .len()
    }

    #[cfg(test)]
    pub(super) fn current_generation_for_tests(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
    ) -> Option<u64> {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .filter(|execution| execution.session_id == *session_id)
            .map(|execution| execution.generation)
    }

    #[cfg(test)]
    pub(super) fn is_current_generation(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        generation: u64,
    ) -> bool {
        self.current_generation_for_tests(run_id, session_id) == Some(generation)
    }

    pub(super) fn resolve_approval(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        resolution: ta_protocol::wire::ApprovalResolution,
    ) -> Result<(), String> {
        let handle = {
            let inner = self
                .inner
                .entries
                .lock()
                .expect("active run owner should not be poisoned");
            let Some(execution) = inner.get(run_id) else {
                return Err(format!(
                    "active execution missing while resolving approval for {}",
                    run_id.as_str(),
                ));
            };
            if execution.session_id != *session_id {
                return Err(format!(
                    "run {} is not owned by session {}",
                    run_id.as_str(),
                    session_id.as_str(),
                ));
            }
            execution.handle.clone().ok_or_else(|| {
                format!(
                    "active execution handle missing while resolving approval for {}",
                    run_id.as_str()
                )
            })?
        };
        handle.resolve_approval(resolution).map_err(|error| {
            format!(
                "failed to resolve approval for {}: {error}",
                run_id.as_str()
            )
        })
    }

    #[cfg(test)]
    pub(super) fn execution_for(&self, run_id: &RunId) -> Option<ActiveExecution> {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .cloned()
    }

    #[cfg(test)]
    pub(super) fn handle_for_tests(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
    ) -> Option<Arc<dyn ExecutionHandle>> {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .filter(|execution| execution.session_id == *session_id)
            .and_then(|execution| execution.handle.clone())
    }
}

impl Drop for ActiveExecutionStore {
    fn drop(&mut self) {
        let handles = {
            let inner = self
                .entries
                .lock()
                .expect("active run owner should not be poisoned");
            inner
                .values()
                .filter_map(|execution| execution.handle.clone())
                .collect::<Vec<_>>()
        };
        for handle in handles {
            let _ = handle.cancel();
        }
    }
}
