use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use ta_code_host::{
    CodeHostAccessToken, CodeHostCredentialStore, CodeHostError, GitHubClient,
    github_https_repository_url, github_repository_from_remote_url,
};
use ta_protocol::wire::{
    CodeHostAccount, CodeHostAccountConnectParams, CodeHostAccountConnectResult,
    CodeHostAccountDisconnectParams, CodeHostAccountDisconnectResult, CodeHostAccountId,
    CodeHostAccountListParams, CodeHostAccountListResult, CodeHostPage, CodeHostProviderKind,
    CodeHostPullRequestActivityParams, CodeHostPullRequestActivityResult,
    CodeHostPullRequestChecksParams, CodeHostPullRequestChecksResult,
    CodeHostPullRequestCommentCreateParams, CodeHostPullRequestCommentCreateResult,
    CodeHostPullRequestDetail, CodeHostPullRequestDetailParams, CodeHostPullRequestEnsureParams,
    CodeHostPullRequestEnsureResult, CodeHostPullRequestListParams, CodeHostPushApplyParams,
    CodeHostPushApplyResult, CodeHostPushPrepareParams, CodeHostPushPrepareResult, CodeHostRemote,
    CodeHostRepositoryContextParams, CodeHostRepositoryContextResult, CodeHostRepositoryRef,
    ProjectId, WorkspaceId,
};
use ta_store::{CodeHostAccountProjection, PersistenceStore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AppService, AppServiceError, git::map_git_error};

const ACCOUNT_DISPLAY_NAME_MAX_BYTES: usize = 128;
const PUSH_TOKEN_TTL_MS: u64 = 5 * 60 * 1000;
const PUSH_COMMIT_LIST_LIMIT: usize = 100;

#[derive(Clone, Default)]
pub(crate) struct CodeHostPushRuntime {
    pub(super) prepared: Arc<Mutex<BTreeMap<String, PreparedPush>>>,
}

impl CodeHostPushRuntime {
    fn take(
        &self,
        owner_principal_id: &str,
        token: &str,
        now_ms: u64,
    ) -> Result<PreparedPush, AppServiceError> {
        let prepared = self
            .prepared
            .lock()
            .expect("code-host push runtime should not be poisoned")
            .remove(token)
            .ok_or(AppServiceError::CodeHostPushTokenInvalid)?;
        if prepared.owner_principal_id != owner_principal_id || now_ms > prepared.expires_at_ms {
            return Err(AppServiceError::CodeHostPushTokenInvalid);
        }
        Ok(prepared)
    }
}

#[derive(Clone)]
pub(super) struct PreparedPush {
    pub owner_principal_id: String,
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub repository_root: PathBuf,
    pub account_id: CodeHostAccountId,
    pub remote: CodeHostRemote,
    pub remote_url: String,
    pub source_head: String,
    pub destination_branch: String,
    pub remote_head: Option<String>,
    pub expires_at_ms: u64,
}

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn code_host_accounts(
        &self,
        owner_principal_id: &str,
        _params: &CodeHostAccountListParams,
    ) -> Result<CodeHostAccountListResult, AppServiceError> {
        let owner_principal_id = super::sanitize_session_owner_principal_id(owner_principal_id)?;
        let accounts = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .code_host_accounts()?
            .into_iter()
            .filter(|projection| projection.owner_principal_id == owner_principal_id)
            .map(|projection| projection.account)
            .collect();
        Ok(CodeHostAccountListResult { accounts })
    }

    pub async fn connect_code_host_account(
        &self,
        owner_principal_id: &str,
        params: &CodeHostAccountConnectParams,
    ) -> Result<CodeHostAccountConnectResult, AppServiceError> {
        let owner_principal_id = super::sanitize_session_owner_principal_id(owner_principal_id)?;
        let display_name = normalize_display_name(&params.display_name)?;
        let host = normalize_host(params.provider, &params.host)?;
        let token = CodeHostAccessToken::new(params.access_token.clone())?;
        let client = client_for(params.provider, &host)?;
        let identity = client
            .validate_account(&token, &CancellationToken::new())
            .await?;
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let duplicates = store.code_host_accounts()?.into_iter().any(|projection| {
            projection.owner_principal_id == owner_principal_id
                && (projection
                    .account
                    .display_name
                    .eq_ignore_ascii_case(&display_name)
                    || (projection.account.provider == params.provider
                        && projection.account.host.eq_ignore_ascii_case(&host)
                        && projection
                            .account
                            .account_login
                            .eq_ignore_ascii_case(&identity.login)))
        });
        if duplicates {
            return Err(AppServiceError::CodeHostAccountDisplayNameInvalid);
        }
        let account = CodeHostAccount {
            id: CodeHostAccountId::new(format!("code-host-account-{}", Uuid::new_v4().simple()))
                .expect("generated code-host account id should be valid"),
            provider: params.provider,
            display_name,
            account_login: identity.login,
            host,
        };
        let credentials = CodeHostCredentialStore::from_default_store()?;
        credentials.store(account.provider, &account.id, &token)?;
        if let Err(error) = store.save_code_host_account(CodeHostAccountProjection {
            owner_principal_id,
            account: account.clone(),
        }) {
            let _ = credentials.delete(account.provider, &account.id);
            return Err(error.into());
        }
        Ok(CodeHostAccountConnectResult { account })
    }

    pub fn disconnect_code_host_account(
        &self,
        owner_principal_id: &str,
        params: &CodeHostAccountDisconnectParams,
    ) -> Result<CodeHostAccountDisconnectResult, AppServiceError> {
        let projection =
            self.code_host_account_for_owner(owner_principal_id, &params.account_id)?;
        CodeHostCredentialStore::from_default_store()?
            .delete(projection.account.provider, &projection.account.id)?;
        let disconnected = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .remove_code_host_account(&params.account_id)?;
        Ok(CodeHostAccountDisconnectResult { disconnected })
    }

    pub fn code_host_repository_context(
        &self,
        owner_principal_id: &str,
        params: &CodeHostRepositoryContextParams,
    ) -> Result<CodeHostRepositoryContextResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let remotes = repository
            .remote_urls()
            .map_err(map_git_error)?
            .into_iter()
            .filter_map(|(remote_name, url)| {
                github_repository_from_remote_url(&url)
                    .ok()
                    .map(|repository| CodeHostRemote {
                        remote_name,
                        repository,
                    })
            })
            .collect();
        Ok(CodeHostRepositoryContextResult { remotes })
    }

    pub fn prepare_code_host_push(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPushPrepareParams,
    ) -> Result<CodeHostPushPrepareResult, AppServiceError> {
        let owner_principal_id = super::sanitize_session_owner_principal_id(owner_principal_id)?;
        if self.workspace_has_active_run(&params.workspace_id)? {
            return Err(AppServiceError::CodeHostWorkspaceRunActive);
        }
        let context = self.code_host_repository_context(
            &owner_principal_id,
            &CodeHostRepositoryContextParams {
                project_id: params.project_id.clone(),
                workspace_id: params.workspace_id.clone(),
            },
        )?;
        let remote = context
            .remotes
            .into_iter()
            .find(|remote| remote.remote_name == params.remote_name)
            .ok_or(AppServiceError::CodeHostRemoteNotFound)?;
        let (client, account, token) =
            self.client_and_token(&owner_principal_id, &params.account_id)?;
        if remote.repository.provider != CodeHostProviderKind::GitHub
            || !remote
                .repository
                .host
                .eq_ignore_ascii_case(client.web_origin().trim_start_matches("https://"))
        {
            return Err(AppServiceError::CodeHostRepositoryNotInWorkspace);
        }
        let authorization = ta_code_host::GitHttpsAuthorization::github(
            client.web_origin(),
            &account.account_login,
            &token,
        )?;
        let network_remote_url = github_https_repository_url(&remote.repository)?;
        let repository = self.project_git_repository(
            &owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let preflight = repository
            .push_preflight(
                &params.remote_name,
                &params.destination_branch,
                &network_remote_url,
                &authorization,
                PUSH_COMMIT_LIST_LIMIT,
            )
            .map_err(map_push_preflight_error)?;
        if github_repository_from_remote_url(&preflight.remote_url)? != remote.repository {
            return Err(AppServiceError::CodeHostPushStateChanged);
        }
        if preflight.commits.is_empty() {
            return Err(CodeHostError::Conflict.into());
        }
        let token = format!("code-host-push-{}", Uuid::new_v4().simple());
        self.code_host_pushes
            .prepared
            .lock()
            .expect("code-host push runtime should not be poisoned")
            .insert(
                token.clone(),
                PreparedPush {
                    owner_principal_id,
                    project_id: params.project_id.clone(),
                    workspace_id: params.workspace_id.clone(),
                    repository_root: repository.root().to_path_buf(),
                    account_id: params.account_id.clone(),
                    remote: remote.clone(),
                    remote_url: preflight.remote_url,
                    source_head: preflight.source_head.clone(),
                    destination_branch: params.destination_branch.clone(),
                    remote_head: preflight.remote_head.clone(),
                    expires_at_ms: current_time_ms().saturating_add(PUSH_TOKEN_TTL_MS),
                },
            );
        Ok(CodeHostPushPrepareResult {
            token,
            remote,
            source_head: preflight.source_head,
            destination_branch: params.destination_branch.clone(),
            remote_head: preflight.remote_head,
            commits: preflight.commits,
            truncated: preflight.truncated,
        })
    }

    pub fn apply_code_host_push(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPushApplyParams,
    ) -> Result<CodeHostPushApplyResult, AppServiceError> {
        let owner_principal_id = super::sanitize_session_owner_principal_id(owner_principal_id)?;
        let prepared =
            self.code_host_pushes
                .take(&owner_principal_id, &params.token, current_time_ms())?;
        if self.workspace_has_active_run(&prepared.workspace_id)? {
            return Err(AppServiceError::CodeHostWorkspaceRunActive);
        }
        let repository = self.project_git_repository(
            &owner_principal_id,
            &prepared.project_id,
            &prepared.workspace_id,
        )?;
        if repository.root() != prepared.repository_root {
            return Err(AppServiceError::CodeHostPushStateChanged);
        }
        let (client, account, token) =
            self.client_and_token(&owner_principal_id, &prepared.account_id)?;
        let authorization = ta_code_host::GitHttpsAuthorization::github(
            client.web_origin(),
            &account.account_login,
            &token,
        )?;
        let network_remote_url = github_https_repository_url(&prepared.remote.repository)?;
        repository
            .push_non_force_if_unchanged(
                &prepared.remote.remote_name,
                &prepared.destination_branch,
                &network_remote_url,
                &prepared.source_head,
                prepared.remote_head.as_deref(),
                &prepared.remote_url,
                &authorization,
            )
            .map_err(map_push_apply_error)?;
        Ok(CodeHostPushApplyResult {
            remote: prepared.remote,
            source_head: prepared.source_head,
            destination_branch: prepared.destination_branch,
        })
    }

    pub async fn list_code_host_pull_requests(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestListParams,
    ) -> Result<CodeHostPage, AppServiceError> {
        self.require_repository_scope(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
            &params.repository,
        )?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        let page = client
            .list_pull_requests(
                &token,
                &params.repository,
                params.cursor.as_deref(),
                params.limit,
                None,
                None,
                &CancellationToken::new(),
            )
            .await?;
        Ok(CodeHostPage {
            items: page.pull_requests,
            next_cursor: page.next_cursor,
        })
    }

    pub async fn code_host_pull_request_detail(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestDetailParams,
    ) -> Result<CodeHostPullRequestDetail, AppServiceError> {
        self.require_repository_scope(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
            &params.repository,
        )?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        Ok(client
            .pull_request_detail(
                &token,
                &params.repository,
                params.number,
                &CancellationToken::new(),
            )
            .await?)
    }

    pub async fn ensure_code_host_pull_request(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestEnsureParams,
    ) -> Result<CodeHostPullRequestEnsureResult, AppServiceError> {
        if self.workspace_has_active_run(&params.workspace_id)? {
            return Err(AppServiceError::CodeHostWorkspaceRunActive);
        }
        let context = self.code_host_repository_context(
            owner_principal_id,
            &CodeHostRepositoryContextParams {
                project_id: params.project_id.clone(),
                workspace_id: params.workspace_id.clone(),
            },
        )?;
        let head = remote_repository(&context.remotes, &params.head_remote_name)?;
        let base = remote_repository(&context.remotes, &params.base_remote_name)?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        Ok(client
            .ensure_pull_request(
                &token,
                base,
                head,
                &params.head_branch,
                &params.base_branch,
                &params.title,
                &params.body,
                params.draft,
                &CancellationToken::new(),
            )
            .await?)
    }

    pub async fn code_host_pull_request_checks(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestChecksParams,
    ) -> Result<CodeHostPullRequestChecksResult, AppServiceError> {
        self.require_repository_scope(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
            &params.repository,
        )?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        Ok(CodeHostPullRequestChecksResult {
            checks: client
                .pull_request_checks(
                    &token,
                    &params.repository,
                    &params.head_sha,
                    &CancellationToken::new(),
                )
                .await?,
        })
    }

    pub async fn code_host_pull_request_activity(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestActivityParams,
    ) -> Result<CodeHostPullRequestActivityResult, AppServiceError> {
        self.require_repository_scope(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
            &params.repository,
        )?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        let page = client
            .pull_request_activity(
                &token,
                &params.repository,
                params.number,
                params.cursor.as_deref(),
                params.limit,
                &CancellationToken::new(),
            )
            .await?;
        Ok(CodeHostPullRequestActivityResult {
            comments: page.comments,
            reviews: page.reviews,
            timeline: page.timeline,
            next_cursor: page.next_cursor,
        })
    }

    pub async fn create_code_host_pull_request_comment(
        &self,
        owner_principal_id: &str,
        params: &CodeHostPullRequestCommentCreateParams,
    ) -> Result<CodeHostPullRequestCommentCreateResult, AppServiceError> {
        self.require_repository_scope(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
            &params.repository,
        )?;
        let (client, _account, token) =
            self.client_and_token(owner_principal_id, &params.account_id)?;
        Ok(CodeHostPullRequestCommentCreateResult {
            comment: client
                .create_comment(
                    &token,
                    &params.repository,
                    params.number,
                    &params.body,
                    &CancellationToken::new(),
                )
                .await?,
        })
    }

    fn code_host_account_for_owner(
        &self,
        owner_principal_id: &str,
        account_id: &CodeHostAccountId,
    ) -> Result<CodeHostAccountProjection, AppServiceError> {
        let owner_principal_id = super::sanitize_session_owner_principal_id(owner_principal_id)?;
        let projection = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .code_host_account(account_id)?
            .ok_or(AppServiceError::CodeHostAccountNotFound)?;
        if projection.owner_principal_id != owner_principal_id {
            return Err(AppServiceError::CodeHostAccountForbidden);
        }
        Ok(projection)
    }

    fn client_and_token(
        &self,
        owner_principal_id: &str,
        account_id: &CodeHostAccountId,
    ) -> Result<(GitHubClient, CodeHostAccount, CodeHostAccessToken), AppServiceError> {
        let projection = self.code_host_account_for_owner(owner_principal_id, account_id)?;
        let client = client_for(projection.account.provider, &projection.account.host)?;
        let token = CodeHostCredentialStore::from_default_store()?
            .load(projection.account.provider, &projection.account.id)?;
        Ok((client, projection.account, token))
    }

    fn require_repository_scope(
        &self,
        owner_principal_id: &str,
        project_id: &ProjectId,
        workspace_id: &WorkspaceId,
        repository: &CodeHostRepositoryRef,
    ) -> Result<(), AppServiceError> {
        let context = self.code_host_repository_context(
            owner_principal_id,
            &CodeHostRepositoryContextParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
            },
        )?;
        if context
            .remotes
            .iter()
            .any(|remote| remote.repository == *repository)
        {
            Ok(())
        } else {
            Err(AppServiceError::CodeHostRepositoryNotInWorkspace)
        }
    }
}

fn normalize_display_name(value: &str) -> Result<String, AppServiceError> {
    let value = value.trim();
    if value.is_empty() || value.len() > ACCOUNT_DISPLAY_NAME_MAX_BYTES {
        return Err(AppServiceError::CodeHostAccountDisplayNameInvalid);
    }
    Ok(value.to_string())
}

fn normalize_host(provider: CodeHostProviderKind, value: &str) -> Result<String, AppServiceError> {
    match provider {
        CodeHostProviderKind::GitHub if value.trim().eq_ignore_ascii_case("github.com") => {
            Ok("github.com".to_string())
        }
        CodeHostProviderKind::GitHub => Err(CodeHostError::InvalidConfig.into()),
    }
}

fn client_for(provider: CodeHostProviderKind, host: &str) -> Result<GitHubClient, AppServiceError> {
    normalize_host(provider, host)?;
    Ok(GitHubClient::github_dot_com()?)
}

fn remote_repository<'a>(
    remotes: &'a [CodeHostRemote],
    remote_name: &str,
) -> Result<&'a CodeHostRepositoryRef, AppServiceError> {
    remotes
        .iter()
        .find(|remote| remote.remote_name == remote_name)
        .map(|remote| &remote.repository)
        .ok_or(AppServiceError::CodeHostRemoteNotFound)
}

fn map_push_preflight_error(error: crate::workspace::git::GitRepositoryError) -> AppServiceError {
    match error {
        crate::workspace::git::GitRepositoryError::RemoteChanged
        | crate::workspace::git::GitRepositoryError::NonFastForward => {
            AppServiceError::CodeHostPushStateChanged
        }
        crate::workspace::git::GitRepositoryError::CommandFailed { detail, .. } => {
            classify_push_transport_error(&detail).into()
        }
        crate::workspace::git::GitRepositoryError::Io(_) => CodeHostError::Unavailable.into(),
        error => map_git_error(error),
    }
}

fn classify_push_transport_error(detail: &str) -> CodeHostError {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("permission to") && detail.contains("denied")
        || detail.contains("requested url returned error: 403")
        || detail.contains("http 403")
    {
        CodeHostError::Forbidden
    } else if detail.contains("authentication failed")
        || detail.contains("could not read username")
        || detail.contains("terminal prompts disabled")
        || detail.contains("requested url returned error: 401")
        || detail.contains("http 401")
    {
        CodeHostError::Unauthorized
    } else if detail.contains("repository not found") {
        CodeHostError::NotFound
    } else {
        CodeHostError::Unavailable
    }
}

fn map_push_apply_error(error: crate::workspace::git::GitRepositoryError) -> AppServiceError {
    match error {
        crate::workspace::git::GitRepositoryError::RemoteChanged
        | crate::workspace::git::GitRepositoryError::NonFastForward => {
            AppServiceError::CodeHostPushStateChanged
        }
        crate::workspace::git::GitRepositoryError::CommandFailed { .. }
        | crate::workspace::git::GitRepositoryError::Io(_) => CodeHostError::OutcomeUnknown.into(),
        error => map_git_error(error),
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(owner: &str, expires_at_ms: u64) -> PreparedPush {
        PreparedPush {
            owner_principal_id: owner.to_string(),
            project_id: ProjectId::new("project-test").expect("project id"),
            workspace_id: WorkspaceId::new("workspace-test").expect("workspace id"),
            repository_root: PathBuf::from("test-repository"),
            account_id: CodeHostAccountId::new("account-test").expect("account id"),
            remote: CodeHostRemote {
                remote_name: "origin".to_string(),
                repository: CodeHostRepositoryRef {
                    provider: CodeHostProviderKind::GitHub,
                    host: "github.com".to_string(),
                    owner: "test-owner".to_string(),
                    name: "test-repository".to_string(),
                },
            },
            remote_url: "https://github.com/test-owner/test-repository.git".to_string(),
            source_head: "1111111111111111111111111111111111111111".to_string(),
            destination_branch: "main".to_string(),
            remote_head: None,
            expires_at_ms,
        }
    }

    #[test]
    fn push_token_is_one_shot_and_principal_bound() {
        let runtime = CodeHostPushRuntime::default();
        runtime
            .prepared
            .lock()
            .expect("runtime lock")
            .insert("push-token".to_string(), prepared("principal-a", 100));
        assert!(matches!(
            runtime.take("principal-b", "push-token", 50),
            Err(AppServiceError::CodeHostPushTokenInvalid)
        ));
        assert!(matches!(
            runtime.take("principal-a", "push-token", 50),
            Err(AppServiceError::CodeHostPushTokenInvalid)
        ));
    }

    #[test]
    fn expired_push_token_is_consumed() {
        let runtime = CodeHostPushRuntime::default();
        runtime
            .prepared
            .lock()
            .expect("runtime lock")
            .insert("push-token".to_string(), prepared("principal-a", 100));
        assert!(matches!(
            runtime.take("principal-a", "push-token", 101),
            Err(AppServiceError::CodeHostPushTokenInvalid)
        ));
        assert!(matches!(
            runtime.take("principal-a", "push-token", 99),
            Err(AppServiceError::CodeHostPushTokenInvalid)
        ));
    }

    #[test]
    fn push_preflight_errors_are_typed_without_exposing_git_output() {
        let cases = [
            (
                "fatal: Authentication failed for a private remote",
                CodeHostError::Unauthorized,
            ),
            (
                "remote: Permission to private/repository denied to local-user. fatal: requested URL returned error: 403",
                CodeHostError::Forbidden,
            ),
            ("remote: Repository not found.", CodeHostError::NotFound),
            (
                "fatal: unable to access a private remote: connection refused",
                CodeHostError::Unavailable,
            ),
        ];

        for (detail, expected) in cases {
            let error = map_push_preflight_error(
                crate::workspace::git::GitRepositoryError::CommandFailed {
                    context: "git remote head".to_string(),
                    detail: detail.to_string(),
                },
            );
            assert!(matches!(
                (&error, &expected),
                (
                    AppServiceError::CodeHost(CodeHostError::Unauthorized),
                    CodeHostError::Unauthorized
                ) | (
                    AppServiceError::CodeHost(CodeHostError::Forbidden),
                    CodeHostError::Forbidden
                ) | (
                    AppServiceError::CodeHost(CodeHostError::NotFound),
                    CodeHostError::NotFound
                ) | (
                    AppServiceError::CodeHost(CodeHostError::Unavailable),
                    CodeHostError::Unavailable
                )
            ));
            assert!(!error.to_string().contains("private"));
            assert!(!error.to_string().contains("local-user"));
        }
    }

    #[test]
    fn failed_push_apply_is_outcome_unknown_without_exposing_git_output() {
        let error =
            map_push_apply_error(crate::workspace::git::GitRepositoryError::CommandFailed {
                context: "git push".to_string(),
                detail: "fatal: private remote and credential context".to_string(),
            });

        assert!(matches!(
            &error,
            AppServiceError::CodeHost(CodeHostError::OutcomeUnknown)
        ));
        assert!(!error.to_string().contains("private"));
        assert!(!error.to_string().contains("credential"));
    }
}
