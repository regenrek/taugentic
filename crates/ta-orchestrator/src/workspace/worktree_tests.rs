use super::*;
use std::error::Error;
use std::process::{Command, Output};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

fn init_repo() -> Result<TempDir, Box<dyn Error + Send + Sync>> {
    let repo = tempfile::tempdir()?;
    git(repo.path(), ["init"])?;
    git(repo.path(), ["config", "user.email", "agent@example.test"])?;
    git(repo.path(), ["config", "user.name", "Agent Test"])?;
    fs::write(repo.path().join(".gitignore"), "target/\n")?;
    fs::write(repo.path().join("README.md"), "# test\n")?;
    git(repo.path(), ["add", ".gitignore", "README.md"])?;
    git(repo.path(), ["commit", "-m", "initial"])?;
    Ok(repo)
}

fn git<'a, I>(repo: &Path, args: I) -> Result<Output, Box<dyn Error + Send + Sync>>
where
    I: IntoIterator<Item = &'a str>,
{
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if output.status.success() {
        return Ok(output);
    }

    Err(format!(
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

fn request(repo: &Path, id: &str, cleanup_policy: CleanupPolicy) -> WorktreeRequest {
    WorktreeRequest {
        parent_repo: repo.to_path_buf(),
        capsule_short_id: id.to_string(),
        recipe_hint: None,
        cleanup_policy,
    }
}

fn branch_exists(repo: &Path, branch: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let output = git(repo, ["branch", "--list", branch])?;
    Ok(!output.stdout.is_empty())
}

#[cfg(windows)]
#[test]
fn git_path_arg_removes_windows_verbatim_prefixes() {
    assert_eq!(
        git_path_arg(Path::new(r"\\?\C:\Users\agent\repo")),
        r"C:\Users\agent\repo"
    );
    assert_eq!(
        git_path_arg(Path::new(r"\\?\UNC\server\share\repo")),
        r"\\server\share\repo"
    );
}

fn create_handle(
    manager: &WorktreeManager,
    repo: &Path,
    id: &str,
    policy: CleanupPolicy,
) -> Result<WorktreeHandle, WorktreeError> {
    manager.create(request(repo, id, policy))
}

#[test]
fn create_roundtrip_lists_worktree_and_branch() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(&manager, repo.path(), "abc123", CleanupPolicy::Keep)?;

    assert!(handle.path().exists());
    assert_eq!(handle.branch(), "ta/capsule-abc123");
    assert!(branch_exists(repo.path(), handle.branch())?);

    let records = manager.list(repo.path())?;
    assert!(records.iter().any(|record| record.path == handle.path()));
    assert!(
        records
            .iter()
            .any(|record| record.branch.as_deref() == Some(handle.branch()))
    );
    Ok(())
}

#[test]
fn scheduled_resource_reattach_uses_exact_published_worktree_identity() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(
        &manager,
        repo.path(),
        "scheduled-reattach",
        CleanupPolicy::Keep,
    )?;
    let path = handle.path().to_path_buf();
    let branch = handle.branch().to_string();
    drop(handle);

    let reattached =
        WorktreeManager::new()?.reattach(repo.path(), &path, &branch, CleanupPolicy::Keep)?;

    assert_eq!(reattached.path(), path);
    assert_eq!(reattached.branch(), branch);
    reattached.cleanup()?;
    Ok(())
}

#[test]
fn multiple_parallel_worktrees_can_coexist() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handles = ["one", "two", "three"]
        .into_iter()
        .map(|id| create_handle(&manager, repo.path(), id, CleanupPolicy::Keep))
        .collect::<Result<Vec<_>, _>>()?;

    for handle in &handles {
        assert!(handle.path().exists());
        assert!(branch_exists(repo.path(), handle.branch())?);
    }
    Ok(())
}

#[test]
fn delete_on_success_drop_deletes_after_success() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(
        &manager,
        repo.path(),
        "success",
        CleanupPolicy::DeleteOnSuccess,
    )?;
    let path = handle.path().to_path_buf();

    handle.mark_success();
    drop(handle);

    assert!(!path.exists());
    assert!(!branch_exists(repo.path(), "ta/capsule-success")?);
    Ok(())
}

#[test]
fn delete_on_success_drop_preserves_after_failure() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(
        &manager,
        repo.path(),
        "failed",
        CleanupPolicy::DeleteOnSuccess,
    )?;
    let path = handle.path().to_path_buf();

    handle.mark_failed();
    drop(handle);

    assert!(path.exists());
    assert!(branch_exists(repo.path(), "ta/capsule-failed")?);
    Ok(())
}

#[test]
fn delete_on_terminal_drop_deletes_after_failure() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(
        &manager,
        repo.path(),
        "terminal",
        CleanupPolicy::DeleteOnTerminal,
    )?;
    let path = handle.path().to_path_buf();

    handle.mark_failed();
    drop(handle);

    assert!(!path.exists());
    assert!(!branch_exists(repo.path(), "ta/capsule-terminal")?);
    Ok(())
}

#[test]
fn keep_drop_preserves_worktree() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(&manager, repo.path(), "keep", CleanupPolicy::Keep)?;
    let path = handle.path().to_path_buf();

    handle.mark_success();
    drop(handle);

    assert!(path.exists());
    assert!(branch_exists(repo.path(), "ta/capsule-keep")?);
    Ok(())
}

#[test]
fn manual_explicit_cleanup_deletes_worktree() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(&manager, repo.path(), "manual", CleanupPolicy::Manual)?;
    let path = handle.path().to_path_buf();

    handle.cleanup()?;

    assert!(!path.exists());
    assert!(!branch_exists(repo.path(), "ta/capsule-manual")?);
    Ok(())
}

#[test]
fn create_rejects_non_git_directory() -> TestResult {
    let repo = tempfile::tempdir()?;
    let manager = WorktreeManager::new()?;

    let error = manager
        .create(request(repo.path(), "abc123", CleanupPolicy::Keep))
        .expect_err("non-git directory should fail");

    assert!(matches!(error, WorktreeError::NotARepo { .. }));
    Ok(())
}

#[test]
fn create_rejects_existing_worktree_path() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let parent_repo = fs::canonicalize(repo.path())?;
    let path = manager.worktree_path(&parent_repo, "existing");
    fs::create_dir_all(&path)?;

    let error = manager
        .create(request(repo.path(), "existing", CleanupPolicy::Keep))
        .expect_err("existing path should fail");

    assert!(matches!(error, WorktreeError::PathExists { .. }));
    Ok(())
}

#[test]
fn create_rejects_existing_branch() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    git(repo.path(), ["branch", "ta/capsule-dup"])?;

    let error = manager
        .create(request(repo.path(), "dup", CleanupPolicy::Keep))
        .expect_err("existing branch should fail");

    assert!(matches!(error, WorktreeError::BranchExists { .. }));
    Ok(())
}

#[test]
fn create_reports_missing_git_binary() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::with_git_binary(PathBuf::from("/nonexistent/git"));

    let error = manager
        .create(request(repo.path(), "abc123", CleanupPolicy::Keep))
        .expect_err("missing git should fail");

    assert!(matches!(error, WorktreeError::GitNotFound(_)));
    Ok(())
}

#[test]
fn cleanup_orphans_reports_capsule_worktrees() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let leaked = create_handle(&manager, repo.path(), "leaked", CleanupPolicy::Keep)?;
    let leaked_path = leaked.path().to_path_buf();
    let managed = create_handle(&manager, repo.path(), "managed", CleanupPolicy::Manual)?;

    std::mem::forget(leaked);
    managed.cleanup()?;

    let orphans = manager.cleanup_orphans(repo.path())?;
    assert!(orphans.contains(&leaked_path));
    assert!(!orphans.iter().any(|path| path.ends_with("managed")));
    Ok(())
}

#[test]
fn drop_never_panics_when_cleanup_fails() -> TestResult {
    let repo = init_repo()?;
    let manager = WorktreeManager::new()?;
    let handle = create_handle(
        &manager,
        repo.path(),
        "panic-free",
        CleanupPolicy::DeleteOnSuccess,
    )?;
    fs::remove_dir_all(handle.path())?;
    handle.mark_success();

    let result = std::panic::catch_unwind(|| drop(handle));
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn concurrent_create_succeeds_for_distinct_capsules() -> TestResult {
    let repo = init_repo()?;
    let git_binary = WorktreeManager::new()?.git_binary;
    let threads = (0..5)
        .map(|index| {
            let repo = repo.path().to_path_buf();
            let git_binary = git_binary.clone();
            std::thread::spawn(move || {
                let manager = WorktreeManager::with_git_binary(git_binary);
                manager.create(request(
                    &repo,
                    &format!("parallel-{index}"),
                    CleanupPolicy::Keep,
                ))
            })
        })
        .collect::<Vec<_>>();

    let mut handles = Vec::new();
    for thread in threads {
        let result = thread
            .join()
            .map_err(|_| "worktree create thread panicked")?;
        handles.push(result?);
    }

    for handle in &handles {
        assert!(handle.path().exists());
        assert!(branch_exists(repo.path(), handle.branch())?);
    }
    Ok(())
}
