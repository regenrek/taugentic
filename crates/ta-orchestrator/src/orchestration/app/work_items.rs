use std::collections::HashSet;
use std::sync::{Arc, Mutex, atomic::Ordering};

use ta_protocol::wire::{WorkItemKey, WorkItemStatus};
use ta_store::PersistenceStore;

use crate::{
    RunSummary, StartRunCommand, WorkItemDismissParams, WorkItemDismissResult, WorkItemListQuery,
    WorkItemListResult, WorkItemRefreshParams, WorkItemTriggerParams, WorkItemTriggerResult,
    WorkSourceSyncState, WorkSourceSyncStatus,
};

use super::{AppDeferredMutationResult, AppService, AppServiceError};

/// A scoped, daemon-wide admission lease for one WorkItem trigger command.
///
/// The persistent WorkItem remains the lifecycle source of truth. This only
/// prevents concurrent command execution while a trigger is starting a run.
pub(crate) struct WorkItemTriggerLease {
    flights: Arc<Mutex<HashSet<WorkItemKey>>>,
    key: WorkItemKey,
}

impl Drop for WorkItemTriggerLease {
    fn drop(&mut self) {
        self.flights
            .lock()
            .expect("work item trigger flights should not be poisoned")
            .remove(&self.key);
    }
}

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub(crate) fn acquire_work_item_trigger_lease(
        &self,
        key: &WorkItemKey,
    ) -> Result<WorkItemTriggerLease, AppServiceError> {
        let flights = Arc::clone(&self.work_item_trigger_flights);
        if !flights
            .lock()
            .expect("work item trigger flights should not be poisoned")
            .insert(key.clone())
        {
            return Err(AppServiceError::WorkItemTriggerInFlight(
                key.as_str().to_string(),
            ));
        }
        Ok(WorkItemTriggerLease {
            flights,
            key: key.clone(),
        })
    }

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
        let _lease = self.acquire_work_item_trigger_lease(&params.key)?;
        self.workflow
            .current()
            .ok_or(AppServiceError::WorkflowNotLoaded)?;
        let item = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .work_item(&params.key)?
                .ok_or_else(|| AppServiceError::WorkItemNotFound(params.key.as_str().to_string()))?
        };
        if item.status != WorkItemStatus::Available {
            return Err(AppServiceError::WorkItemNotAvailable(
                params.key.as_str().to_string(),
            ));
        }
        let started = self.start_run(
            session_id,
            &StartRunCommand {
                objective: objective_for_work_item(&item),
                recipe_id: params.recipe_id.clone(),
                selection: params.selection.clone(),
                attachments: Vec::new(),
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
        item: ta_protocol::wire::WorkItem,
    ) -> Result<(), AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        store.upsert_work_items(&[item])?;
        Ok(())
    }
}

fn objective_for_work_item(item: &ta_protocol::wire::WorkItem) -> String {
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
