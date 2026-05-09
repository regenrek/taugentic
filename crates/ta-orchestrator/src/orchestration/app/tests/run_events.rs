use super::*;

#[test]
fn subscribe_run_events_replays_native_events_after_sequence() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Native replay".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run_id = RunId::new("run-replay").expect("run id");
    let other_run_id = RunId::new("run-other").expect("run id");
    seed_run_projection(
        &service,
        RunProjection {
            id: run_id.clone(),
            last_event_seq: Some(102),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
            ..native_run_projection("run-replay", &session.id, RunStatus::Running, 100)
        },
    );
    {
        let mut store = service
            .store
            .lock()
            .expect("app store should not be poisoned");
        for (sequence, event_run_id) in [
            (100, run_id.clone()),
            (101, other_run_id),
            (102, run_id.clone()),
        ] {
            store
                .append_event(EventRecord {
                    sequence,
                    session_id: session.id.clone(),
                    occurred_at_ms: sequence * 10,
                    payload: DaemonEvent::Run(crate::RunEvent {
                        run_id: event_run_id,
                        status: RunStatus::Running,
                        detail: format!("event {sequence}"),
                        output_contract: None,
                        recipe_id: None,
                        result: None,
                    }),
                })
                .expect("event should seed");
        }
    }

    let replay = service
        .replay_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run_id.clone(),
                after_seq: Some(1),
            },
        )
        .expect("replay");
    let empty = service
        .replay_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id,
                after_seq: Some(102),
            },
        )
        .expect("empty replay");

    assert_eq!(
        replay
            .events
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        vec![100, 102]
    );
    assert_eq!(replay.latest_event_seq, Some(102));
    assert!(empty.events.is_empty());
    assert_eq!(empty.latest_event_seq, Some(102));
}

#[test]
fn subscribe_run_events_redacts_resolved_approval_for_replay_and_live() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Approval redaction".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");

    let replay_run = ensure_running_run(&service, &session.id, "Replay approval redaction");
    let replay_run_id = replay_run.body.id.clone();
    let replay_after_seq = latest_run_event_seq(&service, &replay_run_id);
    let replay_record =
        approval_resolved_record(&session.id, &replay_run_id, replay_after_seq + 1, "replay");
    append_approval_record(&service, &replay_run_id, replay_record);

    let replay = service
        .replay_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: replay_run_id.clone(),
                after_seq: Some(replay_after_seq),
            },
        )
        .expect("approval replay should succeed");
    let replay_json = serde_json::to_value(&replay).expect("replay should serialize");

    assert_eq!(replay.events.len(), 1);
    assert!(matches!(
        &replay.events[0].event,
        PublicDaemonEvent::Approval(PublicApprovalEvent::Resolved { resolution })
            if resolution.run_id == replay_run_id
    ));
    assert_public_approval_json(
        &replay_json,
        "approval-replay",
        &replay_run_id,
        "tool-call-replay",
    );

    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: replay_run_id.clone(),
                after_seq: Some(replay_after_seq + 1),
            },
        )
        .expect("live run should subscribe");
    let live_seq = latest_run_event_seq(&service, &replay_run_id) + 1;
    let live_record = approval_resolved_record(&session.id, &replay_run_id, live_seq, "live");
    append_approval_record(&service, &replay_run_id, live_record.clone());
    service.runtime.publish_record(&live_record);

    let live_delta = subscription
        .receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("live approval event should arrive")
        .expect("live approval item should be ok");
    let live_json = serde_json::to_value(RunEventStreamItem {
        run_id: replay_run_id.clone(),
        payload: RunEventStreamPayload::Delta { delta: live_delta },
    })
    .expect("live notification should serialize");

    assert_public_approval_json(
        &live_json,
        "approval-live",
        &replay_run_id,
        "tool-call-live",
    );
}

#[test]
fn subscribe_run_events_rejects_external_harness() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "External replay".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    seed_run_projection(
        &service,
        RunProjection {
            harness: RunHarnessKind::Acp,
            ..native_run_projection("run-external", &session.id, RunStatus::Running, 100)
        },
    );

    let result = service.replay_run_events(
        &session.id,
        &SubscribeRunEventsRequest {
            session_id: session.id.clone(),
            run_id: RunId::new("run-external").expect("run id"),
            after_seq: None,
        },
    );

    assert!(matches!(
        result,
        Err(AppServiceError::RunNotNativeHarness(run_id)) if run_id == "run-external"
    ));
}

#[test]
fn subscribe_run_events_replay_only_terminal_run_closes_live_receiver() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Terminal run replay".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run_id = RunId::new("run-terminal-subscribe").expect("run id");
    seed_run_projection(
        &service,
        RunProjection {
            id: run_id.clone(),
            last_event_seq: Some(22),
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
            ..native_run_projection(
                "run-terminal-subscribe",
                &session.id,
                RunStatus::Completed,
                200,
            )
        },
    );
    append_and_publish_run_event(&service, &session.id, &run_id, 21, "historical one");
    append_and_publish_run_event(&service, &session.id, &run_id, 22, "historical two");

    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id,
                after_seq: Some(21),
            },
        )
        .expect("terminal run should replay");

    assert!(!subscription.live);
    assert_eq!(
        subscription
            .replay
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        vec![22]
    );
    assert!(matches!(
        subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(50)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn subscribe_run_events_splices_replay_then_live_without_gap() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Live splice".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&service, &session.id, "Resume live stream");

    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run.body.id.clone(),
                after_seq: None,
            },
        )
        .expect("live run should subscribe");
    let next_sequence = subscription.latest_event_seq.expect("seed event sequence") + 1;

    append_and_publish_run_event(
        &service,
        &session.id,
        &run.body.id,
        next_sequence,
        "live after subscribe",
    );

    assert!(subscription.live);
    assert!(!subscription.replay.is_empty());
    assert_eq!(
        subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("live event should arrive")
            .expect("live event item should be ok")
            .seq,
        next_sequence
    );
}

#[test]
fn subscribe_run_events_paginates_replay_before_live_forward() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Live splice paginated replay".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&service, &session.id, "Replay more than one batch");
    let run_id = run.body.id.clone();
    let (after_seq, final_replay_seq) = {
        let mut store = service
            .store
            .lock()
            .expect("app store should not be poisoned");
        let mut projection = store
            .run(&run_id)
            .expect("run lookup should succeed")
            .expect("run should exist");
        let after_seq = projection.last_event_seq.expect("seed event sequence");
        for offset in 1..=(RUN_EVENT_REPLAY_BATCH_LIMIT as u64 + 1) {
            let sequence = after_seq + offset;
            store
                .append_event(EventRecord {
                    sequence,
                    session_id: session.id.clone(),
                    occurred_at_ms: sequence * 10,
                    payload: DaemonEvent::Run(crate::RunEvent {
                        run_id: run_id.clone(),
                        status: RunStatus::Running,
                        detail: format!("paged replay {offset}"),
                        output_contract: None,
                        recipe_id: None,
                        result: None,
                    }),
                })
                .expect("run event should append");
        }
        let final_replay_seq = after_seq + RUN_EVENT_REPLAY_BATCH_LIMIT as u64 + 1;
        projection.last_event_seq = Some(final_replay_seq);
        store
            .save_run(projection)
            .expect("run projection should update");
        (after_seq, final_replay_seq)
    };

    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run_id.clone(),
                after_seq: Some(after_seq),
            },
        )
        .expect("live run should subscribe");
    let live_seq = final_replay_seq + 1;
    append_and_publish_run_event(&service, &session.id, &run_id, live_seq, "live after pages");

    assert_eq!(subscription.replay.len(), RUN_EVENT_REPLAY_BATCH_LIMIT + 1);
    assert_eq!(
        subscription.replay.first().map(|event| event.seq),
        Some(after_seq + 1)
    );
    assert_eq!(
        subscription.replay.last().map(|event| event.seq),
        Some(final_replay_seq)
    );
    assert_eq!(subscription.latest_event_seq, Some(final_replay_seq));
    assert_eq!(
        subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("live event should arrive")
            .expect("live event item should be ok")
            .seq,
        live_seq
    );
}

#[test]
fn subscribe_run_events_dedupes_replay_boundary() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Live splice dedupe".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&service, &session.id, "Dedupe live stream");
    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run.body.id.clone(),
                after_seq: None,
            },
        )
        .expect("live run should subscribe");
    let boundary = subscription.latest_event_seq.expect("seed event sequence");
    let duplicate = EventRecord {
        sequence: boundary,
        session_id: session.id.clone(),
        occurred_at_ms: boundary * 10,
        payload: DaemonEvent::Run(crate::RunEvent {
            run_id: run.body.id.clone(),
            status: RunStatus::Running,
            detail: "duplicate boundary".to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        }),
    };
    service.runtime.publish_record(&duplicate);
    append_and_publish_run_event(
        &service,
        &session.id,
        &run.body.id,
        boundary + 1,
        "post-boundary live",
    );

    let received = subscription
        .receiver
        .recv_timeout(std::time::Duration::from_millis(200))
        .expect("post-boundary live event should arrive")
        .expect("post-boundary live item should be ok");

    assert_eq!(received.seq, boundary + 1);
}

#[test]
fn subscribe_run_events_emits_lagged_before_overflow_close() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Lagged live splice".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&service, &session.id, "Lagged live stream");
    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run.body.id.clone(),
                after_seq: None,
            },
        )
        .expect("live run should subscribe");
    let start_sequence = subscription.latest_event_seq.expect("seed event sequence") + 1;

    let publish_event = |sequence: u64| {
        service.runtime.publish_record(&EventRecord {
            sequence,
            session_id: session.id.clone(),
            occurred_at_ms: sequence * 10,
            payload: DaemonEvent::Run(crate::RunEvent {
                run_id: run.body.id.clone(),
                status: RunStatus::Running,
                detail: "overflow live run subscriber".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
        });
    };

    let mut ok_count_before_overflow = 0;
    publish_event(start_sequence);
    let first_item = subscription
        .receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("first live item should arrive before overflow")
        .expect("first live item should be ok");
    assert_eq!(first_item.seq, start_sequence);
    ok_count_before_overflow += 1;

    let mut next_sequence = start_sequence + 1;
    for _ in 0..1024 {
        publish_event(next_sequence);
        next_sequence += 1;
    }
    let overflow_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !subscription
        .overflowed
        .load(std::sync::atomic::Ordering::SeqCst)
        && std::time::Instant::now() < overflow_deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        subscription
            .overflowed
            .load(std::sync::atomic::Ordering::SeqCst),
        "live run subscriber should overflow before draining the receiver"
    );

    let mut terminal = None;
    let drain_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while terminal.is_none() && std::time::Instant::now() < drain_deadline {
        match subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("stream should close with lagged")
        {
            Ok(_) => {}
            Err(error) => {
                terminal = Some(error);
                break;
            }
        }
    }

    assert!(
        ok_count_before_overflow >= 1,
        "expected at least one Ok(_) before Lagged"
    );
    assert_eq!(terminal, Some(RunEventStreamError::Lagged));
    assert!(matches!(
        subscription
            .receiver
            .recv_timeout(std::time::Duration::from_millis(50)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn subscribe_run_events_rejects_external_harness_for_live_splice() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "External live splice".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    seed_run_projection(
        &service,
        RunProjection {
            harness: RunHarnessKind::Acp,
            ..native_run_projection("run-external-live", &session.id, RunStatus::Running, 100)
        },
    );

    let result = service.subscribe_run_events(
        &session.id,
        &SubscribeRunEventsRequest {
            session_id: session.id.clone(),
            run_id: RunId::new("run-external-live").expect("run id"),
            after_seq: None,
        },
    );

    assert!(matches!(
        result,
        Err(AppServiceError::RunNotNativeHarness(run_id)) if run_id == "run-external-live"
    ));
}

#[test]
fn subscribe_run_events_drop_releases_live_channel_immediately() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Drop live splice".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&service, &session.id, "Drop subscriber");
    let subscription = service
        .subscribe_run_events(
            &session.id,
            &SubscribeRunEventsRequest {
                session_id: session.id.clone(),
                run_id: run.body.id.clone(),
                after_seq: None,
            },
        )
        .expect("live run should subscribe");

    assert_eq!(service.runtime.subscriber_count_for_session(&session.id), 1);
    drop(subscription);

    assert_eq!(service.runtime.subscriber_count_for_session(&session.id), 0);
}

const SECRET_APPROVAL_ACTOR: &str = "human@example.com";
const SECRET_APPROVAL_COMMENTARY: &str = "secret note";

fn latest_run_event_seq(service: &AppService, run_id: &RunId) -> u64 {
    service
        .store
        .lock()
        .expect("app store should not be poisoned")
        .run(run_id)
        .expect("run lookup should succeed")
        .expect("run should exist")
        .last_event_seq
        .expect("seed event sequence")
}

fn approval_resolved_record(
    session_id: &SessionId,
    run_id: &RunId,
    sequence: u64,
    approval_suffix: &str,
) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Approval(crate::ApprovalEvent::Resolved {
            resolution: ApprovalResolution::new(
                ApprovalId::new(format!("approval-{approval_suffix}")).expect("approval id"),
                run_id.clone(),
                ApprovalDecision::Approved,
                crate::ApprovalResolutionReason::User,
                ApprovalActor::new(SECRET_APPROVAL_ACTOR).expect("approval actor"),
                Some(SECRET_APPROVAL_COMMENTARY.to_string()),
            )
            .with_tool_call_id(
                crate::AgentStreamItemId::new(format!("tool-call-{approval_suffix}"))
                    .expect("tool call id"),
            ),
        }),
    }
}

fn append_approval_record(service: &AppService, run_id: &RunId, record: EventRecord) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    store
        .append_event(record.clone())
        .expect("approval event should append");
    let mut run = store
        .run(run_id)
        .expect("run lookup should succeed")
        .expect("run should exist");
    run.last_event_seq = Some(record.sequence);
    store.save_run(run).expect("run projection should update");
}

fn assert_public_approval_json(
    value: &serde_json::Value,
    expected_approval_id: &str,
    expected_run_id: &RunId,
    expected_tool_call_id: &str,
) {
    let serialized = serde_json::to_string(value).expect("json should serialize");
    assert!(!serialized.contains(SECRET_APPROVAL_ACTOR), "{serialized}");
    assert!(
        !serialized.contains(SECRET_APPROVAL_COMMENTARY),
        "{serialized}"
    );
    assert!(!serialized.contains("\"actor\""), "{serialized}");
    assert!(!serialized.contains("\"commentary\""), "{serialized}");

    let resolution =
        public_approval_resolution_json(value).expect("approval resolution should be present");
    assert_eq!(
        resolution
            .get("approvalId")
            .and_then(serde_json::Value::as_str),
        Some(expected_approval_id),
        "{serialized}"
    );
    assert_eq!(
        resolution.get("runId").and_then(serde_json::Value::as_str),
        Some(expected_run_id.as_str()),
        "{serialized}"
    );
    assert_eq!(
        resolution
            .get("toolCallId")
            .and_then(serde_json::Value::as_str),
        Some(expected_tool_call_id),
        "{serialized}"
    );
    assert_eq!(
        resolution
            .get("decision")
            .and_then(serde_json::Value::as_str),
        Some("approved"),
        "{serialized}"
    );
}

fn public_approval_resolution_json(
    value: &serde_json::Value,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match value {
        serde_json::Value::Object(object) => object
            .get("resolution")
            .and_then(serde_json::Value::as_object)
            .or_else(|| object.values().find_map(public_approval_resolution_json)),
        serde_json::Value::Array(items) => items.iter().find_map(public_approval_resolution_json),
        _ => None,
    }
}
