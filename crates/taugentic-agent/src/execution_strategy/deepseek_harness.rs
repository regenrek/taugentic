use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, ApprovalResolution, StreamEmission,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::approval::{ApprovalBridge, ApprovalDescriptor, ApprovalOutcome};
use crate::{ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink};

/// The sealed bridge asset is intentionally not resolved through PATH, Node,
/// or a checkout. M6 packaging supplies it in a later, separately audited
/// slice; until then the selected direct lane fails closed.
pub(crate) async fn dispatch(
    request: ExecutionRequest,
    _sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let model = request.model_id.as_ref().ok_or_else(|| {
        ExecutionError::InvalidConfig("DeepSeek Harness requires an explicit model".to_string())
    })?;
    ta_provider_dsh::DeepSeekModel::parse(model.as_str())
        .map_err(|error| ExecutionError::InvalidConfig(error.to_string()))?;
    Err(ExecutionError::Unsupported(format!(
        "sealed {} runtime asset is unavailable",
        ta_provider_dsh::DSH_RUNTIME_VERSION
    )))
}

/// Test-only injection seam for the later packaged sealed runtime. Production
/// dispatch above never discovers a local executable or PATH command.
#[doc(hidden)]
pub fn dispatch_with_runtime(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    runtime: ta_provider_dsh::SealedRuntime,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let model = request.model_id.as_ref().ok_or_else(|| {
        ExecutionError::InvalidConfig("DeepSeek Harness requires an explicit model".to_string())
    })?;
    let model = ta_provider_dsh::DeepSeekModel::parse(model.as_str()).map_err(map_error)?;
    let cancellation = CancellationToken::new();
    let approval_bridge = Arc::new(ApprovalBridge::new(
        request.run_id.clone(),
        Arc::clone(&sink),
        cancellation.clone(),
    ));
    let run_id = request.run_id.as_str().to_string();
    let bridge = Arc::clone(&approval_bridge);
    let cancelled = cancellation.clone();
    let event_sink = Arc::clone(&sink);
    let (control_sender, controls) = mpsc::unbounded_channel();
    let control_for_thread = control_sender.clone();
    let pending_approvals = Arc::new(Mutex::new(BTreeMap::new()));
    let pending_for_thread = Arc::clone(&pending_approvals);
    let thread = std::thread::Builder::new()
        .name(format!("taugentic-dsh-{run_id}"))
        .spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ta_provider_dsh::DshError::Process(error.to_string()))
                .and_then(|runtime_loop| {
                    runtime_loop.block_on(async move {
                        let mut supervisor = ta_provider_dsh::DshSupervisor::start(runtime).await?;
                        let result = run_turn(
                            &mut supervisor,
                            &request,
                            model,
                            cancelled,
                            bridge,
                            Arc::clone(&event_sink),
                            controls,
                            control_for_thread,
                            pending_for_thread,
                        )
                        .await;
                        if result.is_err() {
                            supervisor.force_reap().await;
                            result
                        } else {
                            supervisor.shutdown().await
                        }
                    })
                });
            match result {
                Ok(()) => {
                    let _ = sink.complete("DeepSeek Harness execution completed");
                }
                Err(error) => {
                    let _ = sink.fail(map_error(error));
                }
            }
        })
        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?;
    Ok(Arc::new(DeepSeekHarnessHandle {
        cancellation,
        thread: Mutex::new(Some(thread)),
        approval_bridge,
        controls: control_sender,
        pending_approvals,
        run_id,
    }))
}

struct DeepSeekHarnessHandle {
    cancellation: CancellationToken,
    thread: Mutex<Option<JoinHandle<()>>>,
    approval_bridge: Arc<ApprovalBridge>,
    controls: mpsc::UnboundedSender<ta_provider_dsh::BridgeControl>,
    pending_approvals: Arc<Mutex<BTreeMap<ta_protocol::wire::ApprovalId, PendingApproval>>>,
    run_id: String,
}
impl ExecutionHandle for DeepSeekHarnessHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.approval_bridge.reject_all("turn_interrupted");
        self.cancellation.cancel();
        self.controls
            .send(ta_provider_dsh::BridgeControl::Cancel {
                run_id: self.run_id.clone(),
            })
            .map_err(|_| ExecutionError::Cancelled("turn_interrupted".to_string()))?;
        Ok(())
    }
    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        if resolution.run_id.as_str() != self.run_id {
            return Err(ExecutionError::ProcessFailed(format!(
                "DSH approval resolution run mismatch: {} != {}",
                resolution.run_id.as_str(),
                self.run_id
            )));
        }
        let mut pending = self.pending_approvals.lock().map_err(|_| {
            ExecutionError::ProcessFailed("DSH approval correlation lock poisoned".to_string())
        })?;
        let Some(pending_approval) = pending.get_mut(&resolution.approval_id) else {
            return Err(ExecutionError::ProcessFailed(format!(
                "DSH approval resolution is unknown, duplicate, or late: {}",
                resolution.approval_id.as_str()
            )));
        };
        if pending_approval.resolution_seen {
            return Err(ExecutionError::ProcessFailed(format!(
                "DSH approval resolution is unknown, duplicate, or late: {}",
                resolution.approval_id.as_str()
            )));
        }
        pending_approval.resolution_seen = true;
        drop(pending);
        self.approval_bridge.resolve_from_runtime(resolution)
    }
}

async fn run_turn(
    supervisor: &mut ta_provider_dsh::DshSupervisor,
    request: &ExecutionRequest,
    model: ta_provider_dsh::DeepSeekModel,
    cancellation: CancellationToken,
    approval_bridge: Arc<ApprovalBridge>,
    sink: Arc<dyn ExecutionSink>,
    mut controls: mpsc::UnboundedReceiver<ta_provider_dsh::BridgeControl>,
    control_sender: mpsc::UnboundedSender<ta_provider_dsh::BridgeControl>,
    pending_approvals: Arc<Mutex<BTreeMap<ta_protocol::wire::ApprovalId, PendingApproval>>>,
) -> Result<(), ta_provider_dsh::DshError> {
    let cancel_sender = control_sender.clone();
    let cancellation_run_id = request.run_id.as_str().to_string();
    let manifest = request.dsh_tool_approval_manifest.clone();
    let cancellation_task = tokio::spawn(async move {
        cancellation.cancelled().await;
        let _ = cancel_sender.send(ta_provider_dsh::BridgeControl::Cancel {
            run_id: cancellation_run_id,
        });
    });
    let result = supervisor
        .run_turn(
            model,
            &request.objective,
            request.resume_provider_session_id.as_deref(),
            &mut controls,
            move |event| {
                let manifest = manifest.clone();
                let approval_bridge = Arc::clone(&approval_bridge);
                let sink = Arc::clone(&sink);
                let pending_approvals = Arc::clone(&pending_approvals);
                let control_sender = control_sender.clone();
                Box::pin(async move {
                    handle_event(
                        event,
                        manifest,
                        approval_bridge,
                        sink,
                        pending_approvals,
                        control_sender,
                    )
                    .await
                })
            },
        )
        .await;
    cancellation_task.abort();
    result
}

async fn handle_event(
    event: ta_provider_dsh::BridgeEvent,
    manifest: std::collections::BTreeMap<String, ta_protocol::wire::ApprovalScope>,
    approval_bridge: Arc<ApprovalBridge>,
    sink: Arc<dyn ExecutionSink>,
    pending_approvals: Arc<Mutex<BTreeMap<ta_protocol::wire::ApprovalId, PendingApproval>>>,
    control_sender: mpsc::UnboundedSender<ta_provider_dsh::BridgeControl>,
) -> Result<(), ta_provider_dsh::DshError> {
    match event {
        ta_provider_dsh::BridgeEvent::Approval {
            approval_id,
            call_id,
            tool_name,
        } => {
            let scope = manifest.get(&tool_name).copied().ok_or_else(|| {
                ta_provider_dsh::DshError::Protocol(format!("unapproved DSH tool: {tool_name}"))
            })?;
            let reservation = approval_bridge
                .reserve_request(
                    scope,
                    &ApprovalDescriptor::new(call_id, tool_name, "DSH tool call requires approval"),
                )
                .map_err(|error| ta_provider_dsh::DshError::Process(error.to_string()))?;
            let daemon_id = reservation.approval_id().clone();
            let inserted = {
                let mut pending = pending_approvals.lock().map_err(|_| {
                    ta_provider_dsh::DshError::Process(
                        "DSH approval correlation lock poisoned".to_string(),
                    )
                })?;
                pending
                    .insert(
                        daemon_id.clone(),
                        PendingApproval {
                            bridge_approval_id: approval_id,
                            resolution_seen: false,
                        },
                    )
                    .is_none()
            };
            if !inserted {
                return Err(ta_provider_dsh::DshError::Protocol(
                    "duplicate daemon approval id".to_string(),
                ));
            }
            // Reserve the canonical daemon id first so a synchronous approval
            // sink can correlate it immediately after publication. The map
            // lock is released before the external sink calls in publish().
            if let Err(error) = reservation.publish() {
                let mut pending = pending_approvals.lock().map_err(|_| {
                    ta_provider_dsh::DshError::Process(
                        "DSH approval correlation lock poisoned".to_string(),
                    )
                })?;
                pending.remove(&daemon_id);
                return Err(ta_provider_dsh::DshError::Process(error.to_string()));
            }
            // Do not await the daemon outcome inside the provider event loop:
            // resolving it re-enters this loop through `ExecutionHandle`.
            // The bridge emits the typed fact above, then this task forwards
            // one correlated outcome back through the control queue.
            tokio::spawn(async move {
                let Ok(outcome) = approval_bridge.wait(daemon_id.clone()).await else {
                    return;
                };
                let bridge_approval_id = {
                    let mut pending = match pending_approvals.lock() {
                        Ok(pending) => pending,
                        Err(_) => return,
                    };
                    let Some(pending) = pending.remove(&daemon_id) else {
                        return;
                    };
                    pending.bridge_approval_id
                };
                let _ = control_sender.send(ta_provider_dsh::BridgeControl::Approval {
                    approval_id: bridge_approval_id,
                    approved: matches!(outcome, ApprovalOutcome::Allow),
                });
            });
            Ok(())
        }
        ta_provider_dsh::BridgeEvent::Cancelled => Err(ta_provider_dsh::DshError::Cancelled),
        ta_provider_dsh::BridgeEvent::Error { message } => {
            Err(ta_provider_dsh::DshError::Protocol(message))
        }
        event => map_event(event, sink.as_ref()),
    }
}
impl Drop for DeepSeekHarnessHandle {
    fn drop(&mut self) {
        let _ = self.cancel();
        if let Ok(mut thread) = self.thread.lock()
            && let Some(thread) = thread.take()
        {
            // The provider owns a bounded deadline for every child operation.
            // Once cancellation is delivered, joining makes Drop prove there
            // is no detached live harness thread or orphaned bridge process.
            let _ = thread.join();
        }
    }
}

struct PendingApproval {
    bridge_approval_id: String,
    resolution_seen: bool,
}

fn map_event(
    event: ta_provider_dsh::BridgeEvent,
    sink: &dyn ExecutionSink,
) -> Result<(), ta_provider_dsh::DshError> {
    match event {
        ta_provider_dsh::BridgeEvent::Stream {
            turn_id,
            item_id,
            delta,
        } => sink
            .push_stream(StreamEmission {
                turn_id: Some(
                    AgentStreamTurnId::new(turn_id)
                        .map_err(|e| ta_provider_dsh::DshError::Protocol(e.to_string()))?,
                ),
                item_id: Some(
                    AgentStreamItemId::new(item_id)
                        .map_err(|e| ta_provider_dsh::DshError::Protocol(e.to_string()))?,
                ),
                fragment_sequence: None,
                frame: AgentStreamFrame::AssistantMessageDelta { delta },
            })
            .map_err(|e| ta_provider_dsh::DshError::Process(e.to_string())),
        ta_provider_dsh::BridgeEvent::Snapshot { continuation } => sink
            .push_provider_session_id(continuation)
            .map_err(|e| ta_provider_dsh::DshError::Process(e.to_string())),
        ta_provider_dsh::BridgeEvent::Error { message } => {
            Err(ta_provider_dsh::DshError::Protocol(message))
        }
        ta_provider_dsh::BridgeEvent::Initialized { .. }
        | ta_provider_dsh::BridgeEvent::Approval { .. }
        | ta_provider_dsh::BridgeEvent::Completed
        | ta_provider_dsh::BridgeEvent::Cancelled
        | ta_provider_dsh::BridgeEvent::Shutdown => Ok(()),
    }
}

fn map_error(error: ta_provider_dsh::DshError) -> ExecutionError {
    match error {
        ta_provider_dsh::DshError::Cancelled => {
            ExecutionError::Cancelled("turn_interrupted".to_string())
        }
        error => ExecutionError::ProcessFailed(error.to_string()),
    }
}
