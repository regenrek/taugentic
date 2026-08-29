use crate::support::*;
use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{
    DaemonProjectOpenParams, DaemonSessionOpenParams, ThreadWorkspaceMutation,
    ThreadWorkspaceUpdateCommand, WorkspacePath, WorkspaceSelector,
};

#[test]
fn persistent_client_keeps_thread_workspace_attached_to_the_live_daemon_session() {
    let root = test_temp_dir("ta-daemon-it-thread-workspace");
    let project_path = root.join("project");
    fs::create_dir_all(&project_path).expect("isolated project should exist");

    let socket_name = unique_name("ta-daemon-it-thread-workspace");
    let mut daemon = ManagedDaemon::spawn_with_root(&socket_name, root, &[], true);
    daemon
        .wait_for_status()
        .expect("live daemon should become ready before the persistent lifecycle");

    let daemon_client = DaemonClient::from_config(daemon.client().config().clone());
    let client = daemon_client
        .connect_persistent("ta-thread-workspace-integration", env!("CARGO_PKG_VERSION"))
        .expect("persistent client should initialize against the live daemon");
    let project = client
        .clone()
        .open_project(DaemonProjectOpenParams {
            path: WorkspacePath::canonicalize_existing(&project_path)
                .expect("isolated project path should canonicalize"),
            trust_acknowledged: true,
        })
        .expect("project should open through the live daemon");
    let workspace_id = project
        .snapshot
        .projects
        .iter()
        .find(|candidate| candidate.id == project.project_id)
        .and_then(|candidate| candidate.workspace_ids.first())
        .cloned()
        .expect("opened project should expose one workspace");
    let opened = client
        .clone()
        .open_session(DaemonSessionOpenParams {
            title: "Thread workspace integration".to_string(),
            workspace: WorkspaceSelector::ByProject {
                project_id: project.project_id,
                workspace_id,
            },
        })
        .expect("session should open through the live daemon");
    client
        .clone()
        .attach_session(opened.session.id.clone())
        .expect("persistent client should attach the opened session");

    let empty = client
        .clone()
        .thread_workspace()
        .expect("attached session should expose an empty thread workspace");
    assert_eq!(empty.session_id, opened.session.id);
    assert_eq!(empty.goal, "");
    assert_eq!(empty.plan, "");
    assert_eq!(empty.notes, "");
    assert_eq!(empty.recap, "");
    assert!(empty.pins.is_empty());
    assert!(empty.work_log.is_empty());

    let updated = client
        .clone()
        .update_thread_workspace(ThreadWorkspaceUpdateCommand {
            mutation: ThreadWorkspaceMutation::GoalSet {
                value: "Complete the workspace".to_string(),
            },
        })
        .expect("attached session should accept a thread workspace mutation");
    assert_eq!(updated.session_id, opened.session.id);
    assert_eq!(updated.goal, "Complete the workspace");
    assert_eq!(updated.work_log.len(), 1);
    assert_eq!(updated.work_log[0].sequence, 1);

    let reread = client
        .clone()
        .thread_workspace()
        .expect("thread workspace should read back through the same socket lifecycle");
    assert_eq!(reread, updated);
    client.close();
}
