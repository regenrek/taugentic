use ta_work_source::{SourceCursor, WorkItem, WorkItemKey, WorkItemStatus, WorkSource};

use super::*;
use crate::WorkItemRepository;

#[test]
fn work_item_lifecycle_preserves_local_status() {
    let mut store = InMemoryStore::current();
    let item = work_item("1", WorkItemStatus::Available, None, 100);
    store
        .upsert_work_items(std::slice::from_ref(&item))
        .expect("upsert");

    let dismissed = store.dismiss_work_item(&item.key).expect("dismiss");
    assert_eq!(
        dismissed.map(|item| item.status),
        Some(WorkItemStatus::Dismissed)
    );

    let refetched = work_item("1", WorkItemStatus::Available, None, 200);
    store
        .upsert_work_items(std::slice::from_ref(&refetched))
        .expect("refetch");
    assert_eq!(
        some(store.work_item(&item.key)).status,
        WorkItemStatus::Dismissed
    );

    let triggered = store
        .mark_work_item_triggered(&item.key, "run-1")
        .expect("trigger");
    assert_eq!(
        triggered.and_then(|item| item.triggered_run_id),
        Some("run-1".to_string())
    );
}

#[test]
fn marks_missing_available_items_stale() {
    let mut store = InMemoryStore::current();
    let active = work_item("1", WorkItemStatus::Available, None, 100);
    let missing = work_item("2", WorkItemStatus::Available, None, 100);
    store
        .upsert_work_items(&[active.clone(), missing.clone()])
        .expect("upsert");

    store
        .mark_missing_work_items_stale(&active.source, std::slice::from_ref(&active.key))
        .expect("stale mark");

    assert_eq!(
        some(store.work_item(&active.key)).status,
        WorkItemStatus::Available
    );
    assert_eq!(
        some(store.work_item(&missing.key)).status,
        WorkItemStatus::Stale
    );
}

#[test]
fn persists_work_source_cursor() {
    let mut store = InMemoryStore::current();
    let cursor = SourceCursor {
        etag: Some("\"etag\"".to_string()),
        last_fetched_at_ms: Some(123),
    };
    store
        .save_work_source_cursor("github:regenrek/taugentic", &cursor)
        .expect("cursor save");

    assert_eq!(
        some(store.work_source_cursor("github:regenrek/taugentic")),
        cursor
    );
}

fn work_item(
    number: &str,
    status: WorkItemStatus,
    triggered_run_id: Option<String>,
    fetched_at_ms: u64,
) -> WorkItem {
    WorkItem {
        key: WorkItemKey::github("regenrek", "taugentic", number).expect("work item key"),
        source: WorkSource::GitHub {
            repo_owner: "regenrek".to_string(),
            repo_name: "taugentic".to_string(),
        },
        external_id: number.to_string(),
        title: format!("Issue {number}"),
        body: "body".to_string(),
        labels: vec!["ready".to_string()],
        url: format!("https://github.com/regenrek/taugentic/issues/{number}"),
        fetched_at_ms,
        status,
        triggered_run_id,
    }
}
