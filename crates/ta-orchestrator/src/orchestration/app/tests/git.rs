use std::process::Command;

use ta_protocol::wire::{
    GitCheckpointApplyRevertParams, GitCheckpointListParams, GitCheckpointPhase,
    GitCheckpointPrepareRevertParams, GitCommitParams, GitDiffParams, GitDiffScope,
    GitPathsMutationParams, GitRepositorySnapshotParams, WorkspacePath,
};
use ta_store::{CheckpointRecord, CommitCheckpointPersist};

use super::*;
use crate::workspace::git::GitRepository;
use crate::workspace::git::GitRepositoryError;

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git should execute");
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn repository_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("repository tempdir");
    git(root.path(), &["init", "--initial-branch=main"]);
    git(
        root.path(),
        &["config", "user.email", "app-git-test@example.invalid"],
    );
    git(
        root.path(),
        &["config", "user.name", "Taugentic App Git Test"],
    );
    std::fs::write(root.path().join("tracked.txt"), "base\n").expect("tracked file");
    git(root.path(), &["add", "--", "tracked.txt"]);
    git(root.path(), &["commit", "-m", "initial"]);
    root
}

#[test]
fn git_command_failures_do_not_cross_the_app_boundary_with_process_output() {
    let error = crate::orchestration::app::git::map_git_error(GitRepositoryError::CommandFailed {
        context: "git status".to_string(),
        detail: "fatal: private remote and credential context".to_string(),
    });

    assert_eq!(error.to_string(), "git operation failed: git status");
}

fn open_project_fixture(
    service: &AppService,
    root: &std::path::Path,
) -> (crate::ProjectId, crate::WorkspaceId) {
    let (project_id, snapshot) = service
        .open_project(
            TEST_OWNER_PRINCIPAL_ID,
            WorkspacePath::canonicalize_existing(root).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .and_then(|project| project.workspace_ids.first())
        .cloned()
        .expect("project workspace");
    (project_id, workspace_id)
}

#[test]
fn git_status_stage_unstage_commit_and_diff_are_project_scoped() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = repository_fixture();
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    std::fs::write(root.path().join("tracked.txt"), "changed\n").expect("tracked change");

    let params = GitRepositorySnapshotParams {
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
    };
    let snapshot = service
        .git_repository_snapshot(TEST_OWNER_PRINCIPAL_ID, &params)
        .expect("status should load")
        .snapshot;
    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert_eq!(snapshot.files.len(), 1);
    assert!(snapshot.files[0].unstaged.is_some());
    assert!(matches!(
        service.git_repository_snapshot(OTHER_TEST_OWNER_PRINCIPAL_ID, &params),
        Err(AppServiceError::ProjectNotFound(_))
    ));

    let paths = GitPathsMutationParams {
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
        paths: vec!["tracked.txt".to_string()],
    };
    let staged = service
        .git_stage_paths(TEST_OWNER_PRINCIPAL_ID, &paths)
        .expect("stage should succeed");
    assert!(staged.snapshot.files[0].staged.is_some());
    let staged_diff = service
        .git_diff(
            TEST_OWNER_PRINCIPAL_ID,
            &GitDiffParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                scope: GitDiffScope::Staged,
            },
        )
        .expect("staged diff should load");
    assert!(staged_diff.patch.contains("+changed"));

    service
        .git_unstage_paths(TEST_OWNER_PRINCIPAL_ID, &paths)
        .expect("unstage should succeed");
    service
        .git_stage_paths(TEST_OWNER_PRINCIPAL_ID, &paths)
        .expect("restage should succeed");
    let committed = service
        .git_commit(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCommitParams {
                project_id,
                workspace_id,
                message: "commit through application owner".to_string(),
            },
        )
        .expect("commit should succeed");
    assert!(committed.commit.is_some());
    assert!(committed.snapshot.files.is_empty());
}

#[test]
fn checkpoint_revert_is_exact_two_phase_one_shot_and_state_bound() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = repository_fixture();
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let repository = GitRepository::open(root.path()).expect("git repository");

    std::fs::write(root.path().join("tracked.txt"), "checkpoint staged\n")
        .expect("staged checkpoint content");
    repository
        .stage_paths(&["tracked.txt".to_string()])
        .expect("checkpoint stage");
    std::fs::write(
        root.path().join("tracked.txt"),
        "checkpoint staged\ncheckpoint unstaged\n",
    )
    .expect("unstaged checkpoint content");
    std::fs::write(root.path().join("checkpoint untracked.txt"), "checkpoint\n")
        .expect("untracked checkpoint content");
    let captured = repository
        .capture_checkpoint("checkpoint-app-test")
        .expect("checkpoint capture");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Git checkpoint test".to_string(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("session should open")
        .session;
    let mut run = native_run_projection(
        "run-git-checkpoint-test",
        &session.id,
        RunStatus::Completed,
        1,
    );
    run.execution_context.workspace_id = workspace_id.clone();
    run.execution_context.effective_cwd =
        WorkspacePath::canonicalize_existing(root.path()).expect("run cwd");
    let run_id = run.id.clone();
    seed_run_projection(&service, run);
    let checkpoint = CheckpointRecord {
        checkpoint_id: "checkpoint-app-test".to_string(),
        workspace_id: workspace_id.clone(),
        run_id,
        revision: 0,
        phase: GitCheckpointPhase::BeforeTurn,
        base_head: captured.base_head,
        staged_commit: captured.staged_commit,
        full_commit: captured.full_commit,
        fingerprint: captured.fingerprint,
        created_at_ms: 1,
    };
    service
        .store
        .lock()
        .expect("app store")
        .commit_checkpoint_persist(CommitCheckpointPersist {
            checkpoint,
            occurred_at_ms: 1,
        })
        .expect("checkpoint should persist");

    std::fs::write(root.path().join("tracked.txt"), "later\n").expect("later content");
    std::fs::remove_file(root.path().join("checkpoint untracked.txt"))
        .expect("remove prior untracked");
    std::fs::write(root.path().join("later.txt"), "remove during revert\n")
        .expect("later untracked");

    let listed = service
        .git_checkpoints(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointListParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("checkpoint list");
    assert_eq!(listed.checkpoints.len(), 1);

    let prepare_params = GitCheckpointPrepareRevertParams {
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
        checkpoint_id: "checkpoint-app-test".to_string(),
    };
    let stale = service
        .git_prepare_checkpoint_revert(TEST_OWNER_PRINCIPAL_ID, &prepare_params)
        .expect("first preview should prepare");
    assert!(stale.patch.contains("later.txt"));
    std::fs::write(
        root.path().join("state-changed.txt"),
        "changed after preview\n",
    )
    .expect("state change");
    assert!(matches!(
        service.git_apply_checkpoint_revert(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointApplyRevertParams {
                token: stale.token.clone()
            }
        ),
        Err(AppServiceError::GitRevertStateChanged)
    ));
    assert!(matches!(
        service.git_apply_checkpoint_revert(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointApplyRevertParams { token: stale.token }
        ),
        Err(AppServiceError::GitRevertTokenInvalid)
    ));

    let active_run_prepared = service
        .git_prepare_checkpoint_revert(TEST_OWNER_PRINCIPAL_ID, &prepare_params)
        .expect("active-run preview should prepare");
    let mut active_run = native_run_projection(
        "run-git-checkpoint-active",
        &session.id,
        RunStatus::Running,
        2,
    );
    active_run.execution_context.workspace_id = workspace_id.clone();
    active_run.execution_context.effective_cwd =
        WorkspacePath::canonicalize_existing(root.path()).expect("active run cwd");
    seed_run_projection(&service, active_run.clone());
    assert!(matches!(
        service.git_apply_checkpoint_revert(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointApplyRevertParams {
                token: active_run_prepared.token.clone()
            }
        ),
        Err(AppServiceError::GitWorkspaceRunActive)
    ));
    assert!(matches!(
        service.git_apply_checkpoint_revert(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointApplyRevertParams {
                token: active_run_prepared.token
            }
        ),
        Err(AppServiceError::GitRevertTokenInvalid)
    ));
    active_run.status = RunStatus::Completed;
    seed_run_projection(&service, active_run);

    let prepared = service
        .git_prepare_checkpoint_revert(TEST_OWNER_PRINCIPAL_ID, &prepare_params)
        .expect("second preview should prepare");
    service
        .git_apply_checkpoint_revert(
            TEST_OWNER_PRINCIPAL_ID,
            &GitCheckpointApplyRevertParams {
                token: prepared.token,
            },
        )
        .expect("confirmed exact revert should apply");
    assert_eq!(
        std::fs::read_to_string(root.path().join("tracked.txt")).expect("restored tracked"),
        "checkpoint staged\ncheckpoint unstaged\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("checkpoint untracked.txt"))
            .expect("restored untracked"),
        "checkpoint\n"
    );
    assert!(!root.path().join("later.txt").exists());
    assert!(!root.path().join("state-changed.txt").exists());
}
