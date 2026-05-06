use ta_store::{
    PersistenceStore, ReceiptListQuery, SessionAgentTurnsPageQuery, SessionApprovalQuery,
    SessionArtifactQuery, SessionEventPageQuery,
};

use crate::{
    ActivityCursor, ActivityPageQuery, AgentTurnsPageQuery, AgentTurnsPageResult,
    ApprovalAttentionState, ArtifactSnapshotResult, ArtifactSummary, CapsuleRecipe,
    GetArtifactQuery, GetRunQuery, ListArtifactsQuery, PublicActivityPageItem,
    PublicActivityPageResult, PublicDaemonEventEnvelope, ReceiptState, RunDetail, RunSummary,
    SessionOverview, SessionOverviewLaneStatus, SessionOverviewQuery, SessionOverviewResult,
    TokenUsageTotals, to_event_cursor,
};

use super::{
    AppService, AppServiceError, clamp_session_overview_recent_activity_limit,
    index_run_summaries_by_session, project_artifact_summary, project_latest_run_for_session,
    project_run_detail, project_session_overview_lane_status, project_session_summary,
    sanitize_session_owner_client_name, sanitize_session_owner_principal_id,
    session_overview_recent_activity_kinds, summarize_event_preview,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn list_recipes(&self) -> Vec<CapsuleRecipe> {
        self.recipe_registry
            .recipes()
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn session_overview(
        &self,
        owner_client_name: &str,
        owner_principal_id: &str,
        query: &SessionOverviewQuery,
    ) -> Result<SessionOverviewResult, AppServiceError> {
        let _owner_client_name = sanitize_session_owner_client_name(owner_client_name)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let recent_activity_limit =
            clamp_session_overview_recent_activity_limit(query.recent_activity_limit);
        let recent_activity_page_limit = recent_activity_limit.max(1);
        let store = self.store.lock().expect("app store should not be poisoned");
        let runs_by_session = index_run_summaries_by_session(store.runs()?);
        let sessions = store
            .sessions()?
            .into_iter()
            .filter(|session| session.owner_principal_id == owner_principal_id)
            .map(|session| {
                let session_summary = project_session_summary(session.clone());
                let session_id = session_summary.id.clone();
                let session_runs = runs_by_session.get(&session_id);
                let recent_events = store.session_event_page(&SessionEventPageQuery {
                    session_id: session_id.clone(),
                    before_sequence: None,
                    limit: recent_activity_page_limit,
                    kinds: session_overview_recent_activity_kinds(),
                })?;
                let approval_items = store.approvals_for_session(&SessionApprovalQuery {
                    session_id: session_id.clone(),
                    run_id: None,
                    approval_id: None,
                })?;
                let latest_run = project_latest_run_for_session(
                    &*store,
                    &session_id,
                    session_runs,
                    &recent_events.records,
                )?;
                let recent_activity = recent_events
                    .records
                    .iter()
                    .take(recent_activity_limit)
                    .map(|record| PublicDaemonEventEnvelope {
                        daemon_instance_id: self.daemon_instance_id.clone(),
                        session_id: record.session_id.clone(),
                        sequence: record.sequence,
                        occurred_at_ms: record.occurred_at_ms,
                        event: record.payload.clone().redact_for_public(),
                    })
                    .collect::<Vec<_>>();
                let pending_approval_count = u32::try_from(approval_items.len())
                    .expect("pending approval count should fit in u32");
                let lane_status = project_session_overview_lane_status(
                    latest_run.as_ref(),
                    pending_approval_count,
                );
                let is_active = matches!(
                    lane_status,
                    SessionOverviewLaneStatus::Active
                        | SessionOverviewLaneStatus::WaitingForApproval
                );

                Ok(SessionOverview {
                    session: session_summary,
                    latest_run,
                    lane_status,
                    is_active,
                    approval_attention: if pending_approval_count > 0 {
                        ApprovalAttentionState::Pending
                    } else {
                        ApprovalAttentionState::Idle
                    },
                    pending_approval_count,
                    last_activity_at_ms: recent_events
                        .records
                        .first()
                        .map(|record| record.occurred_at_ms),
                    last_event_preview: recent_events
                        .records
                        .first()
                        .map(|record| summarize_event_preview(&record.payload)),
                    recent_activity,
                })
            })
            .collect::<Result<Vec<_>, AppServiceError>>()?;
        Ok(SessionOverviewResult { sessions })
    }

    pub fn list_runs(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<Vec<RunSummary>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store
            .runs()?
            .into_iter()
            .filter(|run| run.session_id == *session_id)
            .map(|run| RunSummary {
                id: run.id,
                runtime_profile_id: run.runtime_profile_id,
                objective: run.objective,
                status: run.status,
            })
            .collect())
    }

    pub fn activity_page(
        &self,
        session_id: &crate::SessionId,
        query: &ActivityPageQuery,
    ) -> Result<PublicActivityPageResult, AppServiceError> {
        if query.limit == 0 {
            return Err(AppServiceError::InvalidActivityPageLimit);
        }

        let store = self.store.lock().expect("app store should not be poisoned");
        let page = store.session_event_page(&SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: query.before.as_ref().map(|cursor| cursor.sequence),
            limit: query.limit as usize,
            kinds: query.kinds.clone(),
        })?;
        let items = page
            .records
            .into_iter()
            .map(|record| PublicActivityPageItem {
                cursor: ActivityCursor {
                    sequence: record.sequence,
                },
                occurred_at_ms: record.occurred_at_ms,
                event: record.payload.redact_for_public(),
            })
            .collect::<Vec<_>>();

        Ok(PublicActivityPageResult {
            items,
            next_before: page
                .next_before_sequence
                .map(|sequence| ActivityCursor { sequence }),
            latest_activity_cursor: page
                .latest_sequence
                .map(|sequence| ActivityCursor { sequence }),
        })
    }

    pub fn agent_turns_page(
        &self,
        session_id: &crate::SessionId,
        query: &AgentTurnsPageQuery,
    ) -> Result<AgentTurnsPageResult, AppServiceError> {
        if query.limit == 0 {
            return Err(AppServiceError::InvalidAgentTurnsPageLimit);
        }

        let page = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store.session_agent_turns_page(&SessionAgentTurnsPageQuery {
                session_id: session_id.clone(),
                before_sequence: query.before.as_ref().map(|cursor| cursor.sequence),
                limit: query.limit as usize,
            })?
        };
        let latest_cursor = self.latest_event_cursor_for_session(session_id)?;
        Ok(AgentTurnsPageResult {
            items: page.rows,
            next_before: page
                .next_before_sequence
                .map(|sequence| ActivityCursor { sequence }),
            latest_cursor,
        })
    }

    pub fn latest_event_cursor_for_session(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<Option<crate::DaemonEventCursor>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store
            .session_event_page(&SessionEventPageQuery {
                session_id: session_id.clone(),
                before_sequence: None,
                limit: 1,
                kinds: Vec::new(),
            })?
            .latest_sequence
            .map(|sequence| to_event_cursor(&self.daemon_instance_id, session_id, sequence)))
    }

    pub fn list_artifacts(
        &self,
        session_id: &crate::SessionId,
        query: &ListArtifactsQuery,
    ) -> Result<ArtifactSnapshotResult, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let page = store.session_event_page(&SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 1,
            kinds: Vec::new(),
        })?;
        Ok(ArtifactSnapshotResult {
            items: store
                .artifacts_for_session(&SessionArtifactQuery {
                    session_id: session_id.clone(),
                    run_id: query.run_id.clone(),
                    artifact_id: query.artifact_id.clone(),
                })?
                .into_iter()
                .map(project_artifact_summary)
                .collect::<Vec<_>>(),
            latest_cursor: page
                .latest_sequence
                .map(|sequence| to_event_cursor(&self.daemon_instance_id, session_id, sequence)),
        })
    }

    pub fn get_artifact(
        &self,
        session_id: &crate::SessionId,
        query: &GetArtifactQuery,
    ) -> Result<Option<ArtifactSummary>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store
            .artifact_for_session(&SessionArtifactQuery {
                session_id: session_id.clone(),
                run_id: None,
                artifact_id: Some(query.artifact_id.clone()),
            })?
            .map(project_artifact_summary))
    }

    pub fn get_run(
        &self,
        session_id: &crate::SessionId,
        query: &GetRunQuery,
    ) -> Result<Option<RunDetail>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let Some(run) = store.run(&query.run_id)? else {
            return Ok(None);
        };
        if run.session_id != *session_id {
            return Ok(None);
        }
        let quarantine_receipt = store
            .list(&ReceiptListQuery {
                session_id: session_id.clone(),
                run_id: Some(run.id.clone()),
                state: Some(ReceiptState::Quarantined),
                kind: None,
                parent_run_id: None,
                limit: Some(1),
            })?
            .into_iter()
            .next();
        let token_usage = aggregate_run_token_usage(&*store, session_id, &run.id)?;

        Ok(Some(project_run_detail(
            &run,
            quarantine_receipt,
            token_usage,
        )))
    }
}

fn aggregate_run_token_usage<S>(
    store: &S,
    session_id: &crate::SessionId,
    run_id: &crate::RunId,
) -> Result<Option<TokenUsageTotals>, ta_store::StoreError>
where
    S: PersistenceStore + Send,
{
    let range = store.read_run_events(&ta_store::RunEventRangeQuery {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        after_sequence: None,
        limit: 10_000,
    })?;
    let mut totals = TokenUsageTotals::default();
    for record in range.records {
        let crate::DaemonEvent::TokenUsageRecorded(event) = record.payload else {
            continue;
        };
        totals.prompt_tokens = totals.prompt_tokens.saturating_add(event.prompt_tokens);
        totals.completion_tokens = totals
            .completion_tokens
            .saturating_add(event.completion_tokens);
        totals.cached_tokens = totals
            .cached_tokens
            .saturating_add(event.cached_tokens.unwrap_or(0));
        totals.reasoning_tokens = totals
            .reasoning_tokens
            .saturating_add(event.reasoning_tokens.unwrap_or(0));
    }
    Ok((!totals.is_zero()).then_some(totals))
}
