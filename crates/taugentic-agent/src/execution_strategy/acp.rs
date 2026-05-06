use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ta_protocol::wire::{ApprovalResolution, ApprovalScope};
use ta_provider_acp::adapter::{
    AcpClientTrace, AcpPermissionDecision, AcpPermissionDecisionFuture, AcpPermissionRequest,
    AcpProcessAdapter, AcpProcessConfig, AcpSessionModelUpdate,
};
use ta_provider_acp::descriptor::AcpProviderSpec;
use ta_provider_acp::launch::{self, AcpLaunchInput};
use tokio_util::sync::CancellationToken;

use crate::approval::{ApprovalBridge, ApprovalDescriptor, ApprovalOutcome};
use crate::{ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink};

#[tracing::instrument(skip(request, sink), fields(runtime_profile = %request.runtime_profile_id.as_str()))]
pub(crate) async fn dispatch(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    provider: AcpProviderSpec,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let config = launch::build_config(
        &provider,
        AcpLaunchInput {
            policy_mode: request.policy_mode,
            working_directory: &request.working_directory,
            runtime_extensions: &request.runtime_extensions,
            model_id: request.model_id.as_ref(),
        },
    )?;
    dispatch_with_config(request, sink, config)
}

#[doc(hidden)]
pub fn dispatch_with_config(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    config: AcpProcessConfig,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let cancellation = CancellationToken::new();
    let approval_bridge = Arc::new(ApprovalBridge::new(
        request.run_id.clone(),
        sink.clone(),
        cancellation.clone(),
    ));
    let handle = Arc::new(AcpExecutionHandle {
        cancellation: cancellation.clone(),
        thread: Mutex::new(None),
        approval_bridge: approval_bridge.clone(),
    });
    let thread_handle = spawn_acp_thread(request, sink, config, cancellation, approval_bridge)?;
    *handle.thread.lock().map_err(|_| {
        ExecutionError::ProcessFailed("ACP execution handle lock poisoned".to_string())
    })? = Some(thread_handle);
    Ok(handle)
}

struct AcpExecutionHandle {
    cancellation: CancellationToken,
    thread: Mutex<Option<JoinHandle<()>>>,
    approval_bridge: Arc<ApprovalBridge>,
}

impl ExecutionHandle for AcpExecutionHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.cancellation.cancel();
        Ok(())
    }

    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        self.approval_bridge.resolve_from_runtime(resolution)
    }
}

impl Drop for AcpExecutionHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut thread) = self.thread.lock()
            && thread.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(handle) = thread.take()
        {
            let _ = handle.join();
        }
    }
}

fn spawn_acp_thread(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    config: AcpProcessConfig,
    cancellation: CancellationToken,
    approval_bridge: Arc<ApprovalBridge>,
) -> Result<JoinHandle<()>, ExecutionError> {
    std::thread::Builder::new()
        .name(format!("taugentic-acp-{}", request.run_id.as_str()))
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sink.fail(ExecutionError::ProcessFailed(error.to_string()));
                    return;
                }
            };
            let result = runtime.block_on(run_acp(
                request,
                sink.clone(),
                config,
                cancellation,
                approval_bridge,
            ));
            if let Err(error) = result {
                let _ = sink.fail(error);
            }
        })
        .map_err(|error| {
            ExecutionError::ProcessFailed(format!("failed to spawn ACP lane: {error}"))
        })
}

async fn run_acp(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    config: AcpProcessConfig,
    cancellation: CancellationToken,
    approval_bridge: Arc<ApprovalBridge>,
) -> Result<(), ExecutionError> {
    let trace = AcpClientTrace {
        run_id: request.run_id.as_str().to_string(),
        session_id: request.session_id.as_str().to_string(),
    };
    let mut client = AcpProcessAdapter::new(config).spawn(trace)?;
    let capabilities = client.initialize().await?;
    let session = client.create_session(&capabilities).await?;
    client.set_session_mode_if_needed(&session).await?;
    if matches!(
        client.set_session_model_if_needed(&session).await?,
        AcpSessionModelUpdate::Unsupported
    ) {
        sink.push_activity(
            "ACP provider does not support session/set_model; using provider-selected model",
        )?;
    }
    let mut on_event = |emission| {
        sink.push_stream(emission).map_err(|error| {
            ta_provider_acp::error::AcpClientError::ProcessFailed(error.to_string())
        })
    };
    let mut on_permission = {
        let approval_bridge = approval_bridge.clone();
        move |request: AcpPermissionRequest| -> AcpPermissionDecisionFuture {
            let approval_bridge = approval_bridge.clone();
            Box::pin(async move { resolve_acp_permission(approval_bridge, request).await })
        }
    };
    tokio::select! {
        result = client.prompt_stream_with_permissions(
            &session,
            &request.objective,
            &mut on_event,
            &mut on_permission
        ) => {
            result?;
        }
        () = cancellation.cancelled() => {
            approval_bridge.reject_all("turn_interrupted");
            let _ = client.cancel_session(&session).await;
            let _ = client.shutdown().await;
            return Err(ExecutionError::Cancelled("ACP execution cancelled".to_string()));
        }
    }
    sink.complete("ACP execution completed")?;
    client.shutdown().await?;
    Ok(())
}

async fn resolve_acp_permission(
    approval_bridge: Arc<ApprovalBridge>,
    request: AcpPermissionRequest,
) -> Result<AcpPermissionDecision, ta_provider_acp::error::AcpClientError> {
    let descriptor = ApprovalDescriptor::new(
        request.tool_call_id.clone(),
        request.tool_name.clone(),
        request.reason.clone(),
    );
    let approval_id = approval_bridge
        .request(acp_approval_scope(&request), &descriptor)
        .map_err(|error| {
            ta_provider_acp::error::AcpClientError::ProcessFailed(error.to_string())
        })?;
    let outcome = approval_bridge
        .wait(approval_id)
        .await
        .map_err(|error| ta_provider_acp::error::AcpClientError::Cancelled(error.to_string()))?;
    match outcome {
        ApprovalOutcome::Allow => request
            .allow_once_option_id()
            .map(|option_id| AcpPermissionDecision::Selected {
                option_id: option_id.to_string(),
            })
            .ok_or_else(|| {
                ta_provider_acp::error::AcpClientError::InvalidConfig(format!(
                    "ACP permission request {} has no allow option",
                    request.tool_call_id
                ))
            }),
        ApprovalOutcome::Deny => Ok(request
            .reject_once_option_id()
            .map(|option_id| AcpPermissionDecision::Selected {
                option_id: option_id.to_string(),
            })
            .unwrap_or(AcpPermissionDecision::Cancelled)),
        ApprovalOutcome::TurnInterrupted => Ok(AcpPermissionDecision::Cancelled),
    }
}

fn acp_approval_scope(request: &AcpPermissionRequest) -> ApprovalScope {
    let haystack = format!(
        "{} {}",
        request.tool_name.to_ascii_lowercase(),
        request
            .tool_kind
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
    );
    if haystack.contains("write") || haystack.contains("edit") || haystack.contains("patch") {
        ApprovalScope::FileWrite
    } else if haystack.contains("network") || haystack.contains("fetch") || haystack.contains("web")
    {
        ApprovalScope::NetworkAccess
    } else {
        ApprovalScope::ProcessExec
    }
}
