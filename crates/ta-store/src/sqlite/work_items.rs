use super::*;
use crate::work_items::{merged_work_item, source_kind, source_matches};

impl WorkItemRepository for SqliteStore {
    fn work_items(&self) -> Result<Vec<WorkItem>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM work_items ORDER BY key ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "work_item",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "work_item",
                source,
            })?;
        let mut items = Vec::new();
        for row in rows {
            let json = row.map_err(|source| StoreError::QueryStore {
                entity: "work_item",
                source,
            })?;
            items.push(Self::decode("work_item", json)?);
        }
        Ok(items)
    }

    fn work_item(&self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM work_items WHERE key = ?",
                [key.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "work_item",
                source,
            })?;
        json.map(|json| Self::decode("work_item", json)).transpose()
    }

    fn upsert_work_items(&mut self, items: &[WorkItem]) -> Result<(), StoreError> {
        for item in items {
            let merged = merged_work_item(item, self.work_item(&item.key)?.as_ref());
            self.write_work_item(&merged)?;
        }
        Ok(())
    }

    fn mark_missing_work_items_stale(
        &mut self,
        source: &WorkSource,
        active_keys: &[WorkItemKey],
    ) -> Result<(), StoreError> {
        for mut item in self.work_items()? {
            if source_matches(&item.source, source)
                && !active_keys.iter().any(|key| key == &item.key)
                && item.status == WorkItemStatus::Available
            {
                item.status = WorkItemStatus::Stale;
                self.write_work_item(&item)?;
            }
        }
        Ok(())
    }

    fn dismiss_work_item(&mut self, key: &WorkItemKey) -> Result<Option<WorkItem>, StoreError> {
        let Some(mut item) = self.work_item(key)? else {
            return Ok(None);
        };
        item.status = WorkItemStatus::Dismissed;
        self.write_work_item(&item)?;
        Ok(Some(item))
    }

    fn mark_work_item_triggered(
        &mut self,
        key: &WorkItemKey,
        run_id: &str,
    ) -> Result<Option<WorkItem>, StoreError> {
        let Some(mut item) = self.work_item(key)? else {
            return Ok(None);
        };
        item.status = WorkItemStatus::Triggered;
        item.triggered_run_id = Some(run_id.to_string());
        self.write_work_item(&item)?;
        Ok(Some(item))
    }

    fn work_source_cursor(&self, source_key: &str) -> Result<Option<SourceCursor>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM work_source_cursors WHERE source_key = ?",
                [source_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "work_source_cursor",
                source,
            })?;
        json.map(|json| Self::decode("work_source_cursor", json))
            .transpose()
    }

    fn save_work_source_cursor(
        &mut self,
        source_key: &str,
        cursor: &SourceCursor,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO work_source_cursors (source_key, data_json) VALUES (?, ?)
                 ON CONFLICT(source_key) DO UPDATE SET data_json = excluded.data_json",
                params![source_key, Self::encode("work_source_cursor", cursor)?],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "work_source_cursor",
                source,
            })?;
        Ok(())
    }
}

impl SqliteStore {
    fn write_work_item(&mut self, item: &WorkItem) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO work_items (key, source_kind, status, fetched_at_ms, data_json)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(key) DO UPDATE SET
                    source_kind = excluded.source_kind,
                    status = excluded.status,
                    fetched_at_ms = excluded.fetched_at_ms,
                    data_json = excluded.data_json",
                params![
                    item.key.as_str(),
                    source_kind_storage(source_kind(&item.source)),
                    status_storage(&item.status),
                    item.fetched_at_ms as i64,
                    Self::encode("work_item", item)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "work_item",
                source,
            })?;
        Ok(())
    }
}

fn source_kind_storage(kind: ta_protocol::wire::WorkSourceKind) -> &'static str {
    match kind {
        ta_protocol::wire::WorkSourceKind::GitHub => "github",
    }
}

fn status_storage(status: &WorkItemStatus) -> &'static str {
    match status {
        WorkItemStatus::Available => "available",
        WorkItemStatus::Dismissed => "dismissed",
        WorkItemStatus::Triggered => "triggered",
        WorkItemStatus::Stale => "stale",
    }
}
