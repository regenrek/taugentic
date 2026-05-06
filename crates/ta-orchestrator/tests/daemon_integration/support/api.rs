use super::*;

pub fn open_session(
    stream: &mut SocketConnection,
    request_id: RequestId,
    title: &str,
) -> DaemonSessionOpenResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SESSION_OPEN,
            Some(
                serde_json::to_value(DaemonSessionOpenParams {
                    title: title.to_string(),
                })
                .expect("session open params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("session open result should deserialize")
}

pub fn list_sessions(stream: &mut SocketConnection, request_id: RequestId) -> Vec<SessionSummary> {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SESSION_LIST,
            Some(
                serde_json::to_value(ListSessionsQuery {})
                    .expect("session list params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("session list result should deserialize")
}

pub fn attach_session(
    stream: &mut SocketConnection,
    request_id: RequestId,
    session_id: SessionId,
    session_authority: SessionAuthority,
) -> DaemonSessionAttachResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SESSION_ATTACH,
            Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id,
                    session_authority,
                })
                .expect("session attach params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("session attach result should deserialize")
}

pub fn start_run(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    objective: &str,
) -> RunSummary {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_RUN_START,
            Some(
                serde_json::to_value(StartRunCommand {
                    objective: objective.to_string(),
                    ..StartRunCommand::default()
                })
                .expect("run start params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("run start result should deserialize")
}

pub fn replay_run_events(
    stream: &mut SocketConnection,
    request_id: RequestId,
    session_id: SessionId,
    run_id: RunId,
    after_seq: Option<u64>,
) -> SubscribeRunEventsResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_RUN_REPLAY_EVENTS,
            Some(
                serde_json::to_value(SubscribeRunEventsRequest {
                    session_id,
                    run_id,
                    after_seq,
                })
                .expect("run replay params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("run replay result should deserialize")
}

pub fn replay_run_events_expect_invalid_params(
    stream: &mut SocketConnection,
    request_id: RequestId,
    session_id: SessionId,
    run_id: RunId,
    after_seq: Option<u64>,
) -> JsonRpcError {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_RUN_REPLAY_EVENTS,
            Some(
                serde_json::to_value(SubscribeRunEventsRequest {
                    session_id,
                    run_id,
                    after_seq,
                })
                .expect("run replay params should serialize"),
            ),
        ),
    );
    let error = read_error(&codec, stream);
    assert_eq!(error.id, Some(request_id));
    error
}

pub fn decide_approval(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    approval_id: ApprovalId,
    decision: ApprovalDecision,
) -> DaemonApprovalDecideResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_APPROVAL_DECIDE,
            Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id,
                    decision,
                    commentary: None,
                })
                .expect("approval decide params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("approval decide result should deserialize")
}

pub fn subscribe_run_events_expect_invalid_params(
    stream: &mut SocketConnection,
    request_id: RequestId,
) -> JsonRpcError {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SUBSCRIBE,
            Some(json!({
                "kinds": [DaemonEventKind::Run]
            })),
        ),
    );
    let error = read_error(&codec, stream);
    assert_eq!(error.id, Some(request_id));
    error
}

pub fn get_session(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
) -> Option<SessionSummary> {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SESSION_GET,
            Some(
                serde_json::to_value(GetSessionQuery {})
                    .expect("session get params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("session get result should deserialize")
}

pub fn list_runs(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
) -> Vec<RunSummary> {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_RUN_LIST,
            Some(serde_json::to_value(ListRunsQuery {}).expect("run list params should serialize")),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("run list result should deserialize")
}

pub fn get_run(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    run_id: ta_protocol::wire::RunId,
) -> Option<RunDetail> {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_RUN_GET,
            Some(
                serde_json::to_value(GetRunQuery { run_id })
                    .expect("run get params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("run get result should deserialize")
}

pub fn list_approvals(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    run_id: Option<ta_protocol::wire::RunId>,
    approval_id: Option<ApprovalId>,
) -> ApprovalSnapshotResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_APPROVAL_LIST,
            Some(
                serde_json::to_value(ListApprovalsQuery {
                    run_id,
                    approval_id,
                })
                .expect("approval list params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("approval list result should deserialize")
}

pub fn activity_page(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    kinds: Vec<DaemonEventKind>,
) -> ActivityPageResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_ACTIVITY_PAGE,
            Some(
                serde_json::to_value(ActivityPageQuery {
                    limit: 25,
                    before: None,
                    kinds,
                })
                .expect("activity page params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("activity page result should deserialize")
}

pub fn public_activity_page(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    kinds: Vec<DaemonEventKind>,
) -> PublicActivityPageResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_ACTIVITY_PAGE,
            Some(
                serde_json::to_value(ActivityPageQuery {
                    limit: 25,
                    before: None,
                    kinds,
                })
                .expect("activity page params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("public activity page result should deserialize")
}

pub fn commit_artifact_in_existing_root_store(
    root_dir: &Path,
    socket_name: &str,
    artifact: ArtifactRecord,
) {
    let store_path = store_path_for_root(root_dir, socket_name);
    let mut store = SqliteStore::open(&store_path).expect("sqlite store should reopen");
    store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact,
            occurred_at_ms: 50,
        })
        .expect("artifact should persist in durable root");
}

pub fn commit_checkpoint_in_existing_root_store(
    root_dir: &Path,
    socket_name: &str,
    checkpoint: CheckpointRecord,
) {
    let store_path = store_path_for_root(root_dir, socket_name);
    let mut store = SqliteStore::open(&store_path).expect("sqlite store should reopen");
    store
        .commit_checkpoint_persist(CommitCheckpointPersist {
            checkpoint,
            occurred_at_ms: 50,
        })
        .expect("checkpoint should persist in durable root");
}

pub fn force_run_running_in_existing_root_store(
    root_dir: &Path,
    socket_name: &str,
    run_id: &RunId,
) {
    let store_path = store_path_for_root(root_dir, socket_name);
    let mut store = SqliteStore::open(&store_path).expect("sqlite store should reopen");
    let run = store
        .run(run_id)
        .expect("run lookup should work")
        .expect("run should exist in durable root");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: run.session_id.clone(),
            run: ta_store::RunProjection {
                status: RunStatus::Running,
                ..run.clone()
            },
            events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                run_id: run.id,
                status: RunStatus::Running,
                detail: "Seeded durable running run for integration proof".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 50,
        })
        .expect("run should persist in durable root");
}

pub fn list_artifacts(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
) -> ArtifactSnapshotResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_ARTIFACT_LIST,
            Some(
                serde_json::to_value(ListArtifactsQuery {
                    run_id: None,
                    artifact_id: None,
                })
                .expect("artifact list params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("artifact list result should deserialize")
}

pub fn get_artifact(
    stream: &mut SocketConnection,
    request_id: RequestId,
    _session_id: SessionId,
    artifact_id: ta_protocol::wire::ArtifactId,
) -> Option<ArtifactSummary> {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_ARTIFACT_GET,
            Some(
                serde_json::to_value(GetArtifactQuery { artifact_id })
                    .expect("artifact get params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("artifact get result should deserialize")
}
