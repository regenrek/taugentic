use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, ApprovalActor, ApprovalDecision, ApprovalId,
    ApprovalRequest, ApprovalResolution, ApprovalResolutionReason, ApprovalScope, ApprovalTarget,
    RunId, RuntimeLanePendingState, StreamEmission,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument, warn};
use uuid::Uuid;

use crate::approval::{ApprovalDescriptor, ApprovalOutcome};
use crate::session::{ApprovalStatus, PendingApproval};
use crate::{ExecutionError, ExecutionSink};

pub struct ApprovalBridge {
    run_id: RunId,
    sink: Arc<dyn ExecutionSink>,
    pending: Arc<Mutex<BTreeMap<ApprovalId, PendingApprovalEntry>>>,
    cancellation: CancellationToken,
}

struct PendingApprovalEntry {
    descriptor: ApprovalDescriptor,
    sender: watch::Sender<Option<ApprovalOutcome>>,
    outcome: Option<ApprovalOutcome>,
}

impl ApprovalBridge {
    pub fn new(
        run_id: RunId,
        sink: Arc<dyn ExecutionSink>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            run_id,
            sink,
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            cancellation,
        }
    }

    #[instrument(skip(self, descriptor), fields(run_id = %self.run_id.as_str(), tool_call_id = %descriptor.call_id, tool = %descriptor.tool_name, scope = ?scope))]
    pub fn request(
        &self,
        scope: ApprovalScope,
        descriptor: &ApprovalDescriptor,
    ) -> Result<ApprovalId, ExecutionError> {
        let tool_call_id = item_id(&descriptor.call_id)?;
        let id = ApprovalId::new(format!("approval-{}", Uuid::new_v4().simple()))
            .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?;
        let requested_at_ms = current_time_ms();
        let ttl = ta_policy::ApprovalTtlPolicy::default();
        let request = ApprovalRequest::new(
            id.clone(),
            self.run_id.clone(),
            scope,
            requested_at_ms,
            ttl.expires_at_ms(requested_at_ms),
            ApprovalTarget::ToolCall {
                tool_name: descriptor.tool_name.clone(),
            },
            descriptor.reason.clone(),
        )
        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?
        .with_tool_call_id(tool_call_id.clone());
        let (sender, _receiver) = watch::channel(None);

        self.insert_pending(id.clone(), descriptor.clone(), sender)?;
        if let Err(error) = self.sink.request_approval(request) {
            self.remove_pending(&id)?;
            return Err(error);
        }
        if let Err(error) = self.emit_waiting_for_approval(tool_call_id) {
            self.remove_pending(&id)?;
            return Err(error);
        }

        Ok(id)
    }

    #[instrument(skip(self), fields(approval_id = %id.as_str(), run_id = %self.run_id.as_str()))]
    pub async fn wait(&self, id: ApprovalId) -> Result<ApprovalOutcome, ExecutionError> {
        let mut receiver = self.receiver(&id)?;
        loop {
            if let Some(outcome) = *receiver.borrow() {
                return Ok(outcome);
            }

            tokio::select! {
                changed = receiver.changed() => {
                    changed.map_err(|_| {
                        ExecutionError::Cancelled(format!(
                            "approval {} sender dropped",
                            id.as_str()
                        ))
                    })?;
                }
                () = self.cancellation.cancelled() => {
                    self.resolve(id.clone(), ApprovalOutcome::TurnInterrupted);
                    return Err(ExecutionError::Cancelled("turn_interrupted".to_string()));
                }
            }
        }
    }

    #[instrument(skip(self), fields(approval_id = %id.as_str(), run_id = %self.run_id.as_str(), outcome = ?outcome))]
    pub fn resolve(&self, id: ApprovalId, outcome: ApprovalOutcome) {
        let changed = match self.resolve_once(&id, outcome, ResolutionEmission::Sink) {
            Ok(changed) => changed,
            Err(error) => {
                error!(approval_id = %id.as_str(), error = %error, "approval resolve failed");
                return;
            }
        };
        let Some(sender) = changed else {
            return;
        };

        let _ = sender.send_replace(Some(outcome));
    }

    #[instrument(skip(self, resolution), fields(approval_id = %resolution.approval_id.as_str(), run_id = %self.run_id.as_str(), decision = ?resolution.decision))]
    pub fn resolve_from_runtime(
        &self,
        resolution: ApprovalResolution,
    ) -> Result<(), ExecutionError> {
        if resolution.run_id != self.run_id {
            return Err(ExecutionError::ProcessFailed(format!(
                "approval resolution run mismatch: {} != {}",
                resolution.run_id.as_str(),
                self.run_id.as_str()
            )));
        }
        let id = resolution.approval_id;
        let outcome = match resolution.decision {
            ApprovalDecision::Approved => ApprovalOutcome::Allow,
            ApprovalDecision::Rejected => ApprovalOutcome::Deny,
        };
        let Some(sender) = self.resolve_once(&id, outcome, ResolutionEmission::AlreadyPublished)?
        else {
            return Ok(());
        };
        let _ = sender.send_replace(Some(outcome));
        Ok(())
    }

    #[instrument(skip(self), fields(run_id = %self.run_id.as_str(), reason))]
    pub fn reject_all(&self, reason: &'static str) {
        let pending = match self.pending_ids() {
            Ok(pending) => pending,
            Err(error) => {
                error!(error = %error, "pending approval rejection failed");
                return;
            }
        };
        let count = pending.len();
        for id in pending {
            self.resolve(id, ApprovalOutcome::TurnInterrupted);
        }
        if count == 0 {
            return;
        }
        tracing::debug!(reason, approvals = count, "pending approvals rejected");
    }

    pub fn pending_approvals(&self) -> Result<Vec<PendingApproval>, ExecutionError> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        Ok(pending
            .iter()
            .map(|(id, entry)| PendingApproval {
                id: id.as_str().to_string(),
                reason: entry.descriptor.reason.clone(),
                status: match entry.outcome {
                    None => ApprovalStatus::Pending,
                    Some(ApprovalOutcome::Allow) => ApprovalStatus::Allowed,
                    Some(outcome) => ApprovalStatus::Rejected {
                        reason: outcome
                            .rejection_reason()
                            .unwrap_or("approval_rejected")
                            .to_string(),
                    },
                },
            })
            .collect())
    }

    fn insert_pending(
        &self,
        id: ApprovalId,
        descriptor: ApprovalDescriptor,
        sender: watch::Sender<Option<ApprovalOutcome>>,
    ) -> Result<(), ExecutionError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        pending.insert(
            id,
            PendingApprovalEntry {
                descriptor,
                sender,
                outcome: None,
            },
        );
        Ok(())
    }

    fn remove_pending(&self, id: &ApprovalId) -> Result<(), ExecutionError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        pending.remove(id);
        Ok(())
    }

    fn receiver(
        &self,
        id: &ApprovalId,
    ) -> Result<watch::Receiver<Option<ApprovalOutcome>>, ExecutionError> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        pending
            .get(id)
            .map(|entry| entry.sender.subscribe())
            .ok_or_else(|| {
                ExecutionError::Cancelled(format!("approval {} is not pending", id.as_str()))
            })
    }

    fn pending_ids(&self) -> Result<Vec<ApprovalId>, ExecutionError> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        Ok(pending
            .iter()
            .filter(|(_, entry)| entry.outcome.is_none())
            .map(|(id, _)| id.clone())
            .collect())
    }

    fn resolve_once(
        &self,
        id: &ApprovalId,
        outcome: ApprovalOutcome,
        emission: ResolutionEmission,
    ) -> Result<Option<watch::Sender<Option<ApprovalOutcome>>>, ExecutionError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("approval lock poisoned".to_string()))?;
        let Some(entry) = pending.get_mut(id) else {
            warn!(approval_id = %id.as_str(), "approval resolution ignored because id is unknown");
            return Ok(None);
        };
        match entry.outcome {
            Some(existing) if existing == outcome => Ok(None),
            Some(existing) => {
                error!(
                    approval_id = %id.as_str(),
                    existing = ?existing,
                    attempted = ?outcome,
                    "approval resolution conflict ignored"
                );
                Ok(None)
            }
            None => {
                let tool_call_id = item_id(&entry.descriptor.call_id)?;
                if emission == ResolutionEmission::Sink {
                    self.emit_resolution(id.clone(), tool_call_id, outcome)?;
                }
                entry.outcome = Some(outcome);
                Ok(Some(entry.sender.clone()))
            }
        }
    }

    fn emit_waiting_for_approval(
        &self,
        tool_call_id: AgentStreamItemId,
    ) -> Result<(), ExecutionError> {
        self.sink.push_stream(StreamEmission {
            turn_id: None,
            item_id: Some(tool_call_id),
            fragment_sequence: None,
            frame: AgentStreamFrame::PendingStateChanged {
                state: RuntimeLanePendingState::WaitingForApproval,
            },
        })
    }

    fn emit_resolution(
        &self,
        id: ApprovalId,
        tool_call_id: AgentStreamItemId,
        outcome: ApprovalOutcome,
    ) -> Result<(), ExecutionError> {
        let actor = ApprovalActor::new("taugentic-agent")
            .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?;
        let resolution = ApprovalResolution::new(
            id,
            self.run_id.clone(),
            outcome.decision(),
            approval_resolution_reason(outcome),
            actor,
            Some(outcome.commentary().to_string()),
        )
        .with_tool_call_id(tool_call_id);
        self.sink.resolve_approval(resolution)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolutionEmission {
    Sink,
    AlreadyPublished,
}

fn item_id(id: &str) -> Result<AgentStreamItemId, ExecutionError> {
    AgentStreamItemId::new(id.to_string())
        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))
}

fn approval_resolution_reason(outcome: ApprovalOutcome) -> ApprovalResolutionReason {
    match outcome {
        ApprovalOutcome::Allow | ApprovalOutcome::Deny => ApprovalResolutionReason::User,
        ApprovalOutcome::TurnInterrupted => ApprovalResolutionReason::Cancelled,
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
