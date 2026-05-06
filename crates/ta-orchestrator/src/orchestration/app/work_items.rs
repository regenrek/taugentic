use std::sync::atomic::Ordering;

use ta_store::PersistenceStore;

use crate::{
    RunSummary, StartRunCommand, WorkItemDismissParams, WorkItemDismissResult, WorkItemListQuery,
    WorkItemListResult, WorkItemRefreshParams, WorkItemTriggerParams, WorkItemTriggerResult,
    WorkSourceSyncState, WorkSourceSyncStatus,
};

use super::{AppDeferredMutationResult, AppService, AppServiceError};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn list_work_items(
        &self,
        _query: &WorkItemListQuery,
    ) -> Result<WorkItemListResult, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let workflow_status = self.workflow.status();
        let sync = if workflow_status.loaded.is_some() {
            WorkSourceSyncStatus {
                state: WorkSourceSyncState::Idle,
                last_fetched_at_ms: None,
                detail: workflow_status
                    .last_reload
                    .as_ref()
                    .map(|outcome| format!("workflow reload status: {outcome:?}")),
            }
        } else {
            WorkSourceSyncStatus {
                state: WorkSourceSyncState::Disabled,
                last_fetched_at_ms: None,
                detail: Some("workflow not loaded; background orchestrator is idle".to_string()),
            }
        };
        Ok(WorkItemListResult {
            items: store.work_items()?,
            sync,
        })
    }

    pub fn refresh_work_items(
        &self,
        _params: &WorkItemRefreshParams,
    ) -> Result<WorkItemListResult, AppServiceError> {
        self.work_source_refresh_requested
            .store(true, Ordering::SeqCst);
        let result = self.list_work_items(&WorkItemListQuery {})?;
        tracing::info!(
            item_count = result.items.len(),
            "work item refresh requested; daemon poller owns provider synchronization"
        );
        Ok(WorkItemListResult {
            sync: WorkSourceSyncStatus {
                state: WorkSourceSyncState::RefreshQueued,
                last_fetched_at_ms: result.sync.last_fetched_at_ms,
                detail: Some("refresh queued for daemon-side poller".to_string()),
            },
            ..result
        })
    }

    pub fn dismiss_work_item(
        &self,
        params: &WorkItemDismissParams,
    ) -> Result<WorkItemDismissResult, AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        Ok(WorkItemDismissResult {
            item: store.dismiss_work_item(&params.key)?,
        })
    }

    pub fn trigger_work_item(
        &self,
        session_id: &crate::SessionId,
        params: &WorkItemTriggerParams,
    ) -> Result<AppDeferredMutationResult<WorkItemTriggerResult>, AppServiceError> {
        let workflow = self
            .workflow
            .current()
            .ok_or(AppServiceError::WorkflowNotLoaded)?;
        let item = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .work_item(&params.key)?
                .ok_or_else(|| AppServiceError::WorkItemNotFound(params.key.as_str().to_string()))?
        };
        let started = self.start_run(
            session_id,
            &StartRunCommand {
                objective: objective_for_work_item(&item),
                recipe_id: params.recipe_id.clone(),
                model_id: workflow
                    .runtime_profiles
                    .get("implementer")
                    .or_else(|| workflow.runtime_profiles.values().next())
                    .map(|profile| profile.model.clone()),
                sandbox_profile: None,
            },
        )?;
        let updated = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            store
                .mark_work_item_triggered(&params.key, started.body.id.as_str())?
                .ok_or_else(|| AppServiceError::WorkItemNotFound(params.key.as_str().to_string()))?
        };
        Ok(AppDeferredMutationResult {
            body: WorkItemTriggerResult {
                item: updated,
                run: RunSummary {
                    id: started.body.id,
                    runtime_profile_id: started.body.runtime_profile_id,
                    objective: started.body.objective,
                    status: started.body.status,
                },
            },
            deferred_records: started.deferred_records,
        })
    }

    #[cfg(test)]
    pub(crate) fn seed_work_item_for_tests(
        &self,
        item: ta_work_source::WorkItem,
    ) -> Result<(), AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        store.upsert_work_items(&[item])?;
        Ok(())
    }
}

fn objective_for_work_item(item: &ta_work_source::WorkItem) -> String {
    let labels = if item.labels.is_empty() {
        "none".to_string()
    } else {
        item.labels.join(", ")
    };
    format!(
        "Work on background item {}: {}\n\nSource: {}\nLabels: {}\n\n{}",
        item.external_id, item.title, item.url, labels, item.body
    )
}
