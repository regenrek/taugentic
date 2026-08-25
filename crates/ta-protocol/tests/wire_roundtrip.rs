use ta_protocol::wire::{
    ActivityCursor, ActivityPageQuery, AgentRuntimeModelId, AgentRuntimeStrategyId,
    AgentStreamEvent, AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome,
    ApprovalActor, ApprovalDecision, ApprovalRequest, ApprovalResolution, ApprovalResolutionReason,
    ApprovalScope, ApprovalSnapshotResult, ApprovalTarget, ArtifactEvent, ArtifactId, ArtifactKind,
    ArtifactSnapshotResult, ArtifactSummary, CapsuleResult, DaemonEvent, DaemonEventCursor,
    DaemonEventEnvelope, DaemonEventKind, DaemonRunCompleteWithResultParams,
    DaemonSessionAttachParams, DaemonSessionAttachResult, DaemonSessionOpenParams,
    DaemonSessionOpenResult, DaemonSubscribeParams, DaemonSubscribeResult,
    DaemonWorkspaceGetParams, DaemonWorkspaceGetResult, DaemonWorkspaceListParams,
    DaemonWorkspaceListResult, DaemonWorkspaceOpenParams, DaemonWorkspaceOpenResult, EnvPolicy,
    ExecutionContext, ForkRunRequest, ForkRunResult, GetAgentRuntimeQuery, GetArtifactQuery,
    ListApprovalsQuery, ListArtifactsQuery, ListNativeRunsRequest, ListNativeRunsResult,
    METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT, METHOD_DAEMON_RUN_EVENT, METHOD_DAEMON_RUN_FORK,
    METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST, METHOD_DAEMON_WORKSPACE_OPEN,
    NetworkPolicy, OutputContractKind, PatchResult, PermissionPolicy, ProcessExecPolicy,
    PublicApprovalResolution, PublicDaemonEvent, ResumeRunRequest, ResumeRunResult, ResumeRunState,
    RunEvent, RunEventDelta, RunEventStreamError, RunEventStreamItem, RunEventStreamPayload,
    RunHarnessKind, RunId, RunListEntry, RunListFilter, RunRecord, RunSource, RunStatus,
    RuntimeLanePendingState, RuntimePolicyMode, RuntimeProfileAuthProfilePatch, RuntimeProfileId,
    RuntimeProfileModelIdPatch, RuntimeProfilePatch, SandboxProfile, SessionAuthority, SessionId,
    SessionStatus, SessionSummary, StartRunCommand, StreamEmission, SubscribeRunEventsRequest,
    SubscribeRunEventsResult, TrustState, Workspace, WorkspaceId, WorkspaceMode, WorkspacePath,
    WorkspaceScope, WorkspaceSelector, WorktreeCleanupPolicy,
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
fn start_run_command_serializes_with_camel_case_fields() {
    let command = StartRunCommand {
        objective: "Ship protocol cleanup".to_string(),
        recipe_id: None,
        model_id: None,
    };

    let json = serde_json::to_value(&command).expect("command should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "objective": "Ship protocol cleanup"
        })
    );
}

#[test]
fn run_source_native_subagent_roundtrips_through_json() {
    let source = RunSource::NativeSubagent {
        parent_run_id: RunId::new("run-parent").expect("parent run id"),
        parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("parent turn id"),
        output_contract: None,
        model_id: None,
        recipe_id: None,
        workspace_scope: WorkspaceMode::WorktreeWrite,
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
        planned_write_files: Vec::new(),
    };

    let json = serde_json::to_value(&source).expect("run source should serialize");
    let decoded: RunSource = serde_json::from_value(json.clone()).expect("run source roundtrip");

    assert_eq!(decoded, source);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "nativeSubagent",
            "parentRunId": "run-parent",
            "parentTurnId": "turn-parent",
            "workspaceScope": "worktreeWrite",
            "cleanupPolicy": "deleteOnSuccess"
        })
    );
}

#[test]
fn run_completion_event_roundtrips_with_capsule_result() {
    let result = CapsuleResult::Patch(PatchResult {
        patch_receipt_ids: vec!["receipt_patch".to_string()],
        touched_files: vec!["crates/ta-protocol/src/wire/event.rs".to_string()],
        tests_run_receipt_ids: vec!["receipt_tests".to_string()],
        passing: true,
        blockers: Vec::new(),
    });
    let event = RunEvent {
        run_id: RunId::new("run-1").expect("run id"),
        status: RunStatus::Completed,
        detail: "completed".to_string(),
        output_contract: Some(OutputContractKind::Patch),
        recipe_id: None,
        result: Some(result.clone()),
    };

    let json = serde_json::to_value(&event).expect("run event should serialize");
    let decoded: RunEvent = serde_json::from_value(json.clone()).expect("run event roundtrip");

    assert_eq!(decoded, event);
    assert_eq!(json["outputContract"], "patch");
    assert_eq!(json["result"]["kind"], "patch");
}

#[test]
fn daemon_run_complete_with_result_params_roundtrip() {
    assert_eq!(
        METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT,
        "daemon.run.complete_with_result"
    );
    let params = DaemonRunCompleteWithResultParams {
        run_id: RunId::new("run-1").expect("run id"),
        detail: "completed".to_string(),
        result: None,
    };

    let json = serde_json::to_value(&params).expect("params should serialize");
    let decoded: DaemonRunCompleteWithResultParams =
        serde_json::from_value(json.clone()).expect("params should deserialize");

    assert_eq!(decoded, params);
    assert_eq!(
        json,
        serde_json::json!({
            "runId": "run-1",
            "detail": "completed"
        })
    );
}

#[test]
fn run_source_forked_roundtrips_through_json() {
    let source = RunSource::Forked {
        parent_run_id: RunId::new("run-parent").expect("parent run id"),
        parent_event_seq: 42,
    };

    let json = serde_json::to_value(&source).expect("run source should serialize");
    let decoded: RunSource = serde_json::from_value(json.clone()).expect("run source roundtrip");

    assert_eq!(decoded, source);
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "forked",
            "parentRunId": "run-parent",
            "parentEventSeq": "42"
        })
    );
}

#[test]
fn resume_run_contract_roundtrips_with_server_event_sequence_only() {
    let run_id = RunId::new("run-1").expect("run id");
    let request = ResumeRunRequest {
        run_id: run_id.clone(),
    };
    let result = ResumeRunResult {
        run: RunRecord {
            id: run_id,
            session_id: SessionId::new("session-1").expect("session id"),
            parent_run_id: Some(RunId::new("run-parent").expect("parent run id")),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Resume native work".to_string(),
            status: RunStatus::Running,
            harness: RunHarnessKind::Native,
            source: RunSource::NativeSubagent {
                parent_run_id: RunId::new("run-parent").expect("parent run id"),
                parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("turn id"),
                output_contract: None,
                model_id: None,
                recipe_id: None,
                workspace_scope: WorkspaceMode::WorktreeWrite,
                cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
                planned_write_files: Vec::new(),
            },
            execution_context: execution_context(),
            started_at_ms: Some(100),
            ended_at_ms: None,
            last_event_seq: Some(42),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        },
        state: ResumeRunState::Live,
        latest_event_seq: Some(42),
    };

    let request_json = serde_json::to_value(&request).expect("request should serialize");
    let result_json = serde_json::to_value(&result).expect("result should serialize");
    let decoded_request: ResumeRunRequest =
        serde_json::from_value(request_json.clone()).expect("request should deserialize");
    let decoded: ResumeRunResult =
        serde_json::from_value(result_json.clone()).expect("result should deserialize");

    assert_eq!(
        request_json,
        serde_json::json!({
            "runId": "run-1"
        })
    );
    assert_eq!(decoded_request, request);
    assert_eq!(result_json["latestEventSeq"], "42");
    assert_eq!(decoded, result);
}

#[test]
fn fork_run_contract_roundtrips_with_parent_event_seq() {
    let parent_run_id = RunId::new("run-parent").expect("parent run id");
    assert_eq!(METHOD_DAEMON_RUN_FORK, "daemon.run.fork");
    let request = ForkRunRequest {
        session_id: SessionId::new("session-1").expect("session id"),
        parent_run_id: parent_run_id.clone(),
        parent_event_seq: 42,
        objective: Some("Try a different path".to_string()),
    };
    let result = ForkRunResult {
        run: RunRecord {
            id: RunId::new("run-fork").expect("run id"),
            session_id: SessionId::new("session-1").expect("session id"),
            parent_run_id: Some(parent_run_id.clone()),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Try a different path".to_string(),
            status: RunStatus::Queued,
            harness: RunHarnessKind::Native,
            source: RunSource::Forked {
                parent_run_id,
                parent_event_seq: 42,
            },
            execution_context: execution_context(),
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: Some(43),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        },
    };

    let request_json = serde_json::to_value(&request).expect("request should serialize");
    let result_json = serde_json::to_value(&result).expect("result should serialize");
    let decoded_request: ForkRunRequest =
        serde_json::from_value(request_json.clone()).expect("request should deserialize");
    let decoded_result: ForkRunResult =
        serde_json::from_value(result_json.clone()).expect("result should deserialize");

    assert_eq!(
        request_json,
        serde_json::json!({
            "sessionId": "session-1",
            "parentRunId": "run-parent",
            "parentEventSeq": "42",
            "objective": "Try a different path"
        })
    );
    assert_eq!(result_json["run"]["lastEventSeq"], "43");
    assert_eq!(decoded_request, request);
    assert_eq!(decoded_result, result);
}

#[test]
fn resume_run_request_ignores_unknown_event_sequence_field() {
    let decoded: ResumeRunRequest = serde_json::from_value(serde_json::json!({
        "runId": "run-1",
        "lastEventSeq": 1
    }))
    .expect("unknown resume request fields are ignored by serde");

    assert_eq!(
        decoded,
        ResumeRunRequest {
            run_id: RunId::new("run-1").expect("run id")
        }
    );
}

#[test]
fn replay_run_events_contract_roundtrips_after_sequence() {
    let run_id = RunId::new("run-1").expect("run id");
    assert_eq!(METHOD_DAEMON_RUN_REPLAY_EVENTS, "daemon.run.replay_events");
    assert_eq!(
        METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
        "daemon.run.subscribe_events"
    );

    let request = SubscribeRunEventsRequest {
        session_id: SessionId::new("session-1").expect("session id"),
        run_id: run_id.clone(),
        after_seq: Some(41),
    };
    let result = SubscribeRunEventsResult {
        events: vec![RunEventDelta {
            seq: 42,
            event: PublicDaemonEvent::AgentStream(AgentStreamEvent {
                run_id,
                emission: StreamEmission {
                    turn_id: Some(AgentStreamTurnId::new("turn-1").expect("turn id")),
                    item_id: None,
                    fragment_sequence: None,
                    frame: AgentStreamFrame::AssistantTurnCompleted,
                },
            }),
        }],
        latest_event_seq: Some(42),
    };

    let request_json = serde_json::to_value(&request).expect("request should serialize");
    let result_json = serde_json::to_value(&result).expect("result should serialize");
    let decoded_request: SubscribeRunEventsRequest =
        serde_json::from_value(request_json.clone()).expect("request should deserialize");
    let decoded_result: SubscribeRunEventsResult =
        serde_json::from_value(result_json.clone()).expect("result should deserialize");

    assert_eq!(
        request_json,
        serde_json::json!({
            "sessionId": "session-1",
            "runId": "run-1",
            "afterSeq": "41"
        })
    );
    assert_eq!(result_json["events"][0]["seq"], "42");
    assert_eq!(result_json["latestEventSeq"], "42");
    assert_eq!(decoded_request, request);
    assert_eq!(decoded_result, result);
}

#[test]
fn replay_run_event_stream_error_contract_roundtrips() {
    let lagged_json =
        serde_json::to_value(RunEventStreamError::Lagged).expect("lagged should serialize");
    let history_gap_json = serde_json::to_value(RunEventStreamError::HistoryGap)
        .expect("history gap should serialize");

    assert_eq!(lagged_json, serde_json::json!("lagged"));
    assert_eq!(history_gap_json, serde_json::json!("historyGap"));
    assert_eq!(
        serde_json::from_value::<RunEventStreamError>(lagged_json).expect("lagged roundtrip"),
        RunEventStreamError::Lagged
    );
    assert_eq!(
        serde_json::from_value::<RunEventStreamError>(history_gap_json)
            .expect("history gap roundtrip"),
        RunEventStreamError::HistoryGap
    );
}

#[test]
fn run_event_stream_item_contract_roundtrips_delta_and_error() {
    assert_eq!(METHOD_DAEMON_RUN_EVENT, "daemon.run.event");
    let run_id = RunId::new("run-stream").expect("run id");
    let delta_item = RunEventStreamItem {
        run_id: run_id.clone(),
        payload: RunEventStreamPayload::Delta {
            delta: RunEventDelta {
                seq: 42,
                event: PublicDaemonEvent::AgentStream(AgentStreamEvent {
                    run_id: run_id.clone(),
                    emission: StreamEmission {
                        turn_id: Some(AgentStreamTurnId::new("turn-1").expect("turn id")),
                        item_id: None,
                        fragment_sequence: None,
                        frame: AgentStreamFrame::AssistantTurnCompleted,
                    },
                }),
            },
        },
    };
    let error_item = RunEventStreamItem {
        run_id: run_id.clone(),
        payload: RunEventStreamPayload::Error {
            error: RunEventStreamError::HistoryGap,
        },
    };

    let delta_json = serde_json::to_value(&delta_item).expect("delta item should serialize");
    let error_json = serde_json::to_value(&error_item).expect("error item should serialize");
    let decoded_delta: RunEventStreamItem =
        serde_json::from_value(delta_json.clone()).expect("delta item should deserialize");
    let decoded_error: RunEventStreamItem =
        serde_json::from_value(error_json.clone()).expect("error item should deserialize");

    assert_eq!(delta_json["runId"], "run-stream");
    assert_eq!(delta_json["payload"]["kind"], "delta");
    assert_eq!(delta_json["payload"]["delta"]["seq"], "42");
    assert_eq!(error_json["payload"]["kind"], "error");
    assert_eq!(error_json["payload"]["error"], "historyGap");
    assert_eq!(decoded_delta, delta_item);
    assert_eq!(decoded_error, error_item);
}

#[test]
fn list_native_runs_contract_roundtrips_with_cursor_and_parent_filter() {
    let request = ListNativeRunsRequest {
        filter: Some(RunListFilter {
            harness: Some(vec![RunHarnessKind::Native]),
            status: Some(vec![RunStatus::Running]),
            parent_run_id: Some(RunId::new("run-parent").expect("parent run id")),
        }),
        limit: 25,
        cursor: Some("100:run-parent".to_string()),
    };
    let result = ListNativeRunsResult {
        runs: vec![RunListEntry {
            id: RunId::new("run-child").expect("run id"),
            parent_run_id: Some(RunId::new("run-parent").expect("parent run id")),
            output_contract: None,
            recipe_id: None,
            harness: RunHarnessKind::Native,
            status: RunStatus::Running,
            started_at_ms: Some(120),
            ended_at_ms: None,
            last_event_seq: Some(42),
            objective_preview: Some("Review native child".to_string()),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        }],
        next_cursor: Some("120:run-child".to_string()),
    };

    let request_json = serde_json::to_value(&request).expect("request should serialize");
    let result_json = serde_json::to_value(&result).expect("result should serialize");
    let decoded_request: ListNativeRunsRequest =
        serde_json::from_value(request_json.clone()).expect("request should deserialize");
    let decoded_result: ListNativeRunsResult =
        serde_json::from_value(result_json.clone()).expect("result should deserialize");

    assert_eq!(
        request_json,
        serde_json::json!({
            "filter": {
                "harness": ["native"],
                "status": ["running"],
                "parentRunId": "run-parent"
            },
            "limit": 25,
            "cursor": "100:run-parent"
        })
    );
    assert_eq!(result_json["runs"][0]["startedAtMs"], "120");
    assert_eq!(result_json["runs"][0]["lastEventSeq"], "42");
    assert_eq!(decoded_request, request);
    assert_eq!(decoded_result, result);
}

#[test]
fn daemon_event_roundtrips_through_json() {
    let event = DaemonEvent::Artifact(ArtifactEvent {
        artifact: ArtifactSummary {
            id: ArtifactId::new("artifact-1").expect("artifact id should be valid"),
            run_id: RunId::new("run-1").expect("run id should be valid"),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        },
    });

    let json = serde_json::to_value(&event).expect("event should serialize");
    let decoded: DaemonEvent = serde_json::from_value(json).expect("event should deserialize");

    assert_eq!(decoded, event);
}

#[test]
fn agent_stream_event_roundtrips_through_json() {
    let event = DaemonEvent::AgentStream(AgentStreamEvent {
        run_id: RunId::new("run-1").expect("run id should be valid"),
        emission: StreamEmission {
            turn_id: Some(AgentStreamTurnId::new("turn-1").expect("turn id should be valid")),
            item_id: Some(AgentStreamItemId::new("item-1").expect("item id should be valid")),
            fragment_sequence: Some(3),
            frame: AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed,
            },
        },
    });

    let json = serde_json::to_value(&event).expect("event should serialize");
    let decoded: DaemonEvent =
        serde_json::from_value(json.clone()).expect("event should deserialize");

    assert_eq!(decoded, event);
    assert_eq!(
        json,
        serde_json::json!({
            "agentStream": {
                "runId": "run-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "fragmentSequence": 3,
                "frame": {
                    "kind": "toolCallCompleted",
                    "outcome": "completed"
                }
            }
        })
    );
}

#[test]
fn daemon_session_open_params_serialize_with_camel_case_fields() {
    let params = DaemonSessionOpenParams {
        title: "Build daemon app server".to_string(),
        workspace: WorkspaceSelector::ById {
            id: WorkspaceId::new("workspace-test-default").expect("workspace id"),
        },
    };

    let json = serde_json::to_value(&params).expect("open params should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "title": "Build daemon app server",
            "workspace": {
                "kind": "byId",
                "id": "workspace-test-default"
            }
        })
    );
}

#[test]
fn daemon_session_open_params_support_workspace_selector_by_path() {
    let params = DaemonSessionOpenParams {
        title: "Build daemon app server".to_string(),
        workspace: WorkspaceSelector::ByPath {
            path: WorkspacePath::from_canonical_wire_value("/tmp/taugentic-workspace")
                .expect("workspace path"),
            trust_acknowledged: true,
        },
    };

    let json = serde_json::to_value(&params).expect("open params should serialize");
    let decoded: DaemonSessionOpenParams =
        serde_json::from_value(json.clone()).expect("open params should deserialize");

    assert_eq!(decoded, params);
    assert_eq!(
        json,
        serde_json::json!({
            "title": "Build daemon app server",
            "workspace": {
                "kind": "byPath",
                "path": "/tmp/taugentic-workspace",
                "trustAcknowledged": true
            }
        })
    );
}

#[test]
fn daemon_workspace_rpc_payloads_roundtrip_through_json() {
    assert_eq!(METHOD_DAEMON_WORKSPACE_OPEN, "daemon.workspace.open");
    assert_eq!(METHOD_DAEMON_WORKSPACE_LIST, "daemon.workspace.list");
    assert_eq!(METHOD_DAEMON_WORKSPACE_GET, "daemon.workspace.get");

    let workspace = workspace_summary();
    let open_params = DaemonWorkspaceOpenParams {
        path: workspace.root_realpath.clone(),
        trust_acknowledged: false,
    };
    let open_result = DaemonWorkspaceOpenResult {
        workspace: workspace.clone(),
    };
    let list_params = DaemonWorkspaceListParams::default();
    let list_result = DaemonWorkspaceListResult {
        workspaces: vec![workspace.clone()],
    };
    let get_params = DaemonWorkspaceGetParams {
        id: workspace.id.clone(),
    };
    let get_result = DaemonWorkspaceGetResult { workspace };

    let open_json = serde_json::to_value(&open_params).expect("workspace open params serialize");
    let list_json = serde_json::to_value(&list_params).expect("workspace list params serialize");
    assert_eq!(open_json["trustAcknowledged"], false);
    assert_eq!(list_json, serde_json::json!({}));
    assert_eq!(
        serde_json::from_value::<DaemonWorkspaceOpenParams>(open_json).expect("open params decode"),
        open_params
    );
    assert_eq!(
        serde_json::from_value::<DaemonWorkspaceOpenResult>(
            serde_json::to_value(&open_result).expect("open result serialize")
        )
        .expect("open result decode"),
        open_result
    );
    assert_eq!(
        serde_json::from_value::<DaemonWorkspaceListResult>(
            serde_json::to_value(&list_result).expect("list result serialize")
        )
        .expect("list result decode"),
        list_result
    );
    assert_eq!(
        serde_json::from_value::<DaemonWorkspaceGetParams>(
            serde_json::to_value(&get_params).expect("get params serialize")
        )
        .expect("get params decode"),
        get_params
    );
    assert_eq!(
        serde_json::from_value::<DaemonWorkspaceGetResult>(
            serde_json::to_value(&get_result).expect("get result serialize")
        )
        .expect("get result decode"),
        get_result
    );
}

#[test]
fn artifact_get_query_serializes_with_camel_case_fields() {
    let query = GetArtifactQuery {
        artifact_id: ta_protocol::wire::ArtifactId::new("artifact-1")
            .expect("artifact id should be valid"),
    };

    let json = serde_json::to_value(&query).expect("query should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "artifactId": "artifact-1"
        })
    );
}

#[test]
fn get_agent_runtime_query_serializes_as_empty_object() {
    let query = GetAgentRuntimeQuery::default();

    let json = serde_json::to_value(&query).expect("query should serialize");

    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn runtime_profile_patch_roundtrips_set_and_clear_ops() {
    let patch = RuntimeProfilePatch {
        display_name: Some("Mission Control".to_string()),
        provider_id: Some(
            AgentRuntimeStrategyId::new("provider-codex").expect("provider id should be valid"),
        ),
        model_id: Some(RuntimeProfileModelIdPatch::Set {
            value: AgentRuntimeModelId::new("gpt-5.4").expect("model id should be valid"),
        }),
        auth_profile: Some(RuntimeProfileAuthProfilePatch::Clear),
        policy_mode: Some(RuntimePolicyMode::RequireApproval),
    };

    let json = serde_json::to_value(&patch).expect("patch should serialize");
    let decoded: RuntimeProfilePatch =
        serde_json::from_value(json.clone()).expect("patch should deserialize");

    assert_eq!(
        json,
        serde_json::json!({
            "displayName": "Mission Control",
            "providerId": "provider-codex",
            "modelId": {
                "kind": "set",
                "value": "gpt-5.4"
            },
            "authProfile": {
                "kind": "clear"
            },
            "policyMode": "requireApproval"
        })
    );
    assert_eq!(decoded, patch);
}

#[test]
fn agent_runtime_select_profile_params_require_runtime_profile_id() {
    let params = ta_protocol::wire::DaemonAgentRuntimeSelectProfileParams {
        runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
            .expect("runtime profile id"),
    };

    let json = serde_json::to_value(&params).expect("params should serialize");
    let decoded: ta_protocol::wire::DaemonAgentRuntimeSelectProfileParams =
        serde_json::from_value(json.clone()).expect("params should deserialize");

    assert_eq!(
        json,
        serde_json::json!({
            "runtimeProfileId": "runtime-codex-safe"
        })
    );
    assert_eq!(decoded, params);
}

#[test]
fn approval_list_query_serializes_with_camel_case_fields() {
    let query = ListApprovalsQuery {
        run_id: Some(RunId::new("run-1").expect("run id should be valid")),
        approval_id: None,
    };

    let json = serde_json::to_value(&query).expect("query should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "runId": "run-1"
        })
    );
}

#[test]
fn artifact_list_query_serializes_with_camel_case_fields() {
    let query = ListArtifactsQuery {
        run_id: None,
        artifact_id: None,
    };

    let json = serde_json::to_value(&query).expect("query should serialize");

    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn approval_snapshot_result_roundtrips_through_json() {
    let result = ApprovalSnapshotResult {
        items: vec![ApprovalRequest {
            id: ta_protocol::wire::ApprovalId::new("approval-1").expect("approval id"),
            run_id: RunId::new("run-1").expect("run id"),
            tool_call_id: Some(
                ta_protocol::wire::AgentStreamItemId::new("tool-call-1").expect("tool call id"),
            ),
            scope: ApprovalScope::ProcessExec,
            requested_at_ms: 100,
            expires_at_ms: 200,
            target: ApprovalTarget::ToolCall {
                tool_name: "shell".to_string(),
            },
            reason: "Need shell".to_string(),
        }],
        latest_cursor: Some(daemon_event_cursor(12)),
    };

    let json = serde_json::to_value(&result).expect("approval snapshot should serialize");
    let decoded: ApprovalSnapshotResult =
        serde_json::from_value(json).expect("approval snapshot should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn approval_request_roundtrips_tool_call_id() {
    let request = ApprovalRequest::new(
        ta_protocol::wire::ApprovalId::new("approval-1").expect("approval id"),
        RunId::new("run-1").expect("run id"),
        ApprovalScope::ProcessExec,
        100,
        200,
        ApprovalTarget::ToolCall {
            tool_name: "shell".to_string(),
        },
        "Need shell",
    )
    .expect("approval request")
    .with_tool_call_id(
        ta_protocol::wire::AgentStreamItemId::new("tool-call-1").expect("tool call id"),
    );

    let json = serde_json::to_value(&request).expect("request should serialize");
    let decoded: ApprovalRequest =
        serde_json::from_value(json).expect("request should deserialize");

    assert_eq!(decoded, request);
}

#[test]
fn artifact_snapshot_result_roundtrips_through_json() {
    let result = ArtifactSnapshotResult {
        items: vec![ArtifactSummary {
            id: ArtifactId::new("artifact-1").expect("artifact id"),
            run_id: RunId::new("run-1").expect("run id"),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        }],
        latest_cursor: Some(daemon_event_cursor(12)),
    };

    let json = serde_json::to_value(&result).expect("artifact snapshot should serialize");
    let decoded: ArtifactSnapshotResult =
        serde_json::from_value(json).expect("artifact snapshot should deserialize");

    assert_eq!(decoded, result);
}

#[test]
fn activity_page_query_serializes_with_camel_case_fields() {
    let query = ActivityPageQuery {
        limit: 25,
        before: Some(ActivityCursor { sequence: 42 }),
        kinds: vec![DaemonEventKind::Artifact],
    };

    let json = serde_json::to_value(&query).expect("query should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "limit": 25,
            "before": {
                "sequence": "42"
            },
            "kinds": ["artifact"]
        })
    );
}

#[test]
fn daemon_subscribe_params_roundtrip_with_agent_stream_kind() {
    let params = DaemonSubscribeParams {
        kinds: vec![DaemonEventKind::AgentStream],
        after_cursor: Some(daemon_event_cursor(5)),
    };

    let json = serde_json::to_value(&params).expect("subscribe params should serialize");
    let decoded: DaemonSubscribeParams =
        serde_json::from_value(json.clone()).expect("subscribe params should deserialize");

    assert_eq!(decoded, params);
    assert_eq!(
        json,
        serde_json::json!({
            "kinds": ["agentStream"],
            "afterCursor": {
                "daemonInstanceId": "daemon-1",
                "sessionId": "session-1",
                "sequence": "5"
            }
        })
    );
}

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
        ta_protocol::wire::ApprovalId::new("approval-1").expect("approval id"),
        RunId::new("run-1").expect("run id"),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("principal-ta-cli").expect("approval actor"),
        Some("looks safe".to_string()),
    )
    .with_tool_call_id(
        ta_protocol::wire::AgentStreamItemId::new("tool-call-1").expect("tool call id"),
    );

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

fn workspace_summary() -> Workspace {
    Workspace {
        id: WorkspaceId::new("workspace-test-default").expect("workspace id"),
        root_realpath: WorkspacePath::from_canonical_wire_value("/tmp/taugentic-workspace")
            .expect("workspace path"),
        display_name: "taugentic-workspace".to_string(),
        trust_state: TrustState::UserConfirmed {
            confirmed_at: "2026-05-09T00:00:00Z".to_string(),
        },
        git_repo_root: None,
        created_at: "2026-05-09T00:00:00Z".to_string(),
        last_used_at: "2026-05-09T00:00:00Z".to_string(),
    }
}

fn session_authority() -> SessionAuthority {
    SessionAuthority::new("session-authority-1session-authority-1".to_string())
        .expect("session authority")
}
