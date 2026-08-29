use std::collections::BTreeSet;

use ta_protocol::wire::{SourceCursor, WorkItem, WorkItemKey, WorkItemStatus, WorkSource};

use crate::{
    StoreError, WorkItemRepository,
    work_items::{merged_work_item, source_matches},
};

use super::InMemoryStore;

impl WorkItemRepository for InMemoryStore {
    fn work_items(&self) -> Result<Vec<WorkItem>, StoreError> {
        Ok(self.work_items.values().cloned().collect())
    }

    fn work_item(&self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError> {
        Ok(self.work_items.get(key).cloned())
    }

    fn upsert_work_items(&mut self, items: &[WorkItem]) -> Result<(), StoreError> {
        for item in items {
            let merged = merged_work_item(item, self.work_items.get(&item.key));
            self.work_items.insert(item.key.clone(), merged);
        }
        Ok(())
    }

    fn mark_missing_work_items_stale(
        &mut self,
        source: &WorkSource,
        active_keys: &[WorkItemKey],
    ) -> Result<(), StoreError> {
        let active_keys = active_keys.iter().collect::<BTreeSet<_>>();
        for item in self.work_items.values_mut() {
            if source_matches(&item.source, source)
                && !active_keys.contains(&item.key)
                && item.status == WorkItemStatus::Available
            {
                item.status = WorkItemStatus::Stale;
            }
        }
        Ok(())
    }

    fn dismiss_work_item(&mut self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError> {
        let Some(item) = self.work_items.get_mut(key) else {
            return Ok(None);
        };
        item.status = WorkItemStatus::Dismissed;
        Ok(Some(item.clone()))
    }

    fn mark_work_item_triggered(
        &mut self,
        key: &WorkItemKey,
        run_id: &str,
    ) -> Result<Option<WorkItem>, StoreError> {
        let Some(item) = self.work_items.get_mut(key) else {
            return Ok(None);
        };
        item.status = WorkItemStatus::Triggered;
        item.triggered_run_id = Some(run_id.to_string());
        Ok(Some(item.clone()))
    }

    fn work_source_cursor(&self, source_key: &str) -> Result<Option<SourceCursor>, StoreError> {
        Ok(self.work_source_cursors.get(source_key).cloned())
    }

    fn save_work_source_cursor(
        &mut self,
        source_key: &str,
        cursor: &SourceCursor,
    ) -> Result<(), StoreError> {
        self.work_source_cursors
            .insert(source_key.to_string(), cursor.clone());
        Ok(())
    }
}
