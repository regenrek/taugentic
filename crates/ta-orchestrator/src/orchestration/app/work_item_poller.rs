use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{WorkflowDefinition, WorkflowSourceKind};
use ta_store::PersistenceStore;
use ta_work_source::{
    FetchOutcome, GitHubCredentialProvider, GitHubIssueProvider, GitHubProviderConfig,
    SourceCursor, WorkItemKey, WorkSource, WorkSourceError, WorkSourceLabelFilter,
};
use tokio_util::sync::CancellationToken;

use super::AppService;

const IDLE_WORKFLOW_CHECK_INTERVAL: Duration = Duration::from_secs(5);

impl<S> AppService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn spawn_work_source_poller(
        &self,
        cancellation: CancellationToken,
        github_credentials: Arc<dyn GitHubCredentialProvider>,
    ) -> tokio::task::JoinHandle<()> {
        let app = self.clone();
        tokio::spawn(async move {
            app.run_work_source_poller(cancellation, github_credentials)
                .await;
        })
    }

    async fn run_work_source_poller(
        self,
        cancellation: CancellationToken,
        github_credentials: Arc<dyn GitHubCredentialProvider>,
    ) {
        let mut delay = IDLE_WORKFLOW_CHECK_INTERVAL;
        loop {
            if cancellation.is_cancelled() {
                tracing::info!("work source poller cancelled");
                return;
            }
            let Some(workflow) = self.workflow.current() else {
                tracing::debug!("background orchestrator idle; no workflow loaded");
                delay = IDLE_WORKFLOW_CHECK_INTERVAL;
                if !self.sleep_until_next_poll(delay, &cancellation).await {
                    return;
                }
                continue;
            };
            let Some(config) = GitHubPollConfig::from_workflow(&workflow) else {
                tracing::debug!(
                    source_kind = ?workflow.source.kind,
                    "background source adapter is not implemented for workflow source"
                );
                delay = retry_initial_delay(&workflow);
                if !self.sleep_until_next_poll(delay, &cancellation).await {
                    return;
                }
                continue;
            };
            delay = match self
                .poll_github_once(&config, github_credentials.as_ref(), cancellation.clone())
                .await
            {
                Ok(()) => jittered(
                    retry_initial_delay(&workflow),
                    self.daemon_instance_id.as_bytes(),
                ),
                Err(WorkSourceError::Cancelled) => return,
                Err(WorkSourceError::HttpStatus {
                    backoff: Some(backoff),
                    ..
                }) => {
                    let retry_after = backoff.retry_after.min(retry_max_delay(&workflow));
                    tracing::warn!(
                        retry_after_ms = retry_after.as_millis() as u64,
                        reason = ?backoff.reason,
                        "work source poller rate limited"
                    );
                    retry_after
                }
                Err(error) => {
                    tracing::warn!(error = %error, "work source poll failed");
                    (delay * 2).min(retry_max_delay(&workflow))
                }
            };
            if !self.sleep_until_next_poll(delay, &cancellation).await {
                return;
            }
        }
    }

    async fn sleep_until_next_poll(
        &self,
        delay: Duration,
        cancellation: &CancellationToken,
    ) -> bool {
        let deadline = Instant::now() + delay;
        loop {
            if cancellation.is_cancelled() {
                return false;
            }
            if self
                .work_source_refresh_requested
                .swap(false, Ordering::SeqCst)
            {
                tracing::debug!("work source poller woke for manual refresh");
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return true;
            }
            let sleep_for = (deadline - now).min(Duration::from_secs(1));
            tokio::select! {
                _ = cancellation.cancelled() => return false,
                _ = tokio::time::sleep(sleep_for) => {}
            }
        }
    }

    async fn poll_github_once(
        &self,
        config: &GitHubPollConfig,
        github_credentials: &dyn GitHubCredentialProvider,
        cancellation: CancellationToken,
    ) -> Result<(), WorkSourceError> {
        let token = github_credentials.token()?;
        let source_key = config.source_key();
        let cursor = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .work_source_cursor(&source_key)
                .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?
                .unwrap_or_else(SourceCursor::empty)
        };
        tracing::info!(
            repo = %config.repo,
            backend = ?ta_host_platform::secrets_backend_capability(),
            "work source GitHub poll started"
        );
        let provider = GitHubIssueProvider::new(config.provider_config()?);
        match provider
            .fetch(&token, cursor, current_time_ms(), cancellation)
            .await?
        {
            FetchOutcome::Items { items, cursor } => {
                let active_keys = items
                    .iter()
                    .map(|item| item.key.clone())
                    .collect::<Vec<WorkItemKey>>();
                let source = config.source()?;
                let mut store = self.store.lock().expect("app store should not be poisoned");
                store
                    .upsert_work_items(&items)
                    .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?;
                store
                    .mark_missing_work_items_stale(&source, &active_keys)
                    .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?;
                store
                    .save_work_source_cursor(&source_key, &cursor)
                    .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?;
                tracing::info!(repo = %config.repo, item_count = items.len(), "work source poll stored items");
            }
            FetchOutcome::NotModified { cursor } => {
                let mut store = self.store.lock().expect("app store should not be poisoned");
                store
                    .save_work_source_cursor(&source_key, &cursor)
                    .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?;
                tracing::debug!(repo = %config.repo, "work source poll not modified");
            }
        }
        Ok(())
    }
}

struct GitHubPollConfig {
    repo: String,
    labels: Vec<String>,
    api_base_url: Option<String>,
}

impl GitHubPollConfig {
    fn from_workflow(workflow: &WorkflowDefinition) -> Option<Self> {
        if workflow.source.kind != WorkflowSourceKind::GithubIssues {
            return None;
        }
        let repo = workflow.source.repo.as_ref()?.trim().to_string();
        if repo.is_empty() {
            return None;
        }
        Some(Self {
            repo,
            labels: workflow.source.active_states.clone(),
            api_base_url: None,
        })
    }

    fn provider_config(&self) -> Result<GitHubProviderConfig, WorkSourceError> {
        let (owner, name) = self.repo_parts()?;
        let config = GitHubProviderConfig::new(owner, name, self.label_filter())?;
        match &self.api_base_url {
            Some(api_base_url) => config.with_base_url(api_base_url),
            None => Ok(config),
        }
    }

    fn source(&self) -> Result<WorkSource, WorkSourceError> {
        let (owner, name) = self.repo_parts()?;
        Ok(WorkSource::GitHub {
            repo_owner: owner.to_string(),
            repo_name: name.to_string(),
        })
    }

    fn source_key(&self) -> String {
        format!("github:{}", self.repo)
    }

    fn repo_parts(&self) -> Result<(&str, &str), WorkSourceError> {
        let Some((owner, name)) = self.repo.split_once('/') else {
            return Err(WorkSourceError::InvalidConfig(
                "workflow source.repo must be owner/name for github_issues".to_string(),
            ));
        };
        if owner.trim().is_empty() || name.trim().is_empty() {
            return Err(WorkSourceError::InvalidConfig(
                "workflow source.repo must be owner/name for github_issues".to_string(),
            ));
        }
        Ok((owner, name))
    }

    fn label_filter(&self) -> WorkSourceLabelFilter {
        if self.labels.is_empty() {
            WorkSourceLabelFilter::Any
        } else {
            WorkSourceLabelFilter::AnyOf(self.labels.clone())
        }
    }
}

fn jittered(base: Duration, seed: &[u8]) -> Duration {
    let jitter_ms = seed.iter().fold(0u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(*byte))
    }) % 5_000;
    base + Duration::from_millis(jitter_ms)
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn retry_initial_delay(workflow: &WorkflowDefinition) -> Duration {
    Duration::from_millis(workflow.orchestrator.retry.initial_ms)
}

fn retry_max_delay(workflow: &WorkflowDefinition) -> Duration {
    Duration::from_millis(workflow.orchestrator.retry.max_ms)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use serde_json::json;
    use ta_store::WorkItemRepository;
    use ta_work_source::GitHubToken;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn poll_github_once_uses_injected_host_secret_provider() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/regenrek/taugentic/issues"))
            .and(query_param("page", "1"))
            .and(header("authorization", "Bearer ghp_ssot"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "number": 7,
                "title": "Ship SSOT",
                "body": "body",
                "html_url": "https://github.com/regenrek/taugentic/issues/7",
                "labels": [{ "name": "ready" }]
            }])))
            .expect(1)
            .mount(&server)
            .await;
        let service = AppService::bootstrap()?;
        let config = GitHubPollConfig {
            repo: "regenrek/taugentic".to_string(),
            labels: vec!["ready".to_string()],
            api_base_url: Some(server.uri()),
        };

        service
            .poll_github_once(
                &config,
                &StaticGitHubCredentialProvider,
                CancellationToken::new(),
            )
            .await?;

        let items = service
            .store
            .lock()
            .expect("app store should not be poisoned")
            .work_items()?;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key.as_str(), "github:regenrek/taugentic#7");
        Ok(())
    }

    struct StaticGitHubCredentialProvider;

    impl GitHubCredentialProvider for StaticGitHubCredentialProvider {
        fn token(&self) -> Result<GitHubToken, WorkSourceError> {
            GitHubToken::new("ghp_ssot")
        }
    }
}
