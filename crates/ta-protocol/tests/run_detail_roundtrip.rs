use ta_protocol::wire::{
    AgentStreamTurnId, ContextReceipt, EnvPolicy, ExecutionContext, NetworkPolicy,
    OutputContractKind, PermissionPolicy, ProcessExecPolicy, ReceiptKind, ReceiptProvenance,
    ReceiptState, RunDetail, RunId, RunStatus, RunSummary, RuntimeProfileId, SandboxProfile,
    SessionId, ValidationError, WorkspaceId, WorkspacePath, WorkspaceScope,
};

fn execution_context() -> ExecutionContext {
    let root = WorkspacePath::canonicalize_existing(
        std::env::current_dir().expect("test process should have a current directory"),
    )
    .expect("current directory should canonicalize");
    ExecutionContext {
        workspace_id: WorkspaceId::new("workspace-test").expect("workspace id"),
        workspace_root: root.clone(),
        effective_cwd: root.clone(),
        artifact_root: root.clone(),
        workspace_scope: WorkspaceScope::Local { root: root.clone() },
        sandbox_profile: SandboxProfile {
            read_roots: vec![root.clone()],
            write_roots: vec![root],
            denied_roots: Vec::new(),
            process_exec: ProcessExecPolicy::AllowAll,
        },
        permission_policy: PermissionPolicy::WorkspaceWrite,
        network_policy: NetworkPolicy::Open,
        env_policy: EnvPolicy::workspace_default(),
    }
}

#[test]
fn run_detail_roundtrips_with_typed_result_violation_and_receipt() {
    let detail = RunDetail {
        summary: RunSummary {
            id: RunId::new("run-child").expect("run id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Return a patch".to_string(),
            status: RunStatus::Failed,
        },
        result: None,
        contract_violation: Some(ValidationError::KindMismatch {
            expected: OutputContractKind::Patch,
            got: OutputContractKind::Debug,
        }),
        quarantine_receipt: Some(ContextReceipt {
            id: "receipt-quarantine".to_string(),
            session_id: SessionId::new("session-1").expect("session id"),
            run_id: RunId::new("run-child").expect("run id"),
            parent_run_id: Some(RunId::new("run-parent").expect("parent run id")),
            kind: ReceiptKind::Patch,
            provenance: ReceiptProvenance {
                artifact_id: None,
                agent_turn_id: Some(AgentStreamTurnId::new("turn-parent").expect("turn id")),
                event_seq: Some(42),
                stream_cursor: Some("run:run-child:event:42".to_string()),
            },
            state: ReceiptState::Quarantined,
            title: Some("Patch result".to_string()),
            summary: Some("Patch CapsuleResult quarantined after daemon validation".to_string()),
            created_at_ms: 100,
            promoted_at_ms: None,
            quarantined_at_ms: Some(101),
        }),
        output_contract: Some(OutputContractKind::Patch),
        recipe_id: Some("patch-native-subagent".to_string()),
        parent_run_id: Some(RunId::new("run-parent").expect("parent run id")),
        execution_context: execution_context(),
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
        token_usage: None,
        auth_profile_exhaustion: None,
    };

    let json = serde_json::to_value(&detail).expect("run detail should serialize");
    let decoded: RunDetail = serde_json::from_value(json.clone()).expect("run detail roundtrip");

    assert_eq!(decoded, detail);
    assert_eq!(json["summary"]["runtimeProfileId"], "runtime-openai-safe");
    assert_eq!(json["contractViolation"]["kind"], "kindMismatch");
    assert_eq!(json["quarantineReceipt"]["provenance"]["eventSeq"], "42");
    assert_eq!(json["parentRunId"], "run-parent");
}

#[test]
fn run_detail_skips_absent_optional_fields() {
    let detail = RunDetail {
        summary: RunSummary {
            id: RunId::new("run-running").expect("run id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Still running".to_string(),
            status: RunStatus::Running,
        },
        result: None,
        contract_violation: None,
        quarantine_receipt: None,
        output_contract: None,
        recipe_id: None,
        parent_run_id: None,
        execution_context: execution_context(),
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
        token_usage: None,
        auth_profile_exhaustion: None,
    };

    let json = serde_json::to_value(&detail).expect("run detail should serialize");

    assert!(json.get("result").is_none());
    assert!(json.get("contractViolation").is_none());
    assert!(json.get("quarantineReceipt").is_none());
    assert!(json.get("parentRunId").is_none());
    assert!(json.get("workspaceInfo").is_none());
    assert!(json.get("claimedFiles").is_none());
    assert!(json.get("conflictSummary").is_none());
    assert!(json.get("tokenUsage").is_none());
}
