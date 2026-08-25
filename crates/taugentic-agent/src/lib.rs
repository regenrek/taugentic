mod completion_result;
mod error;
mod request;
mod sink;

pub mod approval;
pub mod artifacts;
pub mod execution_strategy;
pub mod mcp;
mod native_execution;
pub mod patch;
pub mod queues;
pub mod session;
pub mod tools;
pub mod turn_loop;

use std::sync::Arc;

pub use error::ExecutionError;
pub use request::{
    AgentExecutionHarness, AgentExecutionHarnessOwnership, ExecutionHandle, ExecutionRequest,
    ForkInitialState, NativeChildRunRequest, NativeChildRunResult,
};
pub use sink::ExecutionSink;
pub use ta_protocol::wire::StreamEmission;

pub async fn run(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    if request.objective.trim().is_empty() {
        return Err(ExecutionError::InvalidConfig(
            "objective must be non-empty".to_string(),
        ));
    }
    execution_strategy::dispatch(request, sink).await
}
