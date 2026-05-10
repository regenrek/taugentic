use std::path::PathBuf;

use crate::ExecutionError;
use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeStrategyId, AgentStreamTurnId, ApprovalResolution,
    AuthProfileId, CapsuleRecipe, LocalModelEndpointConfig, OutputContractKind, RunId, RunStatus,
    RuntimeExtensionState, RuntimePolicyMode, RuntimeProfileId, SessionId, WorkspaceMode,
    WorktreeCleanupPolicy,
};
use ta_provider_acp::descriptor::AcpProviderSpec;
use ta_provider_llm::client::StreamMessage;

/// Identifies which side owns the inner turn loop and native harness capabilities
/// for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentExecutionHarnessOwnership {
    /// Taugentic owns the turn loop, tools, approvals, sandbox policy, memory,
    /// telemetry, resume semantics, and future native subagent/workflow hooks.
    Native,
    /// Taugentic owns lifecycle, stream mapping, approvals/permission bridging,
    /// MCP forwarding, and the process boundary while the external harness owns
    /// its internal turn loop, tools, subagents, and sandbox.
    ExternalIntegration,
}

/// Typed execution-harness selection for a run.
///
/// This is the source of truth for which harness owns execution. It is
/// deliberately separate from provider identity, model selection, auth profile,
/// and runtime-profile storage identity: Native + OpenAI API key and future
/// Native + OpenAI OAuth/subscription both remain `NativeLoop`; ACP and
/// Codex app-server remain external integration lanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentExecutionHarness {
    NativeLoop,
    Acp { provider: AcpProviderSpec },
    CodexAppServer,
}

impl AgentExecutionHarness {
    pub fn ownership_kind(&self) -> AgentExecutionHarnessOwnership {
        match self {
            Self::NativeLoop => AgentExecutionHarnessOwnership::Native,
            Self::Acp { .. } | Self::CodexAppServer => {
                AgentExecutionHarnessOwnership::ExternalIntegration
            }
        }
    }

    pub fn is_native(&self) -> bool {
        self.ownership_kind() == AgentExecutionHarnessOwnership::Native
    }

    pub fn is_external(&self) -> bool {
        self.ownership_kind() == AgentExecutionHarnessOwnership::ExternalIntegration
    }

    pub fn supports_native_capabilities(&self) -> bool {
        self.is_native()
    }

    pub fn requires_external_process_boundary(&self) -> bool {
        self.is_external()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionRequest {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub runtime_profile_id: RuntimeProfileId,
    pub provider_id: AgentRuntimeStrategyId,
    pub execution_harness: AgentExecutionHarness,
    pub system_prompt: Option<String>,
    pub objective: String,
    pub model_id: Option<AgentRuntimeModelId>,
    pub auth_profile_id: Option<AuthProfileId>,
    pub local_endpoint: Option<LocalModelEndpointConfig>,
    pub policy_mode: RuntimePolicyMode,
    pub resume_provider_session_id: Option<String>,
    pub runtime_extensions: Vec<RuntimeExtensionState>,
    pub working_directory: PathBuf,
    pub artifact_root: PathBuf,
    pub fork_initial_state: Option<ForkInitialState>,
    pub output_contract: Option<OutputContractKind>,
    pub sandbox_profile: Option<String>,
    pub subagent_recipes: Vec<CapsuleRecipe>,
}

/// Initial native-loop history for forked runs.
///
/// Callers must provide deterministic provider history up to the fork boundary,
/// without appending the fork prompt. The native loop appends
/// [`ExecutionRequest::objective`] as the next user message when it builds the
/// run session.
#[derive(Debug, Clone, PartialEq)]
pub struct ForkInitialState {
    pub messages: Vec<StreamMessage>,
    pub provider_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeChildRunRequest {
    pub parent_run_id: RunId,
    pub parent_turn_id: AgentStreamTurnId,
    pub objective: String,
    pub output_contract: Option<OutputContractKind>,
    pub model_id: Option<AgentRuntimeModelId>,
    pub sandbox_profile: Option<String>,
    pub recipe_id: Option<String>,
    pub workspace_scope: WorkspaceMode,
    pub cleanup_policy: WorktreeCleanupPolicy,
    pub planned_write_files: Vec<String>,
}

impl NativeChildRunRequest {
    pub fn new(
        parent_run_id: RunId,
        parent_turn_id: AgentStreamTurnId,
        objective: impl Into<String>,
        output_contract: Option<OutputContractKind>,
        model_id: Option<AgentRuntimeModelId>,
        sandbox_profile: Option<String>,
        recipe_id: Option<String>,
    ) -> Result<Self, ExecutionError> {
        let objective = objective.into().trim().to_string();
        if objective.is_empty() {
            return Err(ExecutionError::InvalidToolInput(
                "subagent objective must be non-empty".to_string(),
            ));
        }
        Ok(Self {
            parent_run_id,
            parent_turn_id,
            objective,
            output_contract,
            model_id,
            sandbox_profile,
            recipe_id,
            workspace_scope: WorkspaceMode::WorkspaceWrite,
            cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
        })
    }

    pub fn with_workspace_scope(mut self, workspace_scope: WorkspaceMode) -> Self {
        self.workspace_scope = workspace_scope;
        self
    }

    pub fn with_cleanup_policy(mut self, cleanup_policy: WorktreeCleanupPolicy) -> Self {
        self.cleanup_policy = cleanup_policy;
        self
    }

    pub fn with_planned_write_files(mut self, planned_write_files: Vec<String>) -> Self {
        self.planned_write_files = planned_write_files;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeChildRunResult {
    pub run_id: RunId,
    pub status: RunStatus,
}

pub trait ExecutionHandle: Send + Sync {
    fn cancel(&self) -> Result<(), ExecutionError>;

    fn resolve_approval(&self, _resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        Err(ExecutionError::Unsupported(
            "runtime execution does not support live approval resolution".to_string(),
        ))
    }
}
