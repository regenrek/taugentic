use ta_protocol::wire::{
    ActivityCursor, ActivityPageQuery, AgentRuntimeMediaCapabilities, AgentRuntimeMediaCapability,
    AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeSelection, AgentRuntimeStrategyId,
    AgentStreamEvent, AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome,
    ApprovalActor, ApprovalDecision, ApprovalRequest, ApprovalResolution, ApprovalResolutionReason,
    ApprovalScope, ApprovalSnapshotResult, ApprovalTarget, ArtifactEvent, ArtifactId, ArtifactKind,
    ArtifactMetadata, ArtifactSnapshotResult, ArtifactSummary, AuthProfileId, CapsuleResult,
    ContinueRunRequest, ContinueRunResult, DaemonEvent, DaemonEventCursor, DaemonEventEnvelope,
    DaemonEventKind, DaemonNavigationInvalidatedParams, DaemonNavigationSubscribeParams,
    DaemonNavigationSubscribeResult, DaemonProjectOpenParams, DaemonProjectOpenResult,
    DaemonRunCompleteWithResultParams, DaemonSessionAttachParams, DaemonSessionAttachResult,
    DaemonSessionOpenParams, DaemonSessionOpenResult, DaemonSubscribeParams, DaemonSubscribeResult,
    DaemonWorkspaceGetParams, DaemonWorkspaceGetResult, DaemonWorkspaceListParams,
    DaemonWorkspaceListResult, DaemonWorkspaceOpenParams, DaemonWorkspaceOpenResult, EnvPolicy,
    ExecutionContext, ForkRunRequest, ForkRunResult, GetAgentRuntimeQuery, GetArtifactQuery,
    JoinRunRequest, ListApprovalsQuery, ListArtifactsQuery, ListNativeRunsRequest,
    ListNativeRunsResult, METHOD_DAEMON_PROJECT_OPEN, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT,
    METHOD_DAEMON_RUN_CONTINUE, METHOD_DAEMON_RUN_EVENT, METHOD_DAEMON_RUN_FORK,
    METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST, METHOD_DAEMON_WORKSPACE_OPEN,
    NativeRunRelationship, NetworkPolicy, OutputContractKind, PatchResult, PermissionPolicy,
    ProcessExecPolicy, PublicApprovalResolution, PublicDaemonEvent, ResumeRunRequest,
    ResumeRunResult, ResumeRunState, RunEvent, RunEventDelta, RunEventStreamError,
    RunEventStreamItem, RunEventStreamPayload, RunExecutionRoute, RunHarnessKind, RunId,
    RunListEntry, RunListFilter, RunRecord, RunSource, RunStatus, RunStatusEvent, RunSummary,
    RuntimeLanePendingState, RuntimePolicyMode, RuntimeProfileId, RuntimeProfilePatch,
    SandboxProfile, SessionAuthority, SessionId, SessionStatus, SessionSummary, SpawnRunRequest,
    StartRunCommand, StreamEmission, SubscribeRunEventsRequest, SubscribeRunEventsResult,
    ThreadWorkspaceMutation, ThreadWorkspacePin, ThreadWorkspaceQuery,
    ThreadWorkspaceUpdateCommand, ThreadWorkspaceWorkLogEntry, ThreadWorkspaceWorkLogKind,
    TrustState, Workspace, WorkspaceId, WorkspaceMode, WorkspacePath, WorkspaceScope,
    WorkspaceSelector, WorktreeCleanupPolicy,
};

#[test]
fn navigation_project_space_intent_uses_optional_camel_case_space_id() {
    let project_id = ta_protocol::wire::ProjectId::new("project-desktop").expect("project id");
    let space_id = ta_protocol::wire::SpaceId::new("space-product").expect("space id");
    let placed = ta_protocol::wire::DaemonNavigationIntent::SetProjectSpace {
        project_id: project_id.clone(),
        space_id: Some(space_id),
    };
    assert_eq!(
        serde_json::to_value(&placed).expect("placed intent serializes"),
        serde_json::json!({
            "kind": "setProjectSpace",
            "projectId": "project-desktop",
            "spaceId": "space-product"
        })
    );
    let ungrouped = ta_protocol::wire::DaemonNavigationIntent::SetProjectSpace {
        project_id,
        space_id: None,
    };
    assert_eq!(
        serde_json::to_value(&ungrouped).expect("ungrouped intent serializes"),
        serde_json::json!({ "kind": "setProjectSpace", "projectId": "project-desktop" })
    );
}

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

fn execution_route() -> RunExecutionRoute {
    RunExecutionRoute {
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        provider_id: AgentRuntimeStrategyId::new("openai").expect("provider id"),
        harness: RunHarnessKind::Native,
        model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
        auth_profile_id: Some(
            ta_protocol::wire::AuthProfileId::new("profile-test").expect("auth profile id"),
        ),
    }
}

#[test]
fn scheduled_work_source_roundtrips_with_frozen_occurrence_link() {
    let context = execution_context();
    let definition = ta_protocol::wire::ScheduledWorkDefinition {
        id: ta_protocol::wire::ScheduledWorkId::new("schedule-release").expect("schedule id"),
        session_id: SessionId::new("session-scheduled").expect("session id"),
        objective: "Run the release checks".to_string(),
        route: execution_route(),
        execution_request: ta_protocol::wire::ScheduledWorkExecutionRequest {
            workspace_id: context.workspace_id.clone(),
            workspace_root: context.workspace_root.clone(),
            repo_root: context.workspace_root.clone(),
            artifact_root: context.artifact_root.clone(),
            workspace_mode: ta_protocol::wire::WorkspaceMode::WorkspaceWrite,
            cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
            workspace_scope: context.workspace_scope.clone(),
            sandbox_profile: context.sandbox_profile.clone(),
            permission_policy: context.permission_policy,
            network_policy: context.network_policy.clone(),
            env_policy: context.env_policy.clone(),
        },
        due_at_ms: 1_700_000_000_000,
        attention_policy: ta_protocol::wire::ScheduledWorkAttentionPolicy::AttentionOnly,
    };
    definition.validate().expect("frozen definition validates");
    assert!(
        definition
            .execution_request
            .matches_execution_context(&context)
    );
    let source = RunSource::ScheduledWork {
        route: definition.route.clone(),
        scheduled_work_id: definition.id.clone(),
        occurrence_id: ta_protocol::wire::ScheduledWorkOccurrenceId::new("occurrence-release")
            .expect("occurrence id"),
    };
    let json = serde_json::to_value(&source).expect("source serializes");
    assert_eq!(json["kind"], "scheduledWork");
    assert_eq!(RunSource::route(&source), &definition.route);
    assert_eq!(
        serde_json::from_value::<RunSource>(json).expect("source deserializes"),
        source
    );
}

#[test]
fn start_run_command_serializes_with_camel_case_fields() {
    let command = StartRunCommand {
        objective: "Ship protocol cleanup".to_string(),
        recipe_id: None,
        selection: AgentRuntimeSelection {
            runtime_profile_id: execution_route().runtime_profile_id,
            auth_profile_id: execution_route().auth_profile_id,
            model_id: execution_route().model_id,
        },
        attachments: Vec::new(),
    };

    let json = serde_json::to_value(&command).expect("command should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "objective": "Ship protocol cleanup",
            "selection": {
                "runtimeProfileId": "runtime-openai-safe",
                "authProfileId": "profile-test",
                "modelId": "gpt-5.6-sol"
            },
            "attachments": []
        })
    );
}

#[test]
fn continue_run_contract_roundtrips_with_session_and_message() {
    let request = ContinueRunRequest {
        session_id: SessionId::new("session-continue").expect("session id"),
        run_id: RunId::new("run-continue").expect("run id"),
        message: "Continue from the durable branch.".to_string(),
    };
    let encoded = serde_json::to_value(&request).expect("serialize continuation request");
    assert_eq!(encoded["sessionId"], "session-continue");
    assert_eq!(encoded["runId"], "run-continue");
    assert_eq!(encoded["message"], "Continue from the durable branch.");
    assert_eq!(
        serde_json::from_value::<ContinueRunRequest>(encoded).expect("roundtrip"),
        request
    );
    assert_eq!(METHOD_DAEMON_RUN_CONTINUE, "daemon.run.continue");
    let result = ContinueRunResult {
        run: RunRecord {
            id: RunId::new("run-continue").expect("run id"),
            session_id: SessionId::new("session-continue").expect("session id"),
            parent_run_id: None,
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe").expect("profile"),
            objective: "Continue from the durable branch.".to_string(),
            status: RunStatus::Running,
            harness: RunHarnessKind::Native,
            source: RunSource::User {
                route: execution_route(),
                output_contract: None,
                model_id: None,
                recipe_id: None,
                attachments: Vec::new(),
            },
            execution_context: execution_context(),
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        },
    };
    assert_eq!(
        serde_json::from_value::<ContinueRunResult>(
            serde_json::to_value(&result).expect("serialize result")
        )
        .expect("result roundtrip"),
        result
    );
}

#[test]
fn navigation_subscription_contract_is_strictly_empty() {
    assert_eq!(
        serde_json::to_value(DaemonNavigationSubscribeParams {}).expect("serialize params"),
        serde_json::json!({})
    );
    assert_eq!(
        serde_json::to_value(DaemonNavigationSubscribeResult {}).expect("serialize result"),
        serde_json::json!({})
    );
    assert_eq!(
        serde_json::to_value(DaemonNavigationInvalidatedParams {}).expect("serialize notification"),
        serde_json::json!({})
    );
    assert!(
        serde_json::from_value::<DaemonNavigationSubscribeParams>(
            serde_json::json!({ "cursor": "forbidden" })
        )
        .is_err()
    );
}

#[test]
fn run_source_native_subagent_roundtrips_through_json() {
    let source = RunSource::NativeSubagent {
        route: execution_route(),
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
            "route": {
                "runtimeProfileId": "runtime-openai-safe",
                "providerId": "openai",
                "harness": "native",
                "modelId": "gpt-5.6-sol",
                "authProfileId": "profile-test"
            },
            "parentRunId": "run-parent",
            "parentTurnId": "turn-parent",
            "workspaceScope": "worktreeWrite",
            "cleanupPolicy": "deleteOnSuccess"
        })
    );
}

#[test]
fn run_status_event_constructor_roundtrips_with_capsule_result_and_validates_reason_categories() {
    let result = CapsuleResult::Patch(PatchResult {
        patch_receipt_ids: vec!["receipt_patch".to_string()],
        touched_files: vec!["crates/ta-protocol/src/wire/event.rs".to_string()],
        tests_run_receipt_ids: vec!["receipt_tests".to_string()],
        passing: true,
        blockers: Vec::new(),
    });
    let event = RunEvent::terminal(
        RunId::new("run-1").expect("run id"),
        RunStatus::Completed,
        ta_protocol::wire::RunStatusReason::new("completed").expect("reason"),
        Some(OutputContractKind::Patch),
        None,
        Some(result.clone()),
    )
    .expect("completed is terminal");

    let json = serde_json::to_value(&event).expect("run event should serialize");
    let decoded: RunEvent = serde_json::from_value(json.clone()).expect("run event roundtrip");

    assert_eq!(decoded, event);
    assert_eq!(json["kind"], "status");
    assert_eq!(json["payload"]["outputContract"], "patch");
    assert_eq!(json["payload"]["result"]["kind"], "patch");
    assert!(
        serde_json::from_value::<RunEvent>(serde_json::json!({
            "kind": "status",
            "payload": { "runId": "run-invalid", "status": "running", "reason": "nope" }
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<RunEvent>(serde_json::json!({
            "kind": "status",
            "payload": { "runId": "run-invalid", "status": "completed" }
        }))
        .is_err()
    );
}

#[test]
fn exhausted_profile_status_roundtrips_without_profile_id_ownership() {
    let event = RunEvent::terminal_with_auth_profile_exhaustion(
        RunId::new("run-exhausted").expect("run id"),
        ta_protocol::wire::RunStatusReason::new("The selected account is rate limited.")
            .expect("reason"),
        ta_protocol::wire::AuthProfileExhaustion::RateLimited,
    )
    .expect("typed exhaustion status");

    let value = serde_json::to_value(&event).expect("status serializes");
    assert_eq!(value["payload"]["authProfileExhaustion"], "rateLimited");
    assert!(value["payload"].get("authProfileId").is_none());
    assert_eq!(
        serde_json::from_value::<RunEvent>(value).expect("status roundtrips"),
        event
    );
}

#[test]
fn run_event_active_terminal_constructors_enforce_status_categories_and_reason() {
    let run_id = RunId::new("run-status-constructor").expect("run id");
    let active = RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
        .expect("running is active");
    assert!(matches!(
        active,
        RunEvent::Status(ref event) if event.reason().is_none()
    ));
    assert_eq!(
        RunEvent::active(run_id.clone(), RunStatus::Completed, None, None, None),
        Err(ta_protocol::wire::DomainError::RunStatusMustBeActive)
    );

    let reason = ta_protocol::wire::RunStatusReason::new("completed").expect("reason");
    let terminal = RunEvent::terminal(
        run_id.clone(),
        RunStatus::Completed,
        reason,
        None,
        None,
        None,
    )
    .expect("completed is terminal");
    assert!(matches!(
        terminal,
        RunEvent::Status(ref event) if event.reason().is_some_and(|reason| reason.as_str() == "completed")
    ));
    assert_eq!(
        RunEvent::terminal(
            run_id,
            RunStatus::Running,
            ta_protocol::wire::RunStatusReason::new("not terminal").expect("reason"),
            None,
            None,
            None,
        ),
        Err(ta_protocol::wire::DomainError::RunStatusMustBeTerminal)
    );
    assert_eq!(
        ta_protocol::wire::RunStatusReason::new(" \t\n "),
        Err(ta_protocol::wire::DomainError::EmptyRunStatusReason)
    );
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
        route: execution_route(),
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
            "route": {
                "runtimeProfileId": "runtime-openai-safe",
                "providerId": "openai",
                "harness": "native",
                "modelId": "gpt-5.6-sol",
                "authProfileId": "profile-test"
            },
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
                route: execution_route(),
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
                route: execution_route(),
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
            relationship: NativeRunRelationship::FreshSpawn {
                parent_run_id: RunId::new("run-parent").expect("parent run id"),
            },
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
    assert_eq!(
        result_json["runs"][0]["relationship"],
        serde_json::json!({
            "kind": "freshSpawn",
            "parentRunId": "run-parent"
        })
    );
    assert_eq!(decoded_request, request);
    assert_eq!(decoded_result, result);
}

#[test]
fn route_switched_continuation_relationship_roundtrips_distinctly_from_a_fork() {
    let relationship = NativeRunRelationship::RouteSwitchedContinuation {
        route: RunExecutionRoute {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe").expect("runtime"),
            provider_id: AgentRuntimeStrategyId::new("codex").expect("provider"),
            harness: RunHarnessKind::CodexAppServer,
            model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model")),
            auth_profile_id: Some(AuthProfileId::new("profile-codex-test").expect("profile")),
        },
        parent_run_id: RunId::new("run-exhausted-parent").expect("parent run id"),
        parent_event_seq: 42,
    };
    let encoded = serde_json::to_value(&relationship).expect("relationship serializes");
    assert_eq!(
        encoded,
        serde_json::json!({
            "kind": "routeSwitchedContinuation",
            "route": {
                "runtimeProfileId": "runtime-codex-safe",
                "providerId": "codex",
                "harness": "codexAppServer",
                "modelId": "gpt-5.6-sol",
                "authProfileId": "profile-codex-test"
            },
            "parentRunId": "run-exhausted-parent",
            "parentEventSeq": "42"
        })
    );
    assert_eq!(
        serde_json::from_value::<NativeRunRelationship>(encoded)
            .expect("relationship deserializes"),
        relationship
    );
}

#[test]
fn fresh_spawn_and_join_contract_roundtrip() {
    let selection = AgentRuntimeSelection {
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe").expect("profile"),
        auth_profile_id: Some(ta_protocol::wire::AuthProfileId::new("profile-test").expect("auth")),
        model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model")),
    };
    let spawn = SpawnRunRequest {
        session_id: SessionId::new("session-fresh").expect("session"),
        parent_run_id: RunId::new("run-parent").expect("parent"),
        objective: "Review this independently".to_string(),
        selection,
        output_contract: None,
        recipe_id: None,
        workspace_scope: WorkspaceMode::WorkspaceWrite,
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
        planned_write_files: Vec::new(),
    };
    let join = JoinRunRequest {
        session_id: spawn.session_id.clone(),
        parent_run_id: spawn.parent_run_id.clone(),
        child_run_id: RunId::new("run-fresh-child").expect("child"),
    };

    let spawn_json = serde_json::to_value(&spawn).expect("spawn should serialize");
    let join_json = serde_json::to_value(&join).expect("join should serialize");
    let decoded_spawn: SpawnRunRequest =
        serde_json::from_value(spawn_json.clone()).expect("spawn should deserialize");
    let decoded_join: JoinRunRequest =
        serde_json::from_value(join_json.clone()).expect("join should deserialize");

    assert_eq!(spawn_json["parentRunId"], "run-parent");
    assert_eq!(
        spawn_json["selection"]["runtimeProfileId"],
        "runtime-openai-safe"
    );
    assert_eq!(join_json["childRunId"], "run-fresh-child");
    assert_eq!(decoded_spawn, spawn);
    assert_eq!(decoded_join, join);
}

#[test]
fn daemon_event_roundtrips_through_json() {
    let event = DaemonEvent::Artifact(ArtifactEvent {
        artifact: ArtifactSummary {
            id: ArtifactId::new("artifact-1").expect("artifact id should be valid"),
            run_id: RunId::new("run-1").expect("run id should be valid"),
            kind: ArtifactKind::Patch,
            metadata: ArtifactMetadata::Standard,
            display_name: "patch.diff".to_string(),
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
                "fragmentSequence": "3",
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
fn daemon_session_open_params_support_project_workspace_selector() {
    let params = DaemonSessionOpenParams {
        title: "Project conversation".to_string(),
        workspace: WorkspaceSelector::ByProject {
            project_id: ta_protocol::wire::ProjectId::new("project-desktop").expect("project id"),
            workspace_id: WorkspaceId::new("workspace-test-default").expect("workspace id"),
        },
    };

    let json = serde_json::to_value(&params).expect("open params should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "title": "Project conversation",
            "workspace": {
                "kind": "byProject",
                "projectId": "project-desktop",
                "workspaceId": "workspace-test-default"
            }
        })
    );
}

#[test]
fn daemon_session_open_params_support_temporary_workspace_selector() {
    let params = DaemonSessionOpenParams {
        title: "Temporary conversation".to_string(),
        workspace: WorkspaceSelector::ByTemporary {
            workspace_id: WorkspaceId::new("workspace-test-default").expect("workspace id"),
        },
    };

    let json = serde_json::to_value(&params).expect("open params should serialize");
    let decoded: DaemonSessionOpenParams =
        serde_json::from_value(json.clone()).expect("open params should deserialize");

    assert_eq!(decoded, params);
    assert_eq!(
        json,
        serde_json::json!({
            "title": "Temporary conversation",
            "workspace": {
                "kind": "byTemporary",
                "workspaceId": "workspace-test-default"
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
fn daemon_project_open_payload_roundtrips_with_navigation_snapshot() {
    assert_eq!(METHOD_DAEMON_PROJECT_OPEN, "daemon.project.open");
    let workspace = workspace_summary();
    let params = DaemonProjectOpenParams {
        path: workspace.root_realpath,
        trust_acknowledged: true,
    };
    let result = DaemonProjectOpenResult {
        project_id: ta_protocol::wire::ProjectId::new("project-open")
            .expect("project id should be valid"),
        snapshot: ta_protocol::wire::NavigationSnapshot {
            spaces: Vec::new(),
            projects: Vec::new(),
            conversations: Vec::new(),
            agents: Vec::new(),
        },
    };

    let params_json = serde_json::to_value(&params).expect("project open params serialize");
    let result_json = serde_json::to_value(&result).expect("project open result serialize");
    assert_eq!(params_json["trustAcknowledged"], true);
    assert_eq!(result_json["projectId"], "project-open");
    assert_eq!(
        serde_json::from_value::<DaemonProjectOpenParams>(params_json)
            .expect("project open params decode"),
        params
    );
    assert_eq!(
        serde_json::from_value::<DaemonProjectOpenResult>(result_json)
            .expect("project open result decode"),
        result
    );
}

#[test]
fn artifact_get_query_serializes_with_camel_case_fields() {
    let query = GetArtifactQuery {
        artifact_id: ta_protocol::wire::ArtifactId::new("artifact-1")
            .expect("artifact id should be valid"),
        pdf_page_index: None,
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
fn runtime_profile_patch_roundtrips_editable_fields() {
    let patch = RuntimeProfilePatch {
        display_name: Some("Mission Control".to_string()),
        policy_mode: Some(RuntimePolicyMode::RequireApproval),
    };

    let json = serde_json::to_value(&patch).expect("patch should serialize");
    let decoded: RuntimeProfilePatch =
        serde_json::from_value(json.clone()).expect("patch should deserialize");

    assert_eq!(
        json,
        serde_json::json!({
            "displayName": "Mission Control",
            "policyMode": "requireApproval"
        })
    );
    assert_eq!(decoded, patch);
}

#[test]
fn auth_profile_preferences_set_contract_roundtrips_as_a_complete_replacement() {
    let params = ta_protocol::wire::DaemonAgentRuntimeAuthProfilePreferencesSetParams {
        auth_profile_id: ta_protocol::wire::AuthProfileId::new("profile-openai-b")
            .expect("profile id"),
        preferences: ta_protocol::wire::AuthProfilePreferences {
            label: "Secondary OpenAI".to_string(),
            order: 1,
            is_default: false,
        },
    };

    let json = serde_json::to_value(&params).expect("params should serialize");
    let decoded: ta_protocol::wire::DaemonAgentRuntimeAuthProfilePreferencesSetParams =
        serde_json::from_value(json.clone()).expect("params should deserialize");

    assert_eq!(
        json,
        serde_json::json!({
            "authProfileId": "profile-openai-b",
            "preferences": {
                "label": "Secondary OpenAI",
                "order": 1,
                "isDefault": false
            }
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
            metadata: ArtifactMetadata::Standard,
            display_name: "patch.diff".to_string(),
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
                metadata: ArtifactMetadata::Standard,
                display_name: "patch.diff".to_string(),
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
        next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
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

#[test]
fn thread_workspace_contract_is_strict_and_roundtrips_every_mutation() {
    let query_error = serde_json::from_value::<ThreadWorkspaceQuery>(serde_json::json!({
        "unexpected": true
    }))
    .expect_err("empty query must reject unknown fields");
    assert!(query_error.to_string().contains("unknown field"));

    let pin = ThreadWorkspacePin {
        run_id: RunId::new("run-thread-workspace").expect("run id"),
        cursor: ActivityCursor { sequence: 7 },
    };
    let mutations = vec![
        ThreadWorkspaceMutation::GoalSet {
            value: "goal".to_string(),
        },
        ThreadWorkspaceMutation::PlanSet {
            value: "plan".to_string(),
        },
        ThreadWorkspaceMutation::NotesSet {
            value: "notes".to_string(),
        },
        ThreadWorkspaceMutation::RecapSet {
            value: "recap".to_string(),
        },
        ThreadWorkspaceMutation::PinAdded { pin: pin.clone() },
        ThreadWorkspaceMutation::PinRemoved {
            cursor: pin.cursor.clone(),
        },
    ];
    for mutation in mutations {
        let command = ThreadWorkspaceUpdateCommand { mutation };
        let value = serde_json::to_value(&command).expect("command serializes");
        assert_eq!(
            serde_json::from_value::<ThreadWorkspaceUpdateCommand>(value)
                .expect("command roundtrips"),
            command
        );
    }

    let error = serde_json::from_value::<ThreadWorkspaceUpdateCommand>(serde_json::json!({
        "mutation": { "kind": "goalSet", "value": "goal", "unexpected": true }
    }))
    .expect_err("update must reject unknown nested fields");
    assert!(error.to_string().contains("unknown field"));

    let entry = ThreadWorkspaceWorkLogEntry {
        sequence: 42,
        occurred_at_ms: 1_725_000_000_123,
        kind: ThreadWorkspaceWorkLogKind::GoalSet,
    };
    assert_eq!(
        serde_json::to_value(entry).expect("entry serializes"),
        serde_json::json!({
            "sequence": "42",
            "occurredAtMs": "1725000000123",
            "kind": "goalSet"
        })
    );
}

#[test]
fn image_media_contract_is_closed() {
    let model = AgentRuntimeModelRef {
        id: AgentRuntimeModelId::new("model-image").expect("model id"),
        display_name: "Image model".to_string(),
        context_limit: None,
        input_cost_per_million_micros: None,
        output_cost_per_million_micros: None,
        reasoning: true,
        tool_call: true,
        structured_output: false,
        media_capabilities: AgentRuntimeMediaCapabilities {
            image_input: AgentRuntimeMediaCapability::Supported,
            image_output: AgentRuntimeMediaCapability::Unsupported,
            voice_input: AgentRuntimeMediaCapability::Unsupported,
            voice_output: AgentRuntimeMediaCapability::Unsupported,
        },
    };
    let model_value = serde_json::to_value(model).expect("model serializes");
    assert!(model_value.get("inputModalities").is_none());
    let capabilities = AgentRuntimeMediaCapabilities {
        image_input: AgentRuntimeMediaCapability::Supported,
        image_output: AgentRuntimeMediaCapability::Unsupported,
        voice_input: AgentRuntimeMediaCapability::Unsupported,
        voice_output: AgentRuntimeMediaCapability::Unsupported,
    };
    assert_eq!(
        serde_json::from_value::<AgentRuntimeMediaCapabilities>(
            serde_json::to_value(capabilities.clone()).expect("capabilities serialize"),
        )
        .expect("capabilities deserialize"),
        capabilities
    );
}
