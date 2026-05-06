use ta_protocol::wire::{
    ApprovalId, ApprovalRequest, ApprovalScope, ApprovalTarget, DomainError, RunId, SessionId,
};

#[test]
fn session_id_rejects_blank_values() {
    let error = SessionId::new("   ").expect_err("blank session id should fail");
    assert_eq!(error, DomainError::EmptyIdentifier("session"));
}

#[test]
fn approval_request_rejects_blank_reason() {
    let error = ApprovalRequest::new(
        ApprovalId::new("approval-1").expect("approval id should be valid"),
        RunId::new("run-1").expect("run id should be valid"),
        ApprovalScope::FileWrite,
        100,
        200,
        ApprovalTarget::FileWrite {
            paths: vec!["src/lib.rs".to_string()],
        },
        " ",
    )
    .expect_err("blank approval reason should fail");

    assert_eq!(error, DomainError::EmptyApprovalReason);
}

#[test]
fn session_id_deserialization_rejects_blank_values() {
    let error = serde_json::from_str::<SessionId>(r#""   ""#)
        .expect_err("blank session id should fail deserialization");

    assert!(
        error
            .to_string()
            .contains("session identifier must not be empty"),
        "unexpected error: {error}"
    );
}

#[test]
fn approval_request_deserialization_rejects_blank_reason() {
    let error = serde_json::from_str::<ApprovalRequest>(
        r#"{
            "id":"approval-1",
            "runId":"run-1",
            "scope":"fileWrite",
            "requestedAtMs":"100",
            "expiresAtMs":"200",
            "target":{"kind":"fileWrite","paths":["src/lib.rs"]},
            "reason":"   "
        }"#,
    )
    .expect_err("blank approval reason should fail deserialization");

    assert!(
        error
            .to_string()
            .contains("approval reason must not be empty"),
        "unexpected error: {error}"
    );
}
