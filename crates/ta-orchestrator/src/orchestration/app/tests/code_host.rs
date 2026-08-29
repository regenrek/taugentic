use std::process::Command;

use ta_protocol::wire::{
    CodeHostAccount, CodeHostAccountId, CodeHostAccountListParams, CodeHostProviderKind,
    CodeHostPushPrepareParams, CodeHostRepositoryContextParams, RunStatus, WorkspacePath,
};
use ta_store::{CodeHostAccountProjection, CodeHostAccountRepository};

use super::*;

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

fn account(id: &str, owner_principal_id: &str, name: &str) -> CodeHostAccountProjection {
    CodeHostAccountProjection {
        owner_principal_id: owner_principal_id.to_string(),
        account: CodeHostAccount {
            id: CodeHostAccountId::new(id).expect("account id"),
            provider: CodeHostProviderKind::GitHub,
            display_name: name.to_string(),
            account_login: format!("login-{id}"),
            host: "github.com".to_string(),
        },
    }
}

#[test]
fn code_host_accounts_are_principal_scoped_without_default_inference() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    {
        let mut store = service.store.lock().expect("app store");
        store
            .save_code_host_account(account("account-a", TEST_OWNER_PRINCIPAL_ID, "Profile A"))
            .expect("first account should seed");
        store
            .save_code_host_account(account(
                "account-b",
                OTHER_TEST_OWNER_PRINCIPAL_ID,
                "Profile B",
            ))
            .expect("second account should seed");
    }
    let visible = service
        .code_host_accounts(TEST_OWNER_PRINCIPAL_ID, &CodeHostAccountListParams {})
        .expect("accounts should list");
    assert_eq!(visible.accounts.len(), 1);
    assert_eq!(visible.accounts[0].id.as_str(), "account-a");
}

#[test]
fn repository_context_is_project_scoped_and_filters_unsupported_remotes() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("repository tempdir");
    git(root.path(), &["init", "--initial-branch=main"]);
    git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example-owner/example-project.git",
        ],
    );
    git(
        root.path(),
        &[
            "remote",
            "add",
            "other",
            "https://example.invalid/example-owner/example-project.git",
        ],
    );
    let (project_id, snapshot) = service
        .open_project(
            TEST_OWNER_PRINCIPAL_ID,
            WorkspacePath::canonicalize_existing(root.path()).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot.projects[0].workspace_ids[0].clone();
    let params = CodeHostRepositoryContextParams {
        project_id,
        workspace_id,
    };
    let context = service
        .code_host_repository_context(TEST_OWNER_PRINCIPAL_ID, &params)
        .expect("repository context should load");
    assert_eq!(context.remotes.len(), 1);
    assert_eq!(context.remotes[0].remote_name, "origin");
    assert!(matches!(
        service.code_host_repository_context(OTHER_TEST_OWNER_PRINCIPAL_ID, &params),
        Err(AppServiceError::ProjectNotFound(_))
    ));
}

#[test]
fn push_prepare_rejects_active_workspace_before_credential_resolution() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("repository tempdir");
    git(root.path(), &["init", "--initial-branch=main"]);
    git(
        root.path(),
        &["config", "user.email", "code-host-test@example.invalid"],
    );
    git(root.path(), &["config", "user.name", "Code Host Test"]);
    std::fs::write(root.path().join("tracked.txt"), "base\n").expect("tracked file");
    git(root.path(), &["add", "--", "tracked.txt"]);
    git(root.path(), &["commit", "-m", "initial"]);
    git(
        root.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example-owner/example-project.git",
        ],
    );
    let (project_id, snapshot) = service
        .open_project(
            TEST_OWNER_PRINCIPAL_ID,
            WorkspacePath::canonicalize_existing(root.path()).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot.projects[0].workspace_ids[0].clone();
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Active code-host run".to_string(),
                workspace_id: workspace_id.clone(),
            },
        )
        .expect("session should open")
        .session;
    let mut run = native_run_projection("run-code-host-active", &session.id, RunStatus::Running, 1);
    run.execution_context.workspace_id = workspace_id.clone();
    run.execution_context.effective_cwd =
        WorkspacePath::canonicalize_existing(root.path()).expect("run cwd");
    seed_run_projection(&service, run);

    assert!(matches!(
        service.prepare_code_host_push(
            TEST_OWNER_PRINCIPAL_ID,
            &CodeHostPushPrepareParams {
                project_id,
                workspace_id,
                account_id: CodeHostAccountId::new("missing-account").expect("account id"),
                remote_name: "origin".to_string(),
                destination_branch: "main".to_string(),
            },
        ),
        Err(AppServiceError::CodeHostWorkspaceRunActive)
    ));
}
