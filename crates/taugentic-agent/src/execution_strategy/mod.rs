pub mod acp;
pub mod codex_app_server;
pub mod native_loop;

use std::sync::Arc;

use crate::{
    AgentExecutionHarness, ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink,
};

pub(crate) async fn dispatch(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    match request.execution_harness.clone() {
        AgentExecutionHarness::NativeLoop => native_loop::dispatch(request, sink).await,
        AgentExecutionHarness::Acp { provider } => acp::dispatch(request, sink, provider).await,
        AgentExecutionHarness::CodexAppServer => codex_app_server::dispatch(request, sink).await,
    }
}
