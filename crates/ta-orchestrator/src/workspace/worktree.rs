use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod worktree_tests;

const DEFAULT_BRANCH_PREFIX: &str = "ta/capsule-";
static GIT_WORKTREE_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
    parent_repo: PathBuf,
    git_binary: PathBuf,
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
        let git_binary = resolve_git_binary("git")
            .ok_or_else(|| WorktreeError::GitNotFound("git".to_string()))?;
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
        self.ensure_git_available()?;
        self.ensure_repo(&request.parent_repo)?;

        let parent_repo = fs::canonicalize(&request.parent_repo)?;
        let branch = self.branch_for(&request);
        let path =
            git_compatible_path(&self.worktree_path(&parent_repo, &request.capsule_short_id));
        if path.exists() {
            return Err(WorktreeError::PathExists { path });
        }

        self.ensure_clean_repo(&parent_repo)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let worktree_path_arg = git_path_arg(&path);
        let output = {
            let _guard = lock_git_mutation();
            self.run_git(
                &parent_repo,
                [
                    "worktree",
                    "add",
                    "-b",
                    branch.as_str(),
                    worktree_path_arg.as_str(),
                ],
                "git worktree add",
            )
        };
        if let Err(error) = output {
            return Err(map_worktree_add_error(error, &path, &branch));
        }

        Ok(WorktreeHandle {
            path,
            branch,
            cleanup_policy: request.cleanup_policy,
            parent_repo,
            git_binary: self.git_binary.clone(),
            state: Arc::new(Mutex::new(WorktreeHandleState::default())),
        })
    }

    pub fn list(&self, parent_repo: &Path) -> Result<Vec<WorktreeRecord>, WorktreeError> {
        self.ensure_git_available()?;
        self.ensure_repo(parent_repo)?;
        let output = self.run_git(
            parent_repo,
            ["worktree", "list", "--porcelain"],
            "git worktree list",
        )?;
        Ok(parse_worktree_list(&output.stdout))
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

    fn ensure_repo(&self, parent_repo: &Path) -> Result<(), WorktreeError> {
        match self.run_git(
            parent_repo,
            ["rev-parse", "--is-inside-work-tree"],
            "git rev-parse",
        ) {
            Ok(output) if String::from_utf8_lossy(&output.stdout).trim() == "true" => Ok(()),
            Ok(_) => Err(WorktreeError::NotARepo {
                path: parent_repo.to_path_buf(),
            }),
            Err(WorktreeError::GitFailed { .. }) => Err(WorktreeError::NotARepo {
                path: parent_repo.to_path_buf(),
            }),
            Err(error) => Err(error),
        }
    }

    fn ensure_clean_repo(&self, parent_repo: &Path) -> Result<(), WorktreeError> {
        let output = self.run_git(
            parent_repo,
            ["status", "--porcelain"],
            "git status --porcelain",
        )?;
        if output.stdout.is_empty() {
            Ok(())
        } else {
            Err(WorktreeError::GitFailed {
                context: "repository must be clean before creating a capsule worktree".to_string(),
                stderr: "working tree has uncommitted changes".to_string(),
            })
        }
    }

    fn ensure_git_available(&self) -> Result<(), WorktreeError> {
        command_output(&self.git_binary, ["--version"], "git --version").map(|_| ())
    }

    fn run_git<'a, I>(
        &self,
        parent_repo: &Path,
        args: I,
        context: &str,
    ) -> Result<Output, WorktreeError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut command = Command::new(&self.git_binary);
        command.arg("-C").arg(parent_repo).args(args);
        run_command(command, context)
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

        remove_worktree(
            &self.git_binary,
            &self.parent_repo,
            &self.path,
            &self.branch,
        )?;
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

fn remove_worktree(
    git_binary: &Path,
    parent_repo: &Path,
    path: &Path,
    branch: &str,
) -> Result<(), WorktreeError> {
    let _guard = lock_git_mutation();
    let path = git_path_arg(path);
    let mut remove = Command::new(git_binary);
    remove
        .arg("-C")
        .arg(parent_repo)
        .args(["worktree", "remove", "--force", path.as_str()]);
    run_command(remove, "git worktree remove")?;

    let mut delete_branch = Command::new(git_binary);
    delete_branch
        .arg("-C")
        .arg(parent_repo)
        .args(["branch", "-D", branch]);
    match run_command(delete_branch, "git branch -D") {
        Ok(_) => Ok(()),
        Err(WorktreeError::GitFailed { stderr, .. }) if branch_missing(&stderr) => Ok(()),
        Err(error) => Err(error),
    }
}

fn parse_worktree_list(stdout: &[u8]) -> Vec<WorktreeRecord> {
    let mut records = Vec::new();
    let mut current: Option<WorktreeRecord> = None;

    for line in String::from_utf8_lossy(stdout).lines() {
        if line.is_empty() {
            if let Some(record) = current.take() {
                records.push(record);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(record) = current.replace(WorktreeRecord {
                path: PathBuf::from(path),
                branch: None,
                head: None,
                locked: false,
            }) {
                records.push(record);
            }
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            record.head = Some(head.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            record.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string(),
            );
        } else if line == "locked" || line.starts_with("locked ") {
            record.locked = true;
        }
    }

    if let Some(record) = current {
        records.push(record);
    }

    records
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

fn command_output<'a, I>(command: &Path, args: I, context: &str) -> Result<Output, WorktreeError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut command = Command::new(command);
    command.args(args);
    run_command(command, context)
}

fn run_command(mut command: Command, context: &str) -> Result<Output, WorktreeError> {
    let output = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            WorktreeError::GitNotFound(command.get_program().to_string_lossy().into_owned())
        } else {
            WorktreeError::Io(error)
        }
    })?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(WorktreeError::GitFailed {
            context: context.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn resolve_git_binary(name: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(name);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate);
    }
    if candidate.components().count() == 1
        && command_output(&candidate, ["--version"], "git --version").is_ok()
    {
        return Some(candidate);
    }

    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|path| path.is_file())
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

fn lock_git_mutation() -> MutexGuard<'static, ()> {
    match GIT_WORKTREE_MUTATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
    {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn branch_missing(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("not found") || lower.contains("branch not found")
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
