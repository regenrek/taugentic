pub mod claims;
pub(crate) mod files;
pub(crate) mod git;
pub(crate) mod terminal;
pub mod worktree;

pub use claims::{
    CapsuleId, ClaimConflict, ClaimError, ClaimHandle, ClaimKind, ClaimRecord, ClaimRegistry,
    ConflictWarning,
};

pub use worktree::{
    CleanupPolicy, WorktreeError, WorktreeHandle, WorktreeManager, WorktreeRecord, WorktreeRequest,
};
