use crate::support::*;
use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{
    DaemonProjectOpenParams, DaemonRunCancelParams, DaemonSessionOpenParams, GetRunTimelineQuery,
    METHOD_DAEMON_RUN_CANCEL, WorkspacePath, WorkspaceSelector,
};

#[test]
fn persistent_client_projects_run_activity_from_a_launched_daemon() {
    let root = test_temp_dir("ta-daemon-it-run-activity");
    let project_path = root.join("project");
    fs::create_dir_all(&project_path).expect("isolated project should exist");

    let socket_name = unique_name("ta-daemon-it-run-activity");
    let mut daemon = ManagedDaemon::spawn_with_root(&socket_name, root, &[], true);
    let status = daemon
        .wait_for_status()
        .expect("launched daemon should become ready before persistent client lifecycle");
    assert!(status.ready);

    let daemon_client = DaemonClient::from_config(daemon.client().config().clone());
    let mut client = daemon_client
        .connect_persistent("ta-run-activity-integration", env!("CARGO_PKG_VERSION"))
        .expect("persistent client should initialize against the launched daemon");
    let project = client
        .open_project(DaemonProjectOpenParams {
            path: WorkspacePath::canonicalize_existing(&project_path)
                .expect("isolated project path should canonicalize"),
            trust_acknowledged: true,
        })
        .expect("project should open through the persistent daemon connection");
    let workspace_id = project
        .snapshot
        .projects
        .iter()
        .find(|candidate| candidate.id == project.project_id)
        .and_then(|candidate| candidate.workspace_ids.first())
        .cloned()
        .expect("opened project should expose one workspace");
    let opened = client
        .open_session(DaemonSessionOpenParams {
            title: "Persistent run activity integration".to_string(),
            workspace: WorkspaceSelector::ByProject {
                project_id: project.project_id,
                workspace_id,
            },
        })
        .expect("session should open through the persistent connection");
    client
        .attach_session(opened.session.id.clone())
        .expect("persistent client should attach its daemon-owned session");

    let sessions = client
        .list_sessions()
        .expect("attached persistent client should list daemon sessions");
    assert_eq!(sessions, vec![opened.session.clone()]);

    let run = client
        .start_run(StartRunCommand {
            objective: "Wait for local approval without invoking a provider".to_string(),
            selection: AgentRuntimeSelection {
                runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                    .expect("runtime profile id"),
                auth_profile_id: Some(
                    AuthProfileId::new("profile-openai-test").expect("auth profile id"),
                ),
                model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
            },
            attachments: Vec::new(),
            recipe_id: None,
        })
        .expect("attached persistent client should start the local approval-waiting run");
    assert_eq!(run.status, RunStatus::WaitingForApproval);

    let runs = client
        .list_runs(ListRunsQuery {})
        .expect("attached persistent client should list only session-scoped runs");
    assert_eq!(runs, vec![run.clone()]);
    let detail = client
        .get_run(GetRunQuery {
            run_id: run.id.clone(),
        })
        .expect("attached persistent client should get its session-scoped run")
        .expect("started run should remain available");
    assert_eq!(detail.summary, run);

    let timeline = client
        .run_timeline(GetRunTimelineQuery {
            session_id: opened.session.id.clone(),
            root_run_id: run.id.clone(),
            after_seq: None,
            limit: None,
        })
        .expect("persistent client should project the daemon-owned run timeline");
    assert_eq!(timeline.session_id, opened.session.id);
    assert_eq!(timeline.root_run_id, run.id);
    assert!(
        timeline
            .events
            .windows(2)
            .all(|pair| pair[0].seq < pair[1].seq)
    );

    let activity = client
        .activity_page(ActivityPageQuery {
            limit: 25,
            before: None,
            kinds: vec![DaemonEventKind::Run, DaemonEventKind::Approval],
        })
        .expect("persistent client should page daemon-owned activity");
    assert!(!activity.items.is_empty());
    assert!(
        activity
            .items
            .windows(2)
            .all(|pair| pair[0].cursor.sequence > pair[1].cursor.sequence)
    );
    assert_eq!(
        activity
            .latest_activity_cursor
            .as_ref()
            .map(|cursor| cursor.sequence),
        activity.items.first().map(|item| item.cursor.sequence)
    );

    let approvals = client
        .list_approvals(ListApprovalsQuery {
            run_id: Some(run.id.clone()),
            approval_id: None,
        })
        .expect("persistent client should expose the run's pending approval");
    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].run_id, run.id);

    let replay = client
        .replay_run_events(SubscribeRunEventsRequest {
            session_id: opened.session.id.clone(),
            run_id: run.id.clone(),
            after_seq: None,
        })
        .expect("persistent client should replay durable run history");
    assert!(!replay.events.is_empty());
    assert!(
        replay
            .events
            .windows(2)
            .all(|pair| pair[0].seq < pair[1].seq)
    );
    assert_eq!(
        replay.latest_event_seq,
        replay.events.last().map(|event| event.seq)
    );

    let cancelled: RunSummary = client
        .call_public(
            METHOD_DAEMON_RUN_CANCEL,
            &DaemonRunCancelParams {
                run_id: run.id.clone(),
                reason: None,
            },
        )
        .expect("persistent client should cancel its attached approval-waiting run");
    assert_eq!(cancelled.id, run.id);
    assert_eq!(cancelled.status, RunStatus::Cancelled);
    client.close();
}
