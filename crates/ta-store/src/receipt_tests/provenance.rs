use ta_protocol::wire::{AgentStreamTurnId, ReceiptKind, ReceiptProvenance};

use crate::{CreateReceipt, InMemoryStore, ReceiptListQuery, ReceiptRepository, StoreError};

use super::{
    artifact_receipt, artifact_receipt_with_stream_cursor, event_turn_receipt, make_run_id,
    make_session_id, mixed_provenance_receipt, parity::with_sqlite_store,
    stream_cursor_only_receipt,
};

#[test]
fn memory_receipts_are_idempotent_by_provenance() {
    let mut store = InMemoryStore::current();
    exercise_provenance_idempotency(&mut store);
}

#[test]
fn sqlite_receipts_are_idempotent_by_provenance() {
    with_sqlite_store("receipts-idempotency", |store| {
        exercise_provenance_idempotency(store);
    });
}

#[test]
fn mixed_provenance_is_rejected_in_memory_and_sqlite() {
    let mut memory = InMemoryStore::current();
    exercise_mixed_provenance_rejection(&mut memory);
    with_sqlite_store("receipts-mixed-provenance", |store| {
        exercise_mixed_provenance_rejection(store);
    });
}

#[test]
fn stream_cursor_only_is_free_form_in_memory_and_sqlite() {
    let mut memory = InMemoryStore::current();
    exercise_stream_cursor_only_is_free_form(&mut memory);
    with_sqlite_store("receipts-stream-cursor-free-form", |store| {
        exercise_stream_cursor_only_is_free_form(store);
    });
}

#[test]
fn artifact_with_stream_cursor_is_artifact_derived() {
    let mut memory = InMemoryStore::current();
    exercise_artifact_with_stream_cursor_is_artifact_derived(&mut memory);
    with_sqlite_store("receipts-artifact-stream-cursor", |store| {
        exercise_artifact_with_stream_cursor_is_artifact_derived(store);
    });
}

#[test]
fn event_turn_with_stream_cursor_is_event_derived() {
    let mut memory = InMemoryStore::current();
    exercise_event_turn_with_stream_cursor_is_event_derived(&mut memory);
    with_sqlite_store("receipts-event-turn-stream-cursor", |store| {
        exercise_event_turn_with_stream_cursor_is_event_derived(store);
    });
}

#[test]
fn stream_cursor_does_not_disambiguate_unique_key() {
    let mut memory = InMemoryStore::current();
    exercise_stream_cursor_does_not_disambiguate_unique_key(&mut memory);
    with_sqlite_store("receipts-stream-cursor-unique-key", |store| {
        exercise_stream_cursor_does_not_disambiguate_unique_key(store);
    });
}

fn exercise_provenance_idempotency(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-a");
    let run_id = make_run_id("run-a");
    let first = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            None,
            ReceiptKind::Patch,
            "artifact-a",
        ))
        .expect("first artifact receipt");
    let duplicate = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            None,
            ReceiptKind::Patch,
            "artifact-a",
        ))
        .expect("duplicate artifact receipt");
    assert_eq!(duplicate, first);

    let event_turn = store
        .create(CreateReceipt {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            parent_run_id: None,
            kind: ReceiptKind::Summary,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: Some(AgentStreamTurnId::new("turn-a").expect("turn id")),
                event_seq: Some(42),
                stream_cursor: Some("cursor-42".to_string()),
            },
            title: Some("turn summary".to_string()),
            summary: None,
        })
        .expect("event-turn receipt");
    let duplicate_event_turn = store
        .create(CreateReceipt {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            parent_run_id: None,
            kind: ReceiptKind::Summary,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: Some(AgentStreamTurnId::new("turn-a").expect("turn id")),
                event_seq: Some(42),
                stream_cursor: None,
            },
            title: None,
            summary: Some("duplicate payload is ignored".to_string()),
        })
        .expect("duplicate event-turn receipt");
    assert_eq!(duplicate_event_turn, event_turn);

    let invalid = store
        .create(CreateReceipt {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            parent_run_id: None,
            kind: ReceiptKind::Risk,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: Some(AgentStreamTurnId::new("turn-b").expect("turn id")),
                event_seq: None,
                stream_cursor: None,
            },
            title: None,
            summary: None,
        })
        .expect_err("partial event-turn provenance is invalid");
    assert!(matches!(invalid, StoreError::InvalidProvenance { .. }));

    store
        .create(CreateReceipt {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            parent_run_id: None,
            kind: ReceiptKind::Risk,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: None,
                event_seq: None,
                stream_cursor: None,
            },
            title: Some("free-form risk".to_string()),
            summary: None,
        })
        .expect("free-form receipt");

    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(run_id),
                state: None,
                kind: None,
                parent_run_id: None,
                limit: None,
            })
            .expect("all receipts")
            .len(),
        3
    );
}

fn exercise_mixed_provenance_rejection(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-mixed");
    let run_id = make_run_id("run-mixed");
    let first = store
        .create(mixed_provenance_receipt(
            &session_id,
            &run_id,
            "artifact-a",
            7,
            "turn-a",
        ))
        .expect_err("mixed provenance must be rejected");
    let second = store
        .create(mixed_provenance_receipt(
            &session_id,
            &run_id,
            "artifact-b",
            7,
            "turn-a",
        ))
        .expect_err("mixed provenance remains rejected with different artifact");

    assert!(matches!(first, StoreError::InvalidProvenance { .. }));
    assert_eq!(first, second);
    assert!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(run_id),
                state: None,
                kind: None,
                parent_run_id: None,
                limit: None,
            })
            .expect("list after rejection")
            .is_empty()
    );
}

fn exercise_stream_cursor_only_is_free_form(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-stream-only");
    let run_id = make_run_id("run-stream-only");
    let first = store
        .create(stream_cursor_only_receipt(
            &session_id,
            &run_id,
            "cursor-free-form-a",
        ))
        .expect("first stream-cursor-only receipt");
    let second = store
        .create(stream_cursor_only_receipt(
            &session_id,
            &run_id,
            "cursor-free-form-b",
        ))
        .expect("second stream-cursor-only receipt");

    assert_ne!(first.id, second.id);
    assert!(first.provenance.artifact_id.is_none());
    assert!(first.provenance.event_seq.is_none());
    assert!(first.provenance.agent_turn_id.is_none());
    assert_eq!(
        first.provenance.stream_cursor.as_deref(),
        Some("cursor-free-form-a")
    );
    assert_eq!(
        second.provenance.stream_cursor.as_deref(),
        Some("cursor-free-form-b")
    );
    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(run_id),
                state: None,
                kind: Some(ReceiptKind::Risk),
                parent_run_id: None,
                limit: None,
            })
            .expect("stream-cursor-only receipts")
            .len(),
        2
    );
}

fn exercise_artifact_with_stream_cursor_is_artifact_derived(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-artifact-stream");
    let run_id = make_run_id("run-artifact-stream");
    let first = store
        .create(artifact_receipt_with_stream_cursor(
            &session_id,
            &run_id,
            ReceiptKind::Patch,
            "artifact-with-stream",
            "cursor-artifact",
        ))
        .expect("artifact receipt with stream cursor");
    let duplicate = store
        .create(artifact_receipt(
            &session_id,
            &run_id,
            None,
            ReceiptKind::Patch,
            "artifact-with-stream",
        ))
        .expect("artifact duplicate without stream cursor");

    assert_eq!(duplicate, first);
    assert_eq!(
        first.provenance.stream_cursor.as_deref(),
        Some("cursor-artifact")
    );
}

fn exercise_event_turn_with_stream_cursor_is_event_derived(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-event-stream");
    let run_id = make_run_id("run-event-stream");
    let first = store
        .create(event_turn_receipt(
            &session_id,
            &run_id,
            84,
            "turn-with-stream",
            Some("cursor-event"),
        ))
        .expect("event-turn receipt with stream cursor");
    let duplicate = store
        .create(event_turn_receipt(
            &session_id,
            &run_id,
            84,
            "turn-with-stream",
            None,
        ))
        .expect("event-turn duplicate without stream cursor");

    assert_eq!(duplicate, first);
    assert_eq!(
        first.provenance.stream_cursor.as_deref(),
        Some("cursor-event")
    );
}

fn exercise_stream_cursor_does_not_disambiguate_unique_key(store: &mut impl ReceiptRepository) {
    let session_id = make_session_id("session-stream-unique");
    let run_id = make_run_id("run-stream-unique");
    let first = store
        .create(artifact_receipt_with_stream_cursor(
            &session_id,
            &run_id,
            ReceiptKind::Evidence,
            "artifact-same-key",
            "cursor-unique-a",
        ))
        .expect("first artifact receipt with stream cursor");
    let duplicate = store
        .create(artifact_receipt_with_stream_cursor(
            &session_id,
            &run_id,
            ReceiptKind::Evidence,
            "artifact-same-key",
            "cursor-unique-b",
        ))
        .expect("duplicate artifact receipt with different stream cursor");

    assert_eq!(duplicate, first);
    assert_eq!(
        duplicate.provenance.stream_cursor.as_deref(),
        Some("cursor-unique-a")
    );
    assert_eq!(
        store
            .list(&ReceiptListQuery {
                session_id,
                run_id: Some(run_id),
                state: None,
                kind: Some(ReceiptKind::Evidence),
                parent_run_id: None,
                limit: None,
            })
            .expect("stream cursor unique-key receipts")
            .len(),
        1
    );
}
