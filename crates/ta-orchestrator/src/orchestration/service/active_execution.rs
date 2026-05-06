use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use taugentic_agent::ExecutionHandle;

use crate::{RunId, SessionId};

#[derive(Debug, Clone)]
pub(super) struct ActiveExecutionOwner {
    inner: Arc<ActiveExecutionStore>,
}

#[derive(Debug)]
struct ActiveExecutionStore {
    entries: Mutex<BTreeMap<RunId, ActiveExecution>>,
    handle_changed: Condvar,
}

pub(super) struct ActiveExecution {
    pub(super) session_id: SessionId,
    handle: Option<Arc<dyn ExecutionHandle>>,
    cancel_requested: bool,
    cancel_delivered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachHandleDisposition {
    Attached,
    CancelRequested,
}

impl Clone for ActiveExecution {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            handle: self.handle.clone(),
            cancel_requested: self.cancel_requested,
            cancel_delivered: self.cancel_delivered,
        }
    }
}

impl std::fmt::Debug for ActiveExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveExecution")
            .field("session_id", &self.session_id)
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
            }),
        }
    }

    pub(super) fn claim_run(&self, run_id: RunId, session_id: SessionId) {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .insert(
                run_id,
                ActiveExecution {
                    session_id,
                    handle: None,
                    cancel_requested: false,
                    cancel_delivered: false,
                },
            );
    }

    pub(super) fn attach_handle(
        &self,
        run_id: &RunId,
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

    pub(super) fn is_running_owned_by(&self, run_id: &RunId, session_id: &SessionId) -> bool {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .get(run_id)
            .is_some_and(|execution| execution.session_id == *session_id)
    }

    pub(super) fn release_run(&self, run_id: &RunId) -> bool {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .remove(run_id)
            .is_some()
    }

    pub(super) fn active_count(&self) -> usize {
        self.inner
            .entries
            .lock()
            .expect("active run owner should not be poisoned")
            .len()
    }

    pub(super) fn cancel_run(&self, run_id: &RunId, session_id: &SessionId) -> Result<(), String> {
        const PENDING_HANDLE_WAIT: Duration = Duration::from_secs(5);
        let deadline = Instant::now() + PENDING_HANDLE_WAIT;
        let handle = {
            let mut inner = self
                .inner
                .entries
                .lock()
                .expect("active run owner should not be poisoned");
            loop {
                let Some(execution) = inner.get_mut(run_id) else {
                    return Err(format!(
                        "active execution missing while cancelling {}",
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
                execution.cancel_requested = true;
                if let Some(handle) = execution.handle.clone() {
                    if execution.cancel_delivered {
                        break None;
                    }
                    execution.cancel_delivered = true;
                    break Some(handle);
                }
                let now = Instant::now();
                if now >= deadline {
                    break None;
                }
                let wait = deadline.saturating_duration_since(now);
                let (next_inner, _) = self
                    .inner
                    .handle_changed
                    .wait_timeout(inner, wait)
                    .expect("active run owner should not be poisoned");
                inner = next_inner;
            }
        };

        if let Some(handle) = handle {
            handle
                .cancel()
                .map_err(|error| format!("failed to cancel run {}: {error}", run_id.as_str()))?;
        }

        Ok(())
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
