use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{Arc, Mutex, OnceLock, RwLock, Weak},
    thread,
};

use sha2::{Digest, Sha256};
use ta_code_host::GitHttpsAuthorization;
use ta_protocol::wire::{
    CodeHostCommitSummary, GIT_PATCH_MAX_BYTES, GIT_STATUS_MAX_ENTRIES, GitChangeKind,
    GitFileStatus, GitRepositorySnapshot, GitWorktreeSummary, WorkspacePath,
};

const GIT_COMMAND_MAX_BYTES: usize = 4 * 1024 * 1024;
const GIT_ERROR_MAX_BYTES: usize = 64 * 1024;
static GIT_REPOSITORY_LOCKS: OnceLock<Mutex<BTreeMap<PathBuf, Weak<RwLock<()>>>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct GitRepository {
    git_binary: PathBuf,
    root: PathBuf,
    lock: Arc<RwLock<()>>,
    #[cfg(test)]
    network_remote_override: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitObjectSnapshot {
    pub base_head: Option<String>,
    pub staged_commit: String,
    pub full_commit: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitPatch {
    pub patch: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitPushPreflight {
    pub remote_url: String,
    pub source_head: String,
    pub remote_head: Option<String>,
    pub commits: Vec<CodeHostCommitSummary>,
    pub truncated: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum GitRepositoryError {
    #[error("git binary not found")]
    GitNotFound,
    #[error("not a git repository")]
    NotARepository,
    #[error("git repository path is invalid")]
    InvalidRepositoryPath,
    #[error("git path is invalid: {0}")]
    InvalidPath(String),
    #[error("git command output exceeded its production bound")]
    OutputTooLarge,
    #[error("git command returned non-UTF-8 output")]
    NonUtf8Output,
    #[error("git command failed: {context}: {detail}")]
    CommandFailed { context: String, detail: String },
    #[error("git status output is malformed")]
    MalformedStatus,
    #[error("git worktree output is malformed")]
    MalformedWorktree,
    #[error("git repository has more than {0} changed paths")]
    TooManyStatusEntries(usize),
    #[error("git checkpoint identity is invalid")]
    InvalidCheckpointIdentity,
    #[error("git remote is invalid or does not exist")]
    InvalidRemote,
    #[error("git remote does not use the authenticated HTTPS origin")]
    UnsupportedRemoteAuthentication,
    #[error("git remote branch changed during preflight")]
    RemoteChanged,
    #[error("git push would not be a fast-forward update")]
    NonFastForward,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
struct CommandResult {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl GitRepository {
    pub(crate) fn resolve_default_binary() -> Result<PathBuf, GitRepositoryError> {
        resolve_binary("git").ok_or(GitRepositoryError::GitNotFound)
    }

    pub fn open(root: &Path) -> Result<Self, GitRepositoryError> {
        let git_binary = Self::resolve_default_binary()?;
        Self::open_with_binary(root, git_binary)
    }

    pub fn open_with_binary(root: &Path, git_binary: PathBuf) -> Result<Self, GitRepositoryError> {
        let root = fs::canonicalize(root).map_err(|_| GitRepositoryError::InvalidRepositoryPath)?;
        let repository = Self {
            git_binary: git_binary.clone(),
            lock: repository_lock(&root),
            root,
            #[cfg(test)]
            network_remote_override: None,
        };
        let inside_result = repository.run(
            &["rev-parse".into(), "--is-inside-work-tree".into()],
            &[],
            None,
            256,
        )?;
        if !inside_result.status.success() {
            return Err(GitRepositoryError::NotARepository);
        }
        let inside = utf8(inside_result.stdout)?;
        if inside.trim() != "true" {
            return Err(GitRepositoryError::NotARepository);
        }
        let top_level = repository.run_text(
            &["rev-parse".into(), "--show-toplevel".into()],
            &[],
            None,
            "git repository root",
        )?;
        let root = fs::canonicalize(top_level.trim())
            .map_err(|_| GitRepositoryError::InvalidRepositoryPath)?;
        let common_dir = repository.run_text(
            &[
                "rev-parse".into(),
                "--path-format=absolute".into(),
                "--git-common-dir".into(),
            ],
            &[],
            None,
            "git common directory",
        )?;
        let common_dir = fs::canonicalize(common_dir.trim())
            .map_err(|_| GitRepositoryError::InvalidRepositoryPath)?;
        Ok(Self {
            git_binary,
            root,
            lock: repository_lock(&common_dir),
            #[cfg(test)]
            network_remote_override: None,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn remote_urls(&self) -> Result<Vec<(String, String)>, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        let names = self.run_text(&["remote".into()], &[], None, "git remote list")?;
        let mut remotes = Vec::new();
        for name in names.lines().map(str::trim).filter(|name| !name.is_empty()) {
            let url = self.run_text(
                &["remote".into(), "get-url".into(), name.into()],
                &[],
                None,
                "git remote URL",
            )?;
            remotes.push((name.to_string(), url.trim().to_string()));
        }
        remotes.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(remotes)
    }

    pub(crate) fn push_preflight(
        &self,
        remote_name: &str,
        destination_branch: &str,
        network_remote_url: &str,
        authorization: &GitHttpsAuthorization,
        commit_limit: usize,
    ) -> Result<GitPushPreflight, GitRepositoryError> {
        validate_remote_name(remote_name)?;
        validate_branch(destination_branch)?;
        let _guard = write_repository(&self.lock);
        let remote_url = self.remote_url_unlocked(remote_name)?;
        require_authenticated_https_remote(network_remote_url, authorization.origin())?;
        let source_head = self.head()?.ok_or(GitRepositoryError::MalformedStatus)?;
        let remote_head =
            self.remote_head_unlocked(network_remote_url, destination_branch, authorization)?;
        if let Some(expected) = remote_head.as_deref() {
            self.fetch_remote_branch_unlocked(
                network_remote_url,
                destination_branch,
                authorization,
            )?;
            let fetched = self.run_text(
                &["rev-parse".into(), "--verify".into(), "FETCH_HEAD".into()],
                &[],
                None,
                "git fetched remote head",
            )?;
            let fetched = fetched.trim();
            validate_object_id(fetched)?;
            if fetched != expected {
                return Err(GitRepositoryError::RemoteChanged);
            }
            let ancestor = self.run(
                &[
                    "merge-base".into(),
                    "--is-ancestor".into(),
                    expected.into(),
                    source_head.clone().into(),
                ],
                &[],
                None,
                256,
            )?;
            if !ancestor.status.success() {
                return Err(GitRepositoryError::NonFastForward);
            }
        }
        let (commits, truncated) =
            self.commit_summaries_unlocked(remote_head.as_deref(), &source_head, commit_limit)?;
        Ok(GitPushPreflight {
            remote_url,
            source_head,
            remote_head,
            commits,
            truncated,
        })
    }

    pub(crate) fn push_non_force_if_unchanged(
        &self,
        remote_name: &str,
        destination_branch: &str,
        network_remote_url: &str,
        expected_source_head: &str,
        expected_remote_head: Option<&str>,
        expected_remote_url: &str,
        authorization: &GitHttpsAuthorization,
    ) -> Result<(), GitRepositoryError> {
        validate_remote_name(remote_name)?;
        validate_branch(destination_branch)?;
        validate_object_id(expected_source_head)?;
        if let Some(expected_remote_head) = expected_remote_head {
            validate_object_id(expected_remote_head)?;
        }
        let _guard = write_repository(&self.lock);
        let remote_url = self.remote_url_unlocked(remote_name)?;
        require_authenticated_https_remote(network_remote_url, authorization.origin())?;
        if remote_url != expected_remote_url
            || self.head()?.as_deref() != Some(expected_source_head)
            || self
                .remote_head_unlocked(network_remote_url, destination_branch, authorization)?
                .as_deref()
                != expected_remote_head
        {
            return Err(GitRepositoryError::RemoteChanged);
        }
        let args = push_arguments(
            &self.network_remote(network_remote_url),
            destination_branch,
            expected_source_head,
        )?;
        let env = authenticated_git_env(authorization);
        self.run_success(&args, &env, None, "git push")?;
        Ok(())
    }

    fn remote_url_unlocked(&self, remote_name: &str) -> Result<String, GitRepositoryError> {
        validate_remote_name(remote_name)?;
        let result = self.run(
            &["remote".into(), "get-url".into(), remote_name.into()],
            &[],
            None,
            GIT_ERROR_MAX_BYTES,
        )?;
        if !result.status.success() {
            return Err(GitRepositoryError::InvalidRemote);
        }
        let value = utf8(result.stdout)?.trim().to_string();
        if value.is_empty() {
            return Err(GitRepositoryError::InvalidRemote);
        }
        Ok(value)
    }

    fn remote_head_unlocked(
        &self,
        network_remote_url: &str,
        destination_branch: &str,
        authorization: &GitHttpsAuthorization,
    ) -> Result<Option<String>, GitRepositoryError> {
        let reference = format!("refs/heads/{destination_branch}");
        let env = authenticated_git_env(authorization);
        let result = self.run(
            &[
                "ls-remote".into(),
                "--exit-code".into(),
                "--refs".into(),
                self.network_remote(network_remote_url),
                reference.clone().into(),
            ],
            &env,
            None,
            GIT_ERROR_MAX_BYTES,
        )?;
        if result.status.code() == Some(2) {
            return Ok(None);
        }
        self.require_success(&result, "git remote head")?;
        let value = utf8(result.stdout)?;
        let (head, returned_reference) = value
            .trim()
            .split_once(char::is_whitespace)
            .ok_or(GitRepositoryError::MalformedStatus)?;
        validate_object_id(head)?;
        if returned_reference.trim() != reference {
            return Err(GitRepositoryError::MalformedStatus);
        }
        Ok(Some(head.to_string()))
    }

    fn fetch_remote_branch_unlocked(
        &self,
        network_remote_url: &str,
        destination_branch: &str,
        authorization: &GitHttpsAuthorization,
    ) -> Result<(), GitRepositoryError> {
        let reference = format!("refs/heads/{destination_branch}");
        let env = authenticated_git_env(authorization);
        self.run_success(
            &[
                "fetch".into(),
                "--quiet".into(),
                "--no-tags".into(),
                self.network_remote(network_remote_url),
                reference.into(),
            ],
            &env,
            None,
            "git push preflight fetch",
        )?;
        Ok(())
    }

    fn network_remote(&self, network_remote_url: &str) -> OsString {
        #[cfg(test)]
        if let Some(network_remote) = &self.network_remote_override {
            return network_remote.into();
        }
        network_remote_url.into()
    }

    fn commit_summaries_unlocked(
        &self,
        remote_head: Option<&str>,
        source_head: &str,
        limit: usize,
    ) -> Result<(Vec<CodeHostCommitSummary>, bool), GitRepositoryError> {
        validate_object_id(source_head)?;
        if let Some(remote_head) = remote_head {
            validate_object_id(remote_head)?;
        }
        let bounded_limit = limit.clamp(1, 100);
        let range = remote_head
            .map(|remote_head| format!("{remote_head}..{source_head}"))
            .unwrap_or_else(|| source_head.to_string());
        let value = self.run_text(
            &[
                "log".into(),
                "--reverse".into(),
                format!("--max-count={}", bounded_limit + 1).into(),
                "--format=%H%x00%s%x00".into(),
                "-z".into(),
                range.into(),
                "--".into(),
            ],
            &[],
            None,
            "git push commit list",
        )?;
        let fields = value
            .split('\0')
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if fields.len() % 2 != 0 {
            return Err(GitRepositoryError::MalformedStatus);
        }
        let mut commits = fields
            .chunks_exact(2)
            .map(|fields| {
                validate_object_id(fields[0])?;
                Ok(CodeHostCommitSummary {
                    id: fields[0].to_string(),
                    subject: fields[1].to_string(),
                })
            })
            .collect::<Result<Vec<_>, GitRepositoryError>>()?;
        let truncated = commits.len() > bounded_limit;
        commits.truncate(bounded_limit);
        Ok((commits, truncated))
    }

    pub fn snapshot(&self) -> Result<GitRepositorySnapshot, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        self.snapshot_unlocked()
    }

    fn snapshot_unlocked(&self) -> Result<GitRepositorySnapshot, GitRepositoryError> {
        let result = self.run(
            &[
                "status".into(),
                "--porcelain=v2".into(),
                "--branch".into(),
                "-z".into(),
                "--untracked-files=all".into(),
            ],
            &[],
            None,
            GIT_COMMAND_MAX_BYTES,
        )?;
        self.require_success(&result, "git status")?;
        if result.stdout_truncated {
            return Err(GitRepositoryError::OutputTooLarge);
        }
        let parsed = parse_status(&result.stdout)?;
        if parsed.files.len() > GIT_STATUS_MAX_ENTRIES {
            return Err(GitRepositoryError::TooManyStatusEntries(
                GIT_STATUS_MAX_ENTRIES,
            ));
        }
        let fingerprint =
            repository_fingerprint(parsed.head.as_deref(), &self.index_tree()?, &result.stdout);
        let worktrees = self.worktrees()?;
        Ok(GitRepositorySnapshot {
            branch: parsed.branch,
            head: parsed.head,
            upstream: parsed.upstream,
            ahead: parsed.ahead,
            behind: parsed.behind,
            files: parsed.files,
            worktrees,
            truncated: false,
            fingerprint,
        })
    }

    pub fn staged_patch(&self) -> Result<GitPatch, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        self.patch_command(&[
            "diff".into(),
            "--cached".into(),
            "--binary".into(),
            "--no-ext-diff".into(),
            "--find-renames".into(),
            "--".into(),
        ])
    }

    pub fn unstaged_patch(&self) -> Result<GitPatch, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        let snapshot = self.capture_objects_unlocked()?;
        self.patch_between_unlocked(&snapshot.staged_commit, &snapshot.full_commit)
    }

    pub fn patch_between(&self, from: &str, to: &str) -> Result<GitPatch, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        self.patch_between_unlocked(from, to)
    }

    fn patch_between_unlocked(&self, from: &str, to: &str) -> Result<GitPatch, GitRepositoryError> {
        validate_object_id(from)?;
        validate_object_id(to)?;
        self.patch_command(&[
            "diff".into(),
            "--binary".into(),
            "--no-ext-diff".into(),
            "--find-renames".into(),
            from.into(),
            to.into(),
            "--".into(),
        ])
    }

    pub fn stage_paths(&self, paths: &[String]) -> Result<(), GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        let paths = validate_paths(paths)?;
        let mut args = vec!["add".into(), "--".into()];
        args.extend(paths);
        self.run_success(&args, &[], None, "git add")?;
        Ok(())
    }

    pub fn unstage_paths(&self, paths: &[String]) -> Result<(), GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        let paths = validate_paths(paths)?;
        let mut args = if self.head()?.is_some() {
            vec!["restore".into(), "--staged".into(), "--".into()]
        } else {
            vec![
                "rm".into(),
                "--cached".into(),
                "-r".into(),
                "--ignore-unmatch".into(),
                "--".into(),
            ]
        };
        args.extend(paths);
        self.run_success(&args, &[], None, "git unstage")?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<String, GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        let message = message.trim();
        if message.is_empty() {
            return Err(GitRepositoryError::CommandFailed {
                context: "git commit".to_string(),
                detail: "commit message must not be empty".to_string(),
            });
        }
        self.run_success(
            &["commit".into(), "-m".into(), message.into()],
            &[("GIT_TERMINAL_PROMPT", OsString::from("0"))],
            None,
            "git commit",
        )?;
        self.head()?.ok_or(GitRepositoryError::MalformedStatus)
    }

    pub fn capture_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<GitObjectSnapshot, GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        validate_checkpoint_id(checkpoint_id)?;
        let snapshot = self.capture_objects_unlocked()?;
        let reference = format!("refs/taugentic/checkpoints/{checkpoint_id}");
        self.run_success(
            &[
                "update-ref".into(),
                reference.into(),
                snapshot.full_commit.clone().into(),
            ],
            &[],
            None,
            "git checkpoint ref update",
        )?;
        Ok(snapshot)
    }

    pub fn delete_checkpoint_ref(&self, checkpoint_id: &str) -> Result<(), GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        validate_checkpoint_id(checkpoint_id)?;
        let reference = format!("refs/taugentic/checkpoints/{checkpoint_id}");
        self.run_success(
            &["update-ref".into(), "-d".into(), reference.into()],
            &[],
            None,
            "git checkpoint ref delete",
        )?;
        Ok(())
    }

    pub fn capture_objects(&self) -> Result<GitObjectSnapshot, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        self.capture_objects_unlocked()
    }

    fn capture_objects_unlocked(&self) -> Result<GitObjectSnapshot, GitRepositoryError> {
        let base_head = self.head()?;
        let staged_tree = self.index_tree()?;
        let staged_commit = self.commit_tree(&staged_tree, base_head.as_deref(), "staged")?;
        let temp_index = self.temp_index_path()?;
        let capture = (|| {
            let env = [("GIT_INDEX_FILE", temp_index.as_os_str().to_os_string())];
            self.run_success(
                &["read-tree".into(), staged_tree.clone().into()],
                &env,
                None,
                "git checkpoint index seed",
            )?;
            self.run_success(
                &["add".into(), "--all".into(), "--".into(), ".".into()],
                &env,
                None,
                "git checkpoint worktree capture",
            )?;
            let full_tree = self.run_text(
                &["write-tree".into()],
                &env,
                None,
                "git checkpoint full tree",
            )?;
            let full_tree = full_tree.trim().to_string();
            validate_object_id(&full_tree)?;
            let full_commit = self.commit_tree(&full_tree, Some(&staged_commit), "full")?;
            let status = self.status_bytes()?;
            let fingerprint = repository_fingerprint(base_head.as_deref(), &staged_tree, &status);
            Ok(GitObjectSnapshot {
                base_head,
                staged_commit,
                full_commit,
                fingerprint,
            })
        })();
        let remove_result = fs::remove_file(&temp_index);
        if let Err(error) = remove_result
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(error.into());
        }
        capture
    }

    pub fn restore_checkpoint(
        &self,
        staged_commit: &str,
        full_commit: &str,
    ) -> Result<GitRepositorySnapshot, GitRepositoryError> {
        let _guard = write_repository(&self.lock);
        validate_object_id(staged_commit)?;
        validate_object_id(full_commit)?;
        self.run_success(
            &["clean".into(), "-fd".into(), "--".into(), ".".into()],
            &[],
            None,
            "git checkpoint clean",
        )?;
        self.run_success(
            &[
                "read-tree".into(),
                "--reset".into(),
                "-u".into(),
                full_commit.into(),
            ],
            &[],
            None,
            "git checkpoint worktree restore",
        )?;
        self.run_success(
            &["read-tree".into(), staged_commit.into()],
            &[],
            None,
            "git checkpoint index restore",
        )?;
        self.snapshot_unlocked()
    }

    pub(crate) fn is_clean(&self) -> Result<bool, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        Ok(parse_status(&self.status_bytes()?)?.files.is_empty())
    }

    pub(crate) fn worktree_summaries(&self) -> Result<Vec<GitWorktreeSummary>, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        self.worktrees()
    }

    pub(crate) fn add_worktree(&self, path: &Path, branch: &str) -> Result<(), GitRepositoryError> {
        validate_branch(branch)?;
        if !path.is_absolute() {
            return Err(GitRepositoryError::InvalidPath(
                "worktree path must be absolute".to_string(),
            ));
        }
        let _guard = write_repository(&self.lock);
        self.run_success(
            &[
                "worktree".into(),
                "add".into(),
                "-b".into(),
                branch.into(),
                path.as_os_str().to_os_string(),
            ],
            &[],
            None,
            "git worktree add",
        )?;
        Ok(())
    }

    pub(crate) fn remove_worktree_and_branch(
        &self,
        path: &Path,
        branch: &str,
    ) -> Result<(), GitRepositoryError> {
        validate_branch(branch)?;
        if !path.is_absolute() {
            return Err(GitRepositoryError::InvalidPath(
                "worktree path must be absolute".to_string(),
            ));
        }
        let _guard = write_repository(&self.lock);
        self.run_success(
            &[
                "worktree".into(),
                "remove".into(),
                "--force".into(),
                path.as_os_str().to_os_string(),
            ],
            &[],
            None,
            "git worktree remove",
        )?;
        let reference = format!("refs/heads/{branch}");
        let exists = self.run(
            &[
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                reference.into(),
            ],
            &[],
            None,
            256,
        )?;
        if exists.status.success() {
            self.run_success(
                &["branch".into(), "-D".into(), branch.into()],
                &[],
                None,
                "git branch delete",
            )?;
        }
        Ok(())
    }

    pub fn current_fingerprint(&self) -> Result<String, GitRepositoryError> {
        let _guard = read_repository(&self.lock);
        let status = self.status_bytes()?;
        Ok(repository_fingerprint(
            self.head()?.as_deref(),
            &self.index_tree()?,
            &status,
        ))
    }

    fn head(&self) -> Result<Option<String>, GitRepositoryError> {
        let result = self.run(
            &[
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "HEAD".into(),
            ],
            &[],
            None,
            256,
        )?;
        if !result.status.success() {
            return Ok(None);
        }
        let value = utf8(result.stdout)?.trim().to_string();
        validate_object_id(&value)?;
        Ok(Some(value))
    }

    fn index_tree(&self) -> Result<String, GitRepositoryError> {
        let value = self.run_text(&["write-tree".into()], &[], None, "git index tree")?;
        let value = value.trim().to_string();
        validate_object_id(&value)?;
        Ok(value)
    }

    fn commit_tree(
        &self,
        tree: &str,
        parent: Option<&str>,
        label: &str,
    ) -> Result<String, GitRepositoryError> {
        validate_object_id(tree)?;
        if let Some(parent) = parent {
            validate_object_id(parent)?;
        }
        let mut args = vec!["commit-tree".into(), tree.into()];
        if let Some(parent) = parent {
            args.extend(["-p".into(), parent.into()]);
        }
        args.extend(["-m".into(), format!("Taugentic {label} checkpoint").into()]);
        let identity = [
            ("GIT_AUTHOR_NAME", OsString::from("Taugentic")),
            ("GIT_AUTHOR_EMAIL", OsString::from("checkpoint@localhost")),
            ("GIT_COMMITTER_NAME", OsString::from("Taugentic")),
            (
                "GIT_COMMITTER_EMAIL",
                OsString::from("checkpoint@localhost"),
            ),
        ];
        let value = self.run_text(&args, &identity, None, "git checkpoint commit-tree")?;
        let value = value.trim().to_string();
        validate_object_id(&value)?;
        Ok(value)
    }

    fn temp_index_path(&self) -> Result<PathBuf, GitRepositoryError> {
        let relative = format!("taugentic/index-{}", uuid::Uuid::new_v4().simple());
        let value = self.run_text(
            &["rev-parse".into(), "--git-path".into(), relative.into()],
            &[],
            None,
            "git checkpoint path",
        )?;
        let path = PathBuf::from(value.trim());
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        let parent = path
            .parent()
            .ok_or(GitRepositoryError::InvalidRepositoryPath)?;
        fs::create_dir_all(parent)?;
        Ok(path)
    }

    fn status_bytes(&self) -> Result<Vec<u8>, GitRepositoryError> {
        let result = self.run(
            &[
                "status".into(),
                "--porcelain=v2".into(),
                "--branch".into(),
                "-z".into(),
                "--untracked-files=all".into(),
            ],
            &[],
            None,
            GIT_COMMAND_MAX_BYTES,
        )?;
        self.require_success(&result, "git status")?;
        if result.stdout_truncated {
            return Err(GitRepositoryError::OutputTooLarge);
        }
        Ok(result.stdout)
    }

    fn worktrees(&self) -> Result<Vec<GitWorktreeSummary>, GitRepositoryError> {
        let value = self.run_text(
            &[
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
                "-z".into(),
            ],
            &[],
            None,
            "git worktree list",
        )?;
        parse_worktrees(value.as_bytes(), &self.root)
    }

    fn patch_command(&self, args: &[OsString]) -> Result<GitPatch, GitRepositoryError> {
        let result = self.run(args, &[], None, GIT_PATCH_MAX_BYTES)?;
        self.require_success(&result, "git diff")?;
        Ok(GitPatch {
            patch: utf8(result.stdout)?,
            truncated: result.stdout_truncated,
        })
    }

    fn run_success(
        &self,
        args: &[OsString],
        envs: &[(&str, OsString)],
        stdin: Option<&[u8]>,
        context: &str,
    ) -> Result<CommandResult, GitRepositoryError> {
        let result = self.run(args, envs, stdin, GIT_COMMAND_MAX_BYTES)?;
        self.require_success(&result, context)?;
        if result.stdout_truncated {
            return Err(GitRepositoryError::OutputTooLarge);
        }
        Ok(result)
    }

    fn run_text(
        &self,
        args: &[OsString],
        envs: &[(&str, OsString)],
        stdin: Option<&[u8]>,
        context: &str,
    ) -> Result<String, GitRepositoryError> {
        let result = self.run_success(args, envs, stdin, context)?;
        utf8(result.stdout)
    }

    fn require_success(
        &self,
        result: &CommandResult,
        context: &str,
    ) -> Result<(), GitRepositoryError> {
        if result.status.success() {
            return Ok(());
        }
        let mut detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
        if detail.is_empty() {
            detail = format!("exit status {}", result.status);
        }
        if result.stderr_truncated {
            detail.push_str(" [truncated]");
        }
        Err(GitRepositoryError::CommandFailed {
            context: context.to_string(),
            detail,
        })
    }

    fn run(
        &self,
        args: &[OsString],
        envs: &[(&str, OsString)],
        stdin: Option<&[u8]>,
        stdout_limit: usize,
    ) -> Result<CommandResult, GitRepositoryError> {
        let mut command = Command::new(&self.git_binary);
        command
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .envs(envs.iter().map(|(key, value)| (*key, value)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        let mut child = command.spawn()?;
        if let Some(bytes) = stdin {
            child
                .stdin
                .take()
                .ok_or_else(|| GitRepositoryError::Io(std::io::Error::other("missing stdin")))?
                .write_all(bytes)?;
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GitRepositoryError::Io(std::io::Error::other("missing stdout")))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| GitRepositoryError::Io(std::io::Error::other("missing stderr")))?;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, GIT_ERROR_MAX_BYTES));
        let status = child.wait()?;
        let (stdout, stdout_truncated) = stdout_reader
            .join()
            .map_err(|_| std::io::Error::other("git stdout reader failed"))??;
        let (stderr, stderr_truncated) = stderr_reader
            .join()
            .map_err(|_| std::io::Error::other("git stderr reader failed"))??;
        Ok(CommandResult {
            status,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
        })
    }
}

#[derive(Default)]
struct ParsedStatus {
    branch: Option<String>,
    head: Option<String>,
    upstream: Option<String>,
    ahead: u32,
    behind: u32,
    files: Vec<GitFileStatus>,
}

fn parse_status(bytes: &[u8]) -> Result<ParsedStatus, GitRepositoryError> {
    let mut parsed = ParsedStatus::default();
    let records = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0;
    while index < records.len() {
        let record = records[index];
        index += 1;
        if record.is_empty() {
            continue;
        }
        let record = std::str::from_utf8(record).map_err(|_| GitRepositoryError::NonUtf8Output)?;
        for line in record.lines() {
            if let Some(value) = line.strip_prefix("# branch.oid ") {
                if value != "(initial)" {
                    parsed.head = Some(value.to_string());
                }
            } else if let Some(value) = line.strip_prefix("# branch.head ") {
                if value != "(detached)" {
                    parsed.branch = Some(value.to_string());
                }
            } else if let Some(value) = line.strip_prefix("# branch.upstream ") {
                parsed.upstream = Some(value.to_string());
            } else if let Some(value) = line.strip_prefix("# branch.ab ") {
                for component in value.split_whitespace() {
                    if let Some(ahead) = component.strip_prefix('+') {
                        parsed.ahead = ahead
                            .parse()
                            .map_err(|_| GitRepositoryError::MalformedStatus)?;
                    } else if let Some(behind) = component.strip_prefix('-') {
                        parsed.behind = behind
                            .parse()
                            .map_err(|_| GitRepositoryError::MalformedStatus)?;
                    }
                }
            } else if let Some(rest) = line.strip_prefix("1 ") {
                let fields = rest.splitn(8, ' ').collect::<Vec<_>>();
                if fields.len() != 8 {
                    return Err(GitRepositoryError::MalformedStatus);
                }
                parsed.files.push(status_entry(fields[0], fields[7], None)?);
            } else if let Some(rest) = line.strip_prefix("2 ") {
                let fields = rest.splitn(9, ' ').collect::<Vec<_>>();
                if fields.len() != 9 || index >= records.len() {
                    return Err(GitRepositoryError::MalformedStatus);
                }
                let original = std::str::from_utf8(records[index])
                    .map_err(|_| GitRepositoryError::NonUtf8Output)?;
                index += 1;
                parsed.files.push(status_entry(
                    fields[0],
                    fields[8],
                    Some(original.to_string()),
                )?);
            } else if let Some(rest) = line.strip_prefix("u ") {
                let fields = rest.splitn(11, ' ').collect::<Vec<_>>();
                if fields.len() != 11 {
                    return Err(GitRepositoryError::MalformedStatus);
                }
                parsed.files.push(GitFileStatus {
                    path: fields[10].to_string(),
                    original_path: None,
                    staged: Some(GitChangeKind::Unmerged),
                    unstaged: Some(GitChangeKind::Unmerged),
                });
            } else if let Some(path) = line.strip_prefix("? ") {
                parsed.files.push(GitFileStatus {
                    path: path.to_string(),
                    original_path: None,
                    staged: None,
                    unstaged: Some(GitChangeKind::Untracked),
                });
            } else if !line.starts_with("! ") {
                return Err(GitRepositoryError::MalformedStatus);
            }
        }
    }
    parsed
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(parsed)
}

fn status_entry(
    xy: &str,
    path: &str,
    original_path: Option<String>,
) -> Result<GitFileStatus, GitRepositoryError> {
    let mut chars = xy.chars();
    let staged = status_kind(chars.next().ok_or(GitRepositoryError::MalformedStatus)?)?;
    let unstaged = status_kind(chars.next().ok_or(GitRepositoryError::MalformedStatus)?)?;
    Ok(GitFileStatus {
        path: path.to_string(),
        original_path,
        staged,
        unstaged,
    })
}

fn status_kind(value: char) -> Result<Option<GitChangeKind>, GitRepositoryError> {
    Ok(match value {
        '.' => None,
        'A' => Some(GitChangeKind::Added),
        'M' => Some(GitChangeKind::Modified),
        'D' => Some(GitChangeKind::Deleted),
        'R' => Some(GitChangeKind::Renamed),
        'C' => Some(GitChangeKind::Copied),
        'T' => Some(GitChangeKind::TypeChanged),
        'U' => Some(GitChangeKind::Unmerged),
        _ => return Err(GitRepositoryError::MalformedStatus),
    })
}

fn parse_worktrees(
    bytes: &[u8],
    current_root: &Path,
) -> Result<Vec<GitWorktreeSummary>, GitRepositoryError> {
    let value = std::str::from_utf8(bytes).map_err(|_| GitRepositoryError::NonUtf8Output)?;
    let mut rows = Vec::new();
    let mut fields = BTreeMap::<String, String>::new();
    for record in value.split('\0') {
        if record.is_empty() {
            if !fields.is_empty() {
                rows.push(worktree_row(&fields, current_root)?);
                fields.clear();
            }
            continue;
        }
        for line in record.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, "true"));
            if key == "worktree" && fields.contains_key("worktree") {
                rows.push(worktree_row(&fields, current_root)?);
                fields.clear();
            }
            fields.insert(key.to_string(), value.to_string());
        }
    }
    if !fields.is_empty() {
        rows.push(worktree_row(&fields, current_root)?);
    }
    Ok(rows)
}

fn worktree_row(
    fields: &BTreeMap<String, String>,
    current_root: &Path,
) -> Result<GitWorktreeSummary, GitRepositoryError> {
    let path = PathBuf::from(
        fields
            .get("worktree")
            .ok_or(GitRepositoryError::MalformedWorktree)?,
    );
    let canonical = fs::canonicalize(&path).map_err(|_| GitRepositoryError::MalformedWorktree)?;
    let path = WorkspacePath::from_canonical_wire_value(canonical.to_string_lossy().into_owned())
        .map_err(|_| GitRepositoryError::MalformedWorktree)?;
    Ok(GitWorktreeSummary {
        current: canonical == current_root,
        path,
        branch: fields
            .get("branch")
            .map(|value| value.trim_start_matches("refs/heads/").to_string()),
        head: fields.get("HEAD").cloned(),
        locked: fields.contains_key("locked"),
    })
}

fn validate_paths(paths: &[String]) -> Result<Vec<OsString>, GitRepositoryError> {
    if paths.is_empty() {
        return Err(GitRepositoryError::InvalidPath(
            "empty path set".to_string(),
        ));
    }
    paths
        .iter()
        .map(|path| {
            let candidate = Path::new(path);
            if path.trim().is_empty()
                || candidate.is_absolute()
                || candidate.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
            {
                return Err(GitRepositoryError::InvalidPath(path.clone()));
            }
            Ok(candidate.as_os_str().to_os_string())
        })
        .collect()
}

fn validate_checkpoint_id(value: &str) -> Result<(), GitRepositoryError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(GitRepositoryError::InvalidCheckpointIdentity);
    }
    Ok(())
}

fn validate_remote_name(value: &str) -> Result<(), GitRepositoryError> {
    if value.is_empty()
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(GitRepositoryError::InvalidRemote);
    }
    Ok(())
}

fn require_authenticated_https_remote(
    remote_url: &str,
    expected_origin: &str,
) -> Result<(), GitRepositoryError> {
    let parsed = url::Url::parse(remote_url)
        .map_err(|_| GitRepositoryError::UnsupportedRemoteAuthentication)?;
    let origin = format!(
        "{}://{}{}",
        parsed.scheme(),
        parsed
            .host_str()
            .ok_or(GitRepositoryError::UnsupportedRemoteAuthentication)?,
        parsed
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
    );
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !origin.eq_ignore_ascii_case(expected_origin)
    {
        return Err(GitRepositoryError::UnsupportedRemoteAuthentication);
    }
    Ok(())
}

fn authenticated_git_env(authorization: &GitHttpsAuthorization) -> Vec<(&'static str, OsString)> {
    vec![
        ("GIT_TERMINAL_PROMPT", OsString::from("0")),
        ("GIT_CONFIG_COUNT", OsString::from("1")),
        (
            "GIT_CONFIG_KEY_0",
            OsString::from(format!("http.{}/.extraHeader", authorization.origin())),
        ),
        (
            "GIT_CONFIG_VALUE_0",
            OsString::from(authorization.expose_extra_header()),
        ),
    ]
}

fn push_arguments(
    network_remote: &OsString,
    destination_branch: &str,
    source_head: &str,
) -> Result<Vec<OsString>, GitRepositoryError> {
    validate_branch(destination_branch)?;
    validate_object_id(source_head)?;
    Ok(vec![
        "push".into(),
        "--porcelain".into(),
        network_remote.clone(),
        format!("{source_head}:refs/heads/{destination_branch}").into(),
    ])
}

fn validate_branch(value: &str) -> Result<(), GitRepositoryError> {
    if value.is_empty()
        || value.starts_with('-')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.ends_with('.')
        || value.contains("..")
        || value.contains("@{")
        || value.bytes().any(|byte| {
            byte.is_ascii_control()
                || matches!(byte, b' ' | b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        })
    {
        return Err(GitRepositoryError::InvalidPath(value.to_string()));
    }
    Ok(())
}

fn validate_object_id(value: &str) -> Result<(), GitRepositoryError> {
    if (value.len() == 40 || value.len() == 64)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(GitRepositoryError::MalformedStatus)
    }
}

fn repository_fingerprint(head: Option<&str>, index_tree: &str, status: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(head.unwrap_or("unborn").as_bytes());
    digest.update([0]);
    digest.update(index_tree.as_bytes());
    digest.update([0]);
    digest.update(status);
    format!("sha256:{:x}", digest.finalize())
}

fn utf8(bytes: Vec<u8>) -> Result<String, GitRepositoryError> {
    String::from_utf8(bytes).map_err(|_| GitRepositoryError::NonUtf8Output)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<(Vec<u8>, bool), std::io::Error> {
    let mut kept = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0u8; 16 * 1024];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(kept.len());
        let take = remaining.min(count);
        kept.extend_from_slice(&buffer[..take]);
        truncated |= take < count;
    }
    Ok((kept, truncated))
}

fn resolve_binary(binary: &str) -> Option<PathBuf> {
    let candidate = Path::new(binary);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|entry| entry.join(binary))
        .find(|path| path.is_file())
}

fn repository_lock(key: &Path) -> Arc<RwLock<()>> {
    let locks = GIT_REPOSITORY_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut locks = match locks.lock() {
        Ok(locks) => locks,
        Err(poisoned) => poisoned.into_inner(),
    };
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(RwLock::new(()));
    locks.insert(key.to_path_buf(), Arc::downgrade(&lock));
    lock
}

fn read_repository(lock: &RwLock<()>) -> std::sync::RwLockReadGuard<'_, ()> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_repository(lock: &RwLock<()>) -> std::sync::RwLockWriteGuard<'_, ()> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

    fn git(repo: &Path, args: &[&str]) -> TestResult {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into())
    }

    fn initialized_repository() -> Result<tempfile::TempDir, Box<dyn Error + Send + Sync>> {
        let directory = tempfile::tempdir()?;
        git(directory.path(), &["init", "--initial-branch=main"])?;
        git(
            directory.path(),
            &["config", "user.email", "git-test@example.invalid"],
        )?;
        git(
            directory.path(),
            &["config", "user.name", "Taugentic Git Test"],
        )?;
        fs::write(directory.path().join("tracked.txt"), "base\n")?;
        git(directory.path(), &["add", "--", "tracked.txt"])?;
        git(directory.path(), &["commit", "-m", "initial"])?;
        Ok(directory)
    }

    #[test]
    fn status_parser_preserves_special_paths_and_staging_split() {
        let bytes = b"# branch.oid abcdef\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\01 M. N... 100644 100644 100644 a b space name.txt\02 R. N... 100644 100644 100644 a b R100 renamed.txt\0old.txt\0? untracked.txt\0";
        let status = parse_status(bytes).expect("status should parse");
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.ahead, 2);
        assert_eq!(status.behind, 1);
        assert_eq!(status.files.len(), 3);
        assert_eq!(status.files[0].path, "renamed.txt");
        assert_eq!(status.files[0].original_path.as_deref(), Some("old.txt"));
        assert_eq!(status.files[1].path, "space name.txt");
        assert_eq!(status.files[2].unstaged, Some(GitChangeKind::Untracked));
    }

    #[test]
    fn path_validation_rejects_scope_escape() {
        assert!(validate_paths(&["src/main.rs".to_string()]).is_ok());
        assert!(validate_paths(&["../outside".to_string()]).is_err());
        assert!(validate_paths(&["/absolute".to_string()]).is_err());
    }

    #[test]
    fn git_push_arguments_are_exact_non_force_and_non_deleting() {
        let source = "1111111111111111111111111111111111111111";
        let network_remote = OsString::from("https://github.com/example/project.git");
        let args = push_arguments(&network_remote, "feature/review", source)
            .expect("push arguments should build")
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "push",
                "--porcelain",
                "https://github.com/example/project.git",
                "1111111111111111111111111111111111111111:refs/heads/feature/review",
            ]
        );
        assert!(!args.iter().any(|arg| arg.contains("force")));
        assert!(!args.last().expect("refspec").starts_with(':'));
    }

    #[test]
    fn git_push_authorization_exists_only_in_environment() {
        let token = ta_code_host::CodeHostAccessToken::new("test-token-value")
            .expect("token should be valid");
        let authorization =
            GitHttpsAuthorization::github("https://github.com", "example-user", &token)
                .expect("authorization should build");
        let network_remote = OsString::from("https://github.com/example/project.git");
        let args = push_arguments(
            &network_remote,
            "main",
            "1111111111111111111111111111111111111111",
        )
        .expect("push arguments should build");
        assert!(
            args.iter()
                .all(|arg| !arg.to_string_lossy().contains(token.expose_secret()))
        );
        let env = authenticated_git_env(&authorization);
        assert_eq!(
            env.iter()
                .find(|(key, _)| *key == "GIT_TERMINAL_PROMPT")
                .map(|(_, value)| value.to_string_lossy().into_owned())
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            env.iter()
                .find(|(key, _)| *key == "GIT_CONFIG_COUNT")
                .map(|(_, value)| value.to_string_lossy().into_owned())
                .as_deref(),
            Some("1")
        );
    }

    #[test]
    fn bounded_reader_drains_but_retains_only_the_limit() {
        let input = vec![b'x'; 128];
        let (kept, truncated) = read_bounded(input.as_slice(), 32).expect("bounded read");
        assert_eq!(kept.len(), 32);
        assert!(truncated);
    }

    #[test]
    fn checkpoint_restore_preserves_staged_unstaged_and_untracked_state() -> TestResult {
        let directory = initialized_repository()?;
        let repository = GitRepository::open(directory.path())?;
        let initial_head = repository.head()?.expect("initialized repository has HEAD");

        fs::write(directory.path().join("tracked.txt"), "staged\n")?;
        repository.stage_paths(&["tracked.txt".to_string()])?;
        fs::write(directory.path().join("tracked.txt"), "staged\nunstaged\n")?;
        fs::write(directory.path().join("untracked file.txt"), "checkpoint\n")?;
        let checkpoint = repository.capture_checkpoint("roundtrip")?;

        fs::write(directory.path().join("tracked.txt"), "later\n")?;
        fs::remove_file(directory.path().join("untracked file.txt"))?;
        fs::write(directory.path().join("later.txt"), "remove me\n")?;

        let restored =
            repository.restore_checkpoint(&checkpoint.staged_commit, &checkpoint.full_commit)?;
        assert_eq!(repository.head()?.as_deref(), Some(initial_head.as_str()));
        assert_eq!(
            fs::read_to_string(directory.path().join("tracked.txt"))?,
            "staged\nunstaged\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("untracked file.txt"))?,
            "checkpoint\n"
        );
        assert!(!directory.path().join("later.txt").exists());

        let tracked = restored
            .files
            .iter()
            .find(|file| file.path == "tracked.txt")
            .expect("tracked file status");
        assert_eq!(tracked.staged, Some(GitChangeKind::Modified));
        assert_eq!(tracked.unstaged, Some(GitChangeKind::Modified));
        let untracked = restored
            .files
            .iter()
            .find(|file| file.path == "untracked file.txt")
            .expect("untracked file status");
        assert_eq!(untracked.unstaged, Some(GitChangeKind::Untracked));

        let reference = format!("refs/taugentic/checkpoints/roundtrip");
        git(directory.path(), &["show-ref", "--verify", &reference])?;
        Ok(())
    }

    #[test]
    fn stage_and_unstage_accept_special_relative_paths() -> TestResult {
        let directory = initialized_repository()?;
        let repository = GitRepository::open(directory.path())?;
        fs::write(directory.path().join("-leading dash.txt"), "dash\n")?;
        fs::write(directory.path().join("space name.txt"), "space\n")?;
        let paths = vec![
            "-leading dash.txt".to_string(),
            "space name.txt".to_string(),
        ];

        repository.stage_paths(&paths)?;
        let staged = repository.snapshot()?;
        assert!(
            staged
                .files
                .iter()
                .filter(|file| file.staged.is_some())
                .count()
                >= 2,
            "both special paths should be staged"
        );

        repository.unstage_paths(&paths)?;
        let unstaged = repository.snapshot()?;
        assert!(
            unstaged
                .files
                .iter()
                .filter(|file| file.unstaged == Some(GitChangeKind::Untracked))
                .count()
                >= 2,
            "both special paths should return to untracked state"
        );
        Ok(())
    }

    #[test]
    fn repository_lock_is_shared_by_linked_worktrees_only() -> TestResult {
        let directory = initialized_repository()?;
        let linked_parent = tempfile::tempdir()?;
        let linked_path = linked_parent.path().join("linked");
        let linked_path_text = linked_path
            .to_str()
            .ok_or("linked worktree path must be valid UTF-8")?;
        git(
            directory.path(),
            &["worktree", "add", "-b", "linked-lock", linked_path_text],
        )?;
        let first = GitRepository::open(directory.path())?;
        let second = GitRepository::open(&linked_path)?;

        let unrelated_directory = initialized_repository()?;
        let unrelated = GitRepository::open(unrelated_directory.path())?;

        assert!(Arc::ptr_eq(&first.lock, &second.lock));
        assert!(!Arc::ptr_eq(&first.lock, &unrelated.lock));
        Ok(())
    }

    #[test]
    fn git_push_is_two_phase_state_bound_and_updates_exact_branch() -> TestResult {
        let directory = initialized_repository()?;
        let bare = tempfile::tempdir()?;
        git(bare.path(), &["init", "--bare"])?;
        let remote_url = "https://github.com/example-owner/example-project.git";
        git(directory.path(), &["remote", "add", "origin", remote_url])?;
        let network_remote = format!("file://{}", bare.path().display());
        git(directory.path(), &["push", &network_remote, "main"])?;
        fs::write(directory.path().join("tracked.txt"), "ahead\n")?;
        git(directory.path(), &["add", "--", "tracked.txt"])?;
        git(directory.path(), &["commit", "-m", "ahead"])?;

        let mut repository = GitRepository::open(directory.path())?;
        repository.network_remote_override = Some(network_remote);
        let token = ta_code_host::CodeHostAccessToken::new("test-token-value")?;
        let authorization =
            GitHttpsAuthorization::github("https://github.com", "example-user", &token)?;
        let stale_preview =
            repository.push_preflight("origin", "main", remote_url, &authorization, 10)?;
        fs::write(directory.path().join("later.txt"), "later\n")?;
        git(directory.path(), &["add", "--", "later.txt"])?;
        git(directory.path(), &["commit", "-m", "later"])?;
        assert!(matches!(
            repository.push_non_force_if_unchanged(
                "origin",
                "main",
                remote_url,
                &stale_preview.source_head,
                stale_preview.remote_head.as_deref(),
                remote_url,
                &authorization,
            ),
            Err(GitRepositoryError::RemoteChanged)
        ));

        let preview =
            repository.push_preflight("origin", "main", remote_url, &authorization, 10)?;
        assert_eq!(preview.remote_url, remote_url);
        assert_eq!(preview.commits.len(), 2);
        assert_eq!(preview.commits[0].subject, "ahead");
        assert_eq!(preview.commits[1].subject, "later");
        let prior_remote = preview.remote_head.clone().expect("remote head");

        repository.push_non_force_if_unchanged(
            "origin",
            "main",
            remote_url,
            &preview.source_head,
            Some(&prior_remote),
            remote_url,
            &authorization,
        )?;
        let remote_head = Command::new("git")
            .arg("--git-dir")
            .arg(bare.path())
            .args(["rev-parse", "refs/heads/main"])
            .output()?;
        assert!(remote_head.status.success());
        assert_eq!(
            String::from_utf8(remote_head.stdout)?.trim(),
            preview.source_head
        );
        assert!(matches!(
            repository.push_non_force_if_unchanged(
                "origin",
                "main",
                remote_url,
                &preview.source_head,
                Some(&prior_remote),
                remote_url,
                &authorization,
            ),
            Err(GitRepositoryError::RemoteChanged)
        ));
        Ok(())
    }

    #[test]
    fn git_push_preflight_rejects_non_fast_forward_remote() -> TestResult {
        let directory = initialized_repository()?;
        let bare = tempfile::tempdir()?;
        git(bare.path(), &["init", "--bare"])?;
        let remote_url = "https://github.com/example-owner/example-project.git";
        git(directory.path(), &["remote", "add", "origin", remote_url])?;
        let network_remote = format!("file://{}", bare.path().display());
        git(directory.path(), &["push", &network_remote, "main"])?;

        let other = tempfile::tempdir()?;
        let bare_path = bare.path().to_str().ok_or("bare path should be utf8")?;
        git(other.path(), &["clone", "--branch", "main", bare_path, "."])?;
        git(
            other.path(),
            &["config", "user.email", "git-test@example.invalid"],
        )?;
        git(other.path(), &["config", "user.name", "Taugentic Git Test"])?;
        fs::write(other.path().join("remote.txt"), "remote\n")?;
        git(other.path(), &["add", "--", "remote.txt"])?;
        git(other.path(), &["commit", "-m", "remote divergence"])?;
        git(other.path(), &["push", "origin", "main"])?;

        fs::write(directory.path().join("local.txt"), "local\n")?;
        git(directory.path(), &["add", "--", "local.txt"])?;
        git(directory.path(), &["commit", "-m", "local divergence"])?;
        let mut repository = GitRepository::open(directory.path())?;
        repository.network_remote_override = Some(network_remote);
        let token = ta_code_host::CodeHostAccessToken::new("test-token-value")?;
        let authorization =
            GitHttpsAuthorization::github("https://github.com", "example-user", &token)?;
        assert!(matches!(
            repository.push_preflight("origin", "main", remote_url, &authorization, 10),
            Err(GitRepositoryError::NonFastForward)
        ));
        Ok(())
    }

    #[test]
    fn git_push_preflight_accepts_an_absent_destination_branch() -> TestResult {
        let directory = initialized_repository()?;
        let bare = tempfile::tempdir()?;
        git(bare.path(), &["init", "--bare"])?;
        let remote_url = "https://github.com/example-owner/example-project.git";
        git(directory.path(), &["remote", "add", "origin", remote_url])?;
        let network_remote = format!("file://{}", bare.path().display());
        fs::write(directory.path().join("ahead.txt"), "ahead\n")?;
        git(directory.path(), &["add", "--", "ahead.txt"])?;
        git(directory.path(), &["commit", "-m", "ahead"])?;

        let mut repository = GitRepository::open(directory.path())?;
        repository.network_remote_override = Some(network_remote);
        let token = ta_code_host::CodeHostAccessToken::new("test-token-value")?;
        let authorization =
            GitHttpsAuthorization::github("https://github.com", "example-user", &token)?;

        let preflight =
            repository.push_preflight("origin", "new-branch", remote_url, &authorization, 10)?;

        assert_eq!(preflight.remote_head, None);
        assert_eq!(preflight.commits.len(), 2);
        assert_eq!(preflight.commits[0].subject, "initial");
        assert_eq!(preflight.commits[1].subject, "ahead");
        assert!(!preflight.truncated);
        Ok(())
    }
}
