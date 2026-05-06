use ta_protocol::wire::{ReceiptKind, ReceiptState};

use crate::{InMemoryStore, ReceiptListQuery, ReceiptRepository, StoreError};

use super::{artifact_receipt, make_run_id, make_session_id, parity::with_sqlite_store};

#[test]
fn memory_receipts_follow_state_machine_and_filters() {
    let mut store = InMemoryStore::current();
    exercise_state_machine_and_filters(&mut store);
}

#[test]
fn sqlite_receipts_follow_state_machine_and_filters() {
    with_sqlite_store("receipts-state-machine", |store| {
        exercise_state_machine_and_filters(store);
    });
}

#[test]
fn idempotent_promote_on_promoted_is_noop_writeback() {
    let mut memory = InMemoryStore::current();
    exercise_idempotent_promote_noop(&mut memory);
    with_sqlite_store("receipts-promote-noop", |store| {
        exercise_idempotent_promote_noop(store);
    });
}

#[test]
fn idempotent_quarantine_on_quarantined_is_noop_writeback() {
    let mut memory = InMemoryStore::current();
    exercise_idempotent_quarantine_noop(&mut memory);
    with_sqlite_store("receipts-quarantine-noop", |store| {
        exercise_idempotent_quarantine_noop(store);
    });
}

fn exercise_state_machine_and_filters(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-a");
    let run_id = make_run_id("run-a");
    let parent_run_id = make_run_id("parent-a");
    let returned = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Patch,
            "artifact-a",
        ))
        .expect("returned receipt");
    assert_eq!(returned.state, ReceiptState::Returned);
    assert!(returned.promoted_at_ms.is_none());
    assert!(returned.quarantined_at_ms.is_none());

    let promoted = store.promote(&returned.id).expect("promote returned");
    assert_eq!(promoted.state, ReceiptState::Promoted);
    assert!(promoted.promoted_at_ms.is_some());
    assert!(promoted.quarantined_at_ms.is_none());
    assert_eq!(
        store.promote(&returned.id).expect("promote idempotent"),
        promoted
    );

    let error = store
        .quarantine(&returned.id)
        .expect_err("promoted receipt must not quarantine");
    assert_eq!(
        error,
        StoreError::ReceiptTransitionViolation {
            receipt_id: returned.id.clone(),
            detail: "cannot quarantine promoted receipt".to_string(),
        }
    );

    let quarantined_source = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            Some(parent_run_id.clone()),
            ReceiptKind::Evidence,
            "artifact-b",
        ))
        .expect("quarantine source");
    let quarantined = store
        .quarantine(&quarantined_source.id)
        .expect("quarantine returned");
    assert_eq!(quarantined.state, ReceiptState::Quarantined);
    assert!(quarantined.quarantined_at_ms.is_some());
    assert_eq!(
        store
            .quarantine(&quarantined_source.id)
            .expect("quarantine idempotent"),
        quarantined
    );

    let promoted_after_quarantine = store
        .promote(&quarantined_source.id)
        .expect("promote quarantined");
    assert_eq!(promoted_after_quarantine.state, ReceiptState::Promoted);
    assert_eq!(
        promoted_after_quarantine.quarantined_at_ms,
        quarantined.quarantined_at_ms
    );
    assert!(promoted_after_quarantine.promoted_at_ms.is_some());

    store
        .create(artifact_receipt(
            &session_id,
            &make_run_id("run-b"),
            None,
            ReceiptKind::Artifact,
            "artifact-c",
        ))
        .expect("other run");
    store
        .create(artifact_receipt(
            &make_session_id("session-b"),
            &run_id,
            None,
            ReceiptKind::Patch,
            "artifact-d",
        ))
        .expect("other session");

    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id: session_id.clone(),
                run_id: Some(run_id.clone()),
                state: Some(ReceiptState::Promoted),
                kind: None,
                parent_run_id: None,
                limit: None,
            })
            .expect("promoted list")
            .len(),
        2
    );
    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id: session_id.clone(),
                run_id: Some(run_id.clone()),
                state: None,
                kind: Some(ReceiptKind::Patch),
                parent_run_id: Some(parent_run_id),
                limit: None,
            })
            .expect("parent/kind list")
            .len(),
        1
    );
    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(make_run_id("run-b")),
                state: None,
                kind: Some(ReceiptKind::Artifact),
                parent_run_id: None,
                limit: None,
            })
            .expect("run/kind list")
            .len(),
        1
    );
}

fn exercise_idempotent_promote_noop(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-promote-noop");
    let run_id = make_run_id("run-promote-noop");
    let returned = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            None,
            ReceiptKind::Patch,
            "artifact-promote-noop",
        ))
        .expect("returned receipt");
    let promoted = store.promote(&returned.id).expect("first promote");
    let promoted_again = store.promote(&returned.id).expect("second promote");

    assert_eq!(promoted_again, promoted);
    assert_eq!(
        store
            .receipt(&returned.id)
            .expect("read promoted")
            .expect("promoted stored"),
        promoted
    );
}

fn exercise_idempotent_quarantine_noop(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-quarantine-noop");
    let run_id = make_run_id("run-quarantine-noop");
    let returned = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            None,
            ReceiptKind::Evidence,
            "artifact-quarantine-noop",
        ))
        .expect("returned receipt");
    let quarantined = store.quarantine(&returned.id).expect("first quarantine");
    let quarantined_again = store.quarantine(&returned.id).expect("second quarantine");

    assert_eq!(quarantined_again, quarantined);
    assert_eq!(
        store
            .receipt(&returned.id)
            .expect("read quarantined")
            .expect("quarantined stored"),
        quarantined
    );
}
