use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use super::git::{GitRepository, GitRepositoryError};

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod worktree_tests;

const DEFAULT_BRANCH_PREFIX: &str = "ta/capsule-";

#[derive(Clone, Debug)]
pub struct WorktreeManager {
    git_binary: PathBuf,
    branch_prefix: String,
}

#[derive(Clone, Debug)]
pub struct WorktreeRequest {
    pub parent_repo: PathBuf,
    pub capsule_short_id: String,
    pub recipe_hint: Option<String>,
    pub cleanup_policy: CleanupPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPolicy {
    DeleteOnSuccess,
    DeleteOnTerminal,
    Keep,
    Manual,
}

#[derive(Debug)]
#[must_use]
pub struct WorktreeHandle {
    path: PathBuf,
    branch: String,
    cleanup_policy: CleanupPolicy,
    repository: GitRepository,
    state: Arc<Mutex<WorktreeHandleState>>,
}

#[derive(Clone, Debug)]
pub struct WorktreeRecord {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub locked: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum WorktreeError {
    #[error("git binary not found: {0}")]
    GitNotFound(String),
    #[error("not a git repository: {path}")]
    NotARepo { path: PathBuf },
    #[error("worktree path already exists: {path}")]
    PathExists { path: PathBuf },
    #[error("branch already exists: {branch}")]
    BranchExists { branch: String },
    #[error("git command failed: {context}: {stderr}")]
    GitFailed { context: String, stderr: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorktreeOutcome {
    Success,
    Failed,
    Cancelled,
    Terminal,
}

#[derive(Debug, Default)]
struct WorktreeHandleState {
    outcome: Option<WorktreeOutcome>,
    cleaned: bool,
}

impl WorktreeManager {
    pub fn new() -> Result<Self, WorktreeError> {
        let git_binary = GitRepository::resolve_default_binary().map_err(map_git_error)?;
        Ok(Self::with_git_binary(git_binary))
    }

    pub fn with_git_binary(git: PathBuf) -> Self {
        Self {
            git_binary: git,
            branch_prefix: DEFAULT_BRANCH_PREFIX.to_string(),
        }
    }

    pub fn with_branch_prefix(mut self, prefix: String) -> Self {
        self.branch_prefix = prefix;
        self
    }

    pub fn create(&self, request: WorktreeRequest) -> Result<WorktreeHandle, WorktreeError> {
        validate_key(&request.capsule_short_id, "capsule_short_id")?;
        let repository = self.repository(&request.parent_repo)?;
        let parent_repo = repository.root().to_path_buf();
        let branch = self.branch_for(&request);
        let path =
            git_compatible_path(&self.worktree_path(&parent_repo, &request.capsule_short_id));
        if path.exists() {
            return Err(WorktreeError::PathExists { path });
        }

        if !repository.is_clean().map_err(map_git_error)? {
            return Err(WorktreeError::GitFailed {
                context: "repository must be clean before creating a capsule worktree".to_string(),
                stderr: "working tree has uncommitted changes".to_string(),
            });
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let output = repository
            .add_worktree(&path, &branch)
            .map_err(map_git_error);
        if let Err(error) = output {
            return Err(map_worktree_add_error(error, &path, &branch));
        }

        Ok(WorktreeHandle {
            path,
            branch,
            cleanup_policy: request.cleanup_policy,
            repository,
            state: Arc::new(Mutex::new(WorktreeHandleState::default())),
        })
    }

    pub fn list(&self, parent_repo: &Path) -> Result<Vec<WorktreeRecord>, WorktreeError> {
        let repository = self.repository(parent_repo)?;
        Ok(repository
            .worktree_summaries()
            .map_err(map_git_error)?
            .into_iter()
            .map(|summary| WorktreeRecord {
                path: summary.path.as_path().to_path_buf(),
                branch: summary.branch,
                head: summary.head,
                locked: summary.locked,
            })
            .collect())
    }

    pub fn cleanup_orphans(&self, parent_repo: &Path) -> Result<Vec<PathBuf>, WorktreeError> {
        Ok(self
            .list(parent_repo)?
            .into_iter()
            .filter_map(|record| match record.branch {
                Some(branch) if branch.starts_with(&self.branch_prefix) => Some(record.path),
                _ => None,
            })
            .collect())
    }

    /// Removes only the deterministic resource allocated for `run_id`. This
    /// is deliberately idempotent: a preparing crash may occur after git has
    /// created the worktree but before the runtime registry receives a handle.
    pub fn discard_unpublished(
        &self,
        parent_repo: &Path,
        run_id: &str,
    ) -> Result<(), WorktreeError> {
        validate_key(run_id, "run_id")?;
        let repository = self.repository(parent_repo)?;
        let path = git_compatible_path(&self.worktree_path(repository.root(), run_id));
        if !path.exists() {
            return Ok(());
        }
        let branch = format!("{}{}", self.branch_prefix, run_id);
        repository
            .remove_worktree_and_branch(&path, &branch)
            .map_err(map_git_error)
    }

    /// Returns the deterministic identity before side effects.  Scheduled
    /// preparation persists this only when cleanup cannot complete.
    pub(crate) fn unpublished_identity(
        &self,
        parent_repo: &Path,
        run_id: &str,
        cleanup_policy: CleanupPolicy,
    ) -> Result<(PathBuf, PathBuf, String, CleanupPolicy), WorktreeError> {
        validate_key(run_id, "run_id")?;
        // This identity is persisted precisely when the repository is no
        // longer openable during preparation cleanup.  It must therefore be
        // derivable without discovery: the frozen repository path and run ID
        // are the complete identity. Repository access belongs to cleanup
        // and published-resource reattachment only.
        let parent = parent_repo.to_path_buf();
        let path = git_compatible_path(&self.worktree_path(&parent, run_id));
        Ok((
            parent,
            path,
            format!("{}{}", self.branch_prefix, run_id),
            cleanup_policy,
        ))
    }

    pub(crate) fn reattach(
        &self,
        parent_repo: &Path,
        path: &Path,
        branch: &str,
        cleanup_policy: CleanupPolicy,
    ) -> Result<WorktreeHandle, WorktreeError> {
        let repository = self.repository(parent_repo)?;
        let expected = git_compatible_path(path);
        let record = self
            .list(repository.root())?
            .into_iter()
            .find(|record| record.path == expected && record.branch.as_deref() == Some(branch));
        if record.is_none() {
            return Err(WorktreeError::GitFailed {
                context: "scheduled worktree identity does not exist".to_string(),
                stderr: format!("{} ({branch})", expected.display()),
            });
        }
        Ok(WorktreeHandle {
            path: expected,
            branch: branch.to_string(),
            cleanup_policy,
            repository,
            state: Arc::new(Mutex::new(WorktreeHandleState::default())),
        })
    }

    fn branch_for(&self, request: &WorktreeRequest) -> String {
        // Discovery mandates deterministic branch names without recipe text.
        format!("{}{}", self.branch_prefix, request.capsule_short_id)
    }

    fn worktree_path(&self, parent_repo: &Path, capsule_short_id: &str) -> PathBuf {
        parent_repo
            .join("target")
            .join("taugentic-worktrees")
            .join(capsule_short_id)
    }

    fn repository(&self, parent_repo: &Path) -> Result<GitRepository, WorktreeError> {
        GitRepository::open_with_binary(parent_repo, self.git_binary.clone()).map_err(|error| {
            if matches!(
                error,
                GitRepositoryError::NotARepository | GitRepositoryError::InvalidRepositoryPath
            ) {
                WorktreeError::NotARepo {
                    path: parent_repo.to_path_buf(),
                }
            } else {
                map_git_error(error)
            }
        })
    }
}

impl WorktreeHandle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn cleanup_policy(&self) -> CleanupPolicy {
        self.cleanup_policy
    }

    pub fn mark_success(&self) {
        self.record_outcome(WorktreeOutcome::Success);
    }

    pub fn mark_terminal(&self) {
        self.record_outcome(WorktreeOutcome::Terminal);
    }

    pub fn mark_failed(&self) {
        self.record_outcome(WorktreeOutcome::Failed);
    }

    pub fn mark_cancelled(&self) {
        self.record_outcome(WorktreeOutcome::Cancelled);
    }

    pub fn cleanup(&self) -> Result<(), WorktreeError> {
        self.cleanup_internal(true)
    }

    fn record_outcome(&self, outcome: WorktreeOutcome) {
        let mut state = lock_state(&self.state);
        state.outcome = Some(outcome);
    }

    fn cleanup_internal(&self, explicit: bool) -> Result<(), WorktreeError> {
        {
            let state = lock_state(&self.state);
            if state.cleaned {
                return Ok(());
            }
            if !explicit && !cleanup_allowed(self.cleanup_policy, state.outcome) {
                return Ok(());
            }
        }

        self.repository
            .remove_worktree_and_branch(&self.path, &self.branch)
            .map_err(map_git_error)?;
        let mut state = lock_state(&self.state);
        state.cleaned = true;
        Ok(())
    }
}

impl Drop for WorktreeHandle {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup_internal(false) {
            tracing::warn!(
                error_kind = error.kind(),
                "capsule worktree cleanup failed during drop"
            );
        }
    }
}

fn cleanup_allowed(policy: CleanupPolicy, outcome: Option<WorktreeOutcome>) -> bool {
    match policy {
        CleanupPolicy::DeleteOnSuccess => matches!(outcome, Some(WorktreeOutcome::Success)),
        CleanupPolicy::DeleteOnTerminal => outcome.is_some(),
        CleanupPolicy::Keep | CleanupPolicy::Manual => false,
    }
}

fn map_worktree_add_error(error: WorktreeError, path: &Path, branch: &str) -> WorktreeError {
    let WorktreeError::GitFailed { context, stderr } = error else {
        return error;
    };
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("branch") && lower.contains("already exists") {
        return WorktreeError::BranchExists {
            branch: branch.to_string(),
        };
    }
    if lower.contains("already exists") || lower.contains("already registered") {
        return WorktreeError::PathExists {
            path: path.to_path_buf(),
        };
    }
    WorktreeError::GitFailed { context, stderr }
}

fn validate_key(value: &str, field: &str) -> Result<(), WorktreeError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(WorktreeError::GitFailed {
            context: format!("invalid {field}"),
            stderr: "value must match [A-Za-z0-9._-]+".to_string(),
        });
    }
    Ok(())
}

fn git_path_arg(path: &Path) -> String {
    platform_git_path_arg(path)
}

fn git_compatible_path(path: &Path) -> PathBuf {
    PathBuf::from(git_path_arg(path))
}

#[cfg(windows)]
fn platform_git_path_arg(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    raw.into_owned()
}

#[cfg(not(windows))]
fn platform_git_path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn lock_state(state: &Arc<Mutex<WorktreeHandleState>>) -> MutexGuard<'_, WorktreeHandleState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn map_git_error(error: GitRepositoryError) -> WorktreeError {
    match error {
        GitRepositoryError::GitNotFound => WorktreeError::GitNotFound("git".to_string()),
        GitRepositoryError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            WorktreeError::GitNotFound("git".to_string())
        }
        GitRepositoryError::Io(error) => WorktreeError::Io(error),
        GitRepositoryError::CommandFailed { context, detail } => WorktreeError::GitFailed {
            context,
            stderr: detail,
        },
        error => WorktreeError::GitFailed {
            context: "git repository operation".to_string(),
            stderr: error.to_string(),
        },
    }
}

impl WorktreeError {
    fn kind(&self) -> &'static str {
        match self {
            Self::GitNotFound(_) => "git_not_found",
            Self::NotARepo { .. } => "not_a_repo",
            Self::PathExists { .. } => "path_exists",
            Self::BranchExists { .. } => "branch_exists",
            Self::GitFailed { .. } => "git_failed",
            Self::Io(_) => "io",
        }
    }
}
