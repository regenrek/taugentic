mod daemon_control;
mod host;
mod orchestration;
pub mod workspace;

pub mod daemon {
    pub use crate::host::bootstrap::lifecycle::run;
}

pub use crate::daemon_control::{
    DaemonControlOperationError, daemon_log_path_for_socket_address, resolve_daemon_binary,
};
pub use crate::host::config::daemon_log_path_for_current_env;
pub use crate::orchestration::{
    DelegateRecipeResolutionRequest, RecipeLoadDiagnostic, RecipeRegistry, RecipeRegistryError,
    RegistryLoadOutcome, ResolvedDelegateRecipeRequest, resolve_delegate_recipe,
};
pub use crate::workspace::{
    CapsuleId, ClaimConflict, ClaimError, ClaimHandle, ClaimKind, ClaimRecord, ClaimRegistry,
    CleanupPolicy, ConflictWarning, WorktreeError, WorktreeHandle, WorktreeManager, WorktreeRecord,
    WorktreeRequest,
};

pub(crate) use daemon_control::*;
pub(crate) use orchestration::*;
pub(crate) use ta_jsonrpc::*;
pub(crate) use ta_protocol::wire::*;
