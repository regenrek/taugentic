use ta_protocol::wire::{
    ArtifactId, ContextReceipt, ContextReceiptEvent, DaemonEvent, PublicDaemonEvent, ReceiptKind,
    ReceiptProvenance, ReceiptState, RunId, SessionId,
};

#[test]
fn context_receipt_event_redacts_public_surface() {
    let receipt = ContextReceipt {
        id: "receipt_public".to_string(),
        session_id: SessionId::new("session-1").expect("session id"),
        run_id: RunId::new("run-1").expect("run id"),
        parent_run_id: None,
        kind: ReceiptKind::Patch,
        provenance: ReceiptProvenance {
            artifact_id: Some(ArtifactId::new("artifact-1").expect("artifact id")),
            agent_turn_id: None,
            event_seq: None,
            stream_cursor: None,
        },
        state: ReceiptState::Returned,
        title: Some("internal title".to_string()),
        summary: Some("public summary".to_string()),
        created_at_ms: 1,
        promoted_at_ms: None,
        quarantined_at_ms: None,
    };

    let public =
        DaemonEvent::ContextReceipt(ContextReceiptEvent::Created { receipt }).redact_for_public();
    let json = serde_json::to_value(public).expect("public event should serialize");

    assert_eq!(json["contextReceipt"]["receipt"]["id"], "receipt_public");
    assert_eq!(
        json["contextReceipt"]["receipt"]["summary"],
        "public summary"
    );
    assert!(json["contextReceipt"]["receipt"].get("sessionId").is_none());
    assert!(json["contextReceipt"]["receipt"].get("runId").is_none());
    assert!(json["contextReceipt"]["receipt"].get("title").is_none());
    assert!(!json.to_string().contains("storage"));
}

#[test]
fn context_receipt_event_kind_roundtrips() {
    let event = DaemonEvent::ContextReceipt(ContextReceiptEvent::Quarantined {
        receipt: ContextReceipt {
            id: "receipt_kind".to_string(),
            session_id: SessionId::new("session-1").expect("session id"),
            run_id: RunId::new("run-1").expect("run id"),
            parent_run_id: None,
            kind: ReceiptKind::Evidence,
            provenance: ReceiptProvenance {
                artifact_id: Some(ArtifactId::new("artifact-1").expect("artifact id")),
                agent_turn_id: None,
                event_seq: None,
                stream_cursor: None,
            },
            state: ReceiptState::Quarantined,
            title: None,
            summary: None,
            created_at_ms: 1,
            promoted_at_ms: None,
            quarantined_at_ms: Some(2),
        },
    });

    let decoded: DaemonEvent =
        serde_json::from_value(serde_json::to_value(&event).expect("event json"))
            .expect("event should roundtrip");
    assert_eq!(
        decoded.kind(),
        ta_protocol::wire::DaemonEventKind::ContextReceipt
    );
    assert!(matches!(
        PublicDaemonEvent::from(decoded),
        PublicDaemonEvent::ContextReceipt(_)
    ));
}
