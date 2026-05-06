use ta_protocol::wire::{
    ApprovalRequest, ApprovalResolution, ArtifactKind, CapsuleResult, StreamEmission,
};
use ta_provider_llm::client::LlmTokenUsage;

use crate::{ExecutionError, NativeChildRunRequest, NativeChildRunResult};

pub trait ExecutionSink: Send + Sync {
    fn push_stream(&self, emission: StreamEmission) -> Result<(), ExecutionError>;
    fn record_token_usage(&self, usage: LlmTokenUsage) -> Result<(), ExecutionError>;
    fn push_activity(&self, detail: &str) -> Result<(), ExecutionError>;
    fn push_provider_session_id(&self, id: String) -> Result<(), ExecutionError>;
    fn request_approval(&self, request: ApprovalRequest) -> Result<(), ExecutionError>;
    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError>;
    fn record_artifact(&self, kind: ArtifactKind, storage_path: &str)
    -> Result<(), ExecutionError>;
    fn start_native_child_run(
        &self,
        _request: NativeChildRunRequest,
    ) -> Result<NativeChildRunResult, ExecutionError> {
        Err(ExecutionError::Unsupported(
            "runtime execution does not support native child runs".to_string(),
        ))
    }
    fn complete(&self, detail: &str) -> Result<(), ExecutionError>;
    fn complete_with_result(
        &self,
        detail: &str,
        result: Option<CapsuleResult>,
    ) -> Result<(), ExecutionError> {
        if result.is_some() {
            return Err(ExecutionError::Unsupported(
                "runtime execution does not support structured completion results".to_string(),
            ));
        }
        self.complete(detail)
    }
    fn fail(&self, error: ExecutionError) -> Result<(), ExecutionError>;
}
