use ta_work_source::{
    SourceCursor, WorkItem, WorkItemKey, WorkItemStatus, WorkSource, WorkSourceKind,
};

use crate::StoreError;

pub trait WorkItemRepository {
    fn work_items(&self) -> Result<Vec<WorkItem>, StoreError>;
    fn work_item(&self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError>;
    fn upsert_work_items(&mut self, items: &[WorkItem]) -> Result<(), StoreError>;
    fn mark_missing_work_items_stale(
        &mut self,
        source: &WorkSource,
        active_keys: &[WorkItemKey],
    ) -> Result<(), StoreError>;
    fn dismiss_work_item(&mut self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError>;
    fn mark_work_item_triggered(
        &mut self,
        key: &WorkItemKey,
        run_id: &str,
    ) -> Result<Option<WorkItem>, StoreError>;
    fn work_source_cursor(&self, source_key: &str) -> Result<Option<SourceCursor>, StoreError>;
    fn save_work_source_cursor(
        &mut self,
        source_key: &str,
        cursor: &SourceCursor,
    ) -> Result<(), StoreError>;
}

pub(crate) fn merged_work_item(incoming: &WorkItem, existing: Option<&WorkItem>) -> WorkItem {
    let Some(existing) = existing else {
        return incoming.clone();
    };
    match existing.status {
        WorkItemStatus::Dismissed | WorkItemStatus::Triggered => WorkItem {
            status: existing.status.clone(),
            triggered_run_id: existing.triggered_run_id.clone(),
            ..incoming.clone()
        },
        WorkItemStatus::Available | WorkItemStatus::Stale => incoming.clone(),
    }
}

pub(crate) fn source_kind(source: &WorkSource) -> WorkSourceKind {
    match source {
        WorkSource::GitHub { .. } => WorkSourceKind::GitHub,
    }
}

pub(crate) fn source_matches(left: &WorkSource, right: &WorkSource) -> bool {
    match (left, right) {
        (
            WorkSource::GitHub {
                repo_owner: left_owner,
                repo_name: left_name,
            },
            WorkSource::GitHub {
                repo_owner: right_owner,
                repo_name: right_name,
            },
        ) => left_owner == right_owner && left_name == right_name,
    }
}
