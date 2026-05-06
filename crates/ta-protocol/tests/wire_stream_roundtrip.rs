use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentToolCallOutcome, ApprovalActor, ApprovalDecision,
    ApprovalId, ApprovalResolution, ApprovalResolutionReason, ArtifactEvent, ArtifactId,
    ArtifactKind, ArtifactSummary, DaemonEvent, DaemonEventCursor, DaemonEventEnvelope,
    DaemonEventKind, DaemonSessionAttachParams, DaemonSessionAttachResult, DaemonSessionOpenResult,
    DaemonSubscribeParams, DaemonSubscribeResult, PublicApprovalResolution, RunId,
    RuntimeLanePendingState, SessionAuthority, SessionId, SessionStatus, SessionSummary,
};

#[test]
fn pending_state_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::PendingStateChanged {
        state: RuntimeLanePendingState::WaitingForApproval,
    };

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "pendingStateChanged",
            "state": "waitingForApproval"
        })
    );
}

#[test]
fn assistant_turn_started_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::AssistantTurnStarted;

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "assistantTurnStarted"
        })
    );
}

#[test]
fn assistant_message_delta_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::AssistantMessageDelta {
        delta: "partial".to_string(),
    };

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "assistantMessageDelta",
            "delta": "partial"
        })
    );
}

#[test]
fn assistant_turn_completed_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::AssistantTurnCompleted;

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "assistantTurnCompleted"
        })
    );
}

#[test]
fn tool_call_started_frame_serializes_tool_name_as_camel_case() {
    let frame = AgentStreamFrame::ToolCallStarted {
        tool_name: "shell".to_string(),
        input: r#"{"cmd":"echo hi"}"#.to_string(),
    };

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "toolCallStarted",
            "toolName": "shell",
            "input": "{\"cmd\":\"echo hi\"}"
        })
    );
}

#[test]
fn tool_call_progressed_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::ToolCallProgressed {
        delta: "stdout".to_string(),
    };

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "toolCallProgressed",
            "delta": "stdout"
        })
    );
}

#[test]
fn tool_call_completed_frame_roundtrips_through_json() {
    let frame = AgentStreamFrame::ToolCallCompleted {
        outcome: AgentToolCallOutcome::Completed,
    };

    let json = serde_json::to_value(&frame).expect("frame should serialize");
    let decoded: AgentStreamFrame =
        serde_json::from_value(json.clone()).expect("frame should deserialize");

    assert_eq!(decoded, frame);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "toolCallCompleted",
            "outcome": "completed"
        })
    );
}

#[test]
fn daemon_subscribe_result_ready_roundtrips_through_json() {
    let result = DaemonSubscribeResult::Ready {
        latest_cursor: Some(daemon_event_cursor(42)),
    };

    let json = serde_json::to_value(&result).expect("subscribe result should serialize");
    let decoded: DaemonSubscribeResult =
        serde_json::from_value(json).expect("subscribe result should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn daemon_subscribe_result_history_gap_roundtrips_through_json() {
    let result = DaemonSubscribeResult::HistoryGap {
        latest_cursor: Some(daemon_event_cursor(7)),
    };

    let json = serde_json::to_value(&result).expect("subscribe result should serialize");
    let decoded: DaemonSubscribeResult =
        serde_json::from_value(json).expect("subscribe result should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn daemon_subscribe_params_roundtrip_with_after_cursor() {
    let params = DaemonSubscribeParams {
        kinds: vec![DaemonEventKind::Run],
        after_cursor: Some(daemon_event_cursor(5)),
    };

    let json = serde_json::to_value(&params).expect("subscribe params should serialize");
    let decoded: DaemonSubscribeParams =
        serde_json::from_value(json).expect("subscribe params should deserialize");

    assert_eq!(decoded, params);
}

#[test]
fn daemon_event_envelope_roundtrips_with_full_lineage() {
    let envelope = DaemonEventEnvelope {
        daemon_instance_id: "daemon-1".to_string(),
        session_id: SessionId::new("session-1").expect("session id"),
        sequence: 42,
        occurred_at_ms: 99,
        event: DaemonEvent::Artifact(ArtifactEvent {
            artifact: ArtifactSummary {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                run_id: RunId::new("run-1").expect("run id"),
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
        }),
    };

    let json = serde_json::to_value(&envelope).expect("event envelope should serialize");
    let decoded: DaemonEventEnvelope =
        serde_json::from_value(json).expect("event envelope should deserialize");

    assert_eq!(decoded, envelope);
}

#[test]
fn daemon_session_open_result_roundtrips_through_json() {
    let result = DaemonSessionOpenResult {
        session: session_summary(),
        latest_cursor: Some(daemon_event_cursor(3)),
        session_authority: session_authority(),
    };

    let json = serde_json::to_value(&result).expect("session open result should serialize");
    let decoded: DaemonSessionOpenResult =
        serde_json::from_value(json).expect("session open result should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn daemon_session_attach_params_roundtrip_with_session_authority() {
    let params = DaemonSessionAttachParams {
        session_id: SessionId::new("session-1").expect("session id"),
        session_authority: session_authority(),
    };

    let json = serde_json::to_value(&params).expect("attach params should serialize");
    let decoded: DaemonSessionAttachParams =
        serde_json::from_value(json).expect("attach params should deserialize");

    assert_eq!(decoded, params);
}

#[test]
fn daemon_session_attach_result_roundtrips_through_json() {
    let result = DaemonSessionAttachResult {
        session: session_summary(),
        latest_cursor: Some(daemon_event_cursor(9)),
        session_authority: session_authority(),
    };

    let json = serde_json::to_value(&result).expect("session attach result should serialize");
    let decoded: DaemonSessionAttachResult =
        serde_json::from_value(json).expect("session attach result should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn approval_resolution_roundtrips_with_actor() {
    let resolution = ApprovalResolution::new(
        ApprovalId::new("approval-1").expect("approval id"),
        RunId::new("run-1").expect("run id"),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("principal-ta-cli").expect("approval actor"),
        Some("looks safe".to_string()),
    )
    .with_tool_call_id(AgentStreamItemId::new("tool-call-1").expect("tool call id"));

    let json = serde_json::to_value(&resolution).expect("resolution should serialize");
    let decoded: ApprovalResolution =
        serde_json::from_value(json).expect("resolution should deserialize");

    assert_eq!(decoded, resolution);
}

#[test]
fn approval_resolution_roundtrips_without_actor() {
    let json = serde_json::json!({
        "approvalId": "approval-1",
        "runId": "run-1",
        "decision": "approved",
        "reason": "user",
    });

    let decoded: ApprovalResolution =
        serde_json::from_value(json.clone()).expect("resolution should deserialize");
    let reencoded = serde_json::to_value(&decoded).expect("resolution should serialize");

    assert_eq!(decoded.actor, None);
    assert_eq!(decoded.commentary, None);
    assert_eq!(reencoded, json);
}

#[test]
fn public_approval_resolution_rejects_internal_only_fields() {
    let error = serde_json::from_value::<PublicApprovalResolution>(serde_json::json!({
        "approvalId": "approval-1",
        "runId": "run-1",
        "decision": "approved",
        "reason": "user",
        "actor": { "principalId": "principal-1" },
        "commentary": "looks safe"
    }))
    .expect_err("public approval resolution should reject internal-only fields");

    assert!(error.to_string().contains("unknown field"));
}

fn daemon_event_cursor(sequence: u64) -> DaemonEventCursor {
    DaemonEventCursor {
        daemon_instance_id: "daemon-1".to_string(),
        session_id: SessionId::new("session-1").expect("session id"),
        sequence,
    }
}

fn session_summary() -> SessionSummary {
    SessionSummary {
        id: SessionId::new("session-1").expect("session id"),
        title: "Build daemon app server".to_string(),
        status: SessionStatus::Idle,
    }
}

fn session_authority() -> SessionAuthority {
    SessionAuthority::new("session-authority-1session-authority-1".to_string())
        .expect("session authority")
}
