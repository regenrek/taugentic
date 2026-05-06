use ta_protocol::wire::{ReceiptKind, ReceiptState};

use crate::{InMemoryStore, ReceiptListQuery, ReceiptRepository};

use super::{artifact_receipt, make_run_id, make_session_id, parity::with_sqlite_store};

#[test]
fn list_filter_by_run_state_kind_parent_works_in_memory_and_sqlite() {
    let mut memory = InMemoryStore::current();
    exercise_list_filter_by_run_state_kind_parent(&mut memory);
    with_sqlite_store("receipts-list-filter-parent", |store| {
        exercise_list_filter_by_run_state_kind_parent(store);
    });
}

#[test]
fn list_limit_is_applied_in_memory_and_sqlite() {
    let mut memory = InMemoryStore::current();
    exercise_list_limit(&mut memory);
    with_sqlite_store("receipts-list-limit", |store| {
        exercise_list_limit(store);
    });
}

fn exercise_list_filter_by_run_state_kind_parent(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-filter");
    let other_session_id = make_session_id("session-filter-other");
    let run_id = make_run_id("run-filter");
    let other_run_id = make_run_id("run-filter-other");
    let parent_run_id = make_run_id("parent-filter");
    let other_parent_run_id = make_run_id("parent-filter-other");

    let target = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Patch,
            "artifact-target",
        ))
        .expect("target receipt");
    let target = store.promote(&target.id).expect("promote target");

    let returned_same_keys = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Patch,
            "artifact-returned",
        ))
        .expect("returned same keys");
    assert_eq!(returned_same_keys.state, ReceiptState::Returned);

    let wrong_kind = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Evidence,
            "artifact-kind",
        ))
        .expect("wrong kind");
    store.promote(&wrong_kind.id).expect("promote wrong kind");

    let wrong_parent = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(other_parent_run_id),
            ReceiptKind::Patch,
            "artifact-parent",
        ))
        .expect("wrong parent");
    store
        .promote(&wrong_parent.id)
        .expect("promote wrong parent");

    let wrong_run = store
        .create(artifact_receipt(
            &session_id,
            &other_run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Patch,
            "artifact-run",
        ))
        .expect("wrong run");
    store.promote(&wrong_run.id).expect("promote wrong run");

    let wrong_session = store
        .create(artifact_receipt(
            &other_session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Patch,
            "artifact-session",
        ))
        .expect("wrong session");
    store
        .promote(&wrong_session.id)
        .expect("promote wrong session");

    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(run_id),
                state: Some(ReceiptState::Promoted),
                kind: Some(ReceiptKind::Patch),
                parent_run_id: Some(parent_run_id),
                limit: None,
            })
            .expect("fully narrowed list"),
        vec![target]
    );
}

fn exercise_list_limit(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-limit");
    let run_id = make_run_id("run-limit");
    for index in 0..5 {
        store
            .create(artifact_receipt(
                &session_id,
                &run_id,
                None,
                ReceiptKind::Patch,
                &format!("artifact-limit-{index}"),
            ))
            .expect("receipt should seed");
    }

    let limited = store
        .list(&ReceiptListQuery {
            session_id,
            run_id: Some(run_id),
            state: None,
            kind: Some(ReceiptKind::Patch),
            parent_run_id: None,
            limit: Some(3),
        })
        .expect("limited list");

    assert_eq!(limited.len(), 3);
}
