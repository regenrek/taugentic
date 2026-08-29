use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ta_protocol::wire::{
    TerminalAttachParams, TerminalCloseParams, TerminalInputParams, TerminalListParams,
    TerminalResizeParams, TerminalSessionStatus, TerminalSpawnParams, WorkspacePath,
};

use super::*;

fn open_project_fixture(
    service: &AppService,
    root: &std::path::Path,
) -> (crate::ProjectId, crate::WorkspaceId) {
    let (project_id, snapshot) = service
        .open_project(
            TEST_OWNER_PRINCIPAL_ID,
            WorkspacePath::canonicalize_existing(root).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .and_then(|project| project.workspace_ids.first())
        .cloned()
        .expect("project workspace");
    (project_id, workspace_id)
}

#[test]
fn terminal_survives_detach_and_reattach_without_replacing_the_shell() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let spawned = service
        .spawn_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalSpawnParams {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                rows: 24,
                cols: 80,
                user_approved: true,
            },
        )
        .expect("terminal should spawn");
    let terminal_id = spawned.terminal.id.clone();
    let first = service
        .attach_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalAttachParams {
                terminal_id: terminal_id.clone(),
            },
            41,
        )
        .expect("terminal should attach");
    service
        .terminal_input(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalInputParams {
                terminal_id: terminal_id.clone(),
                data_base64: BASE64.encode(b"printf 'terminal-first\\n'\n"),
            },
        )
        .expect("terminal input should write");
    let first_event = first
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("first output should stream");
    assert!(matches!(
        first_event,
        crate::workspace::terminal::TerminalRuntimeEvent::Output { .. }
    ));
    drop(first);

    service
        .resize_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalResizeParams {
                terminal_id: terminal_id.clone(),
                rows: 42,
                cols: 132,
            },
        )
        .expect("detached terminal should resize");
    service
        .terminal_input(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalInputParams {
                terminal_id: terminal_id.clone(),
                data_base64: BASE64.encode(b"printf 'terminal-second\\n'\n"),
            },
        )
        .expect("detached terminal should keep accepting input");
    std::thread::sleep(Duration::from_millis(100));

    let second = service
        .attach_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalAttachParams {
                terminal_id: terminal_id.clone(),
            },
            42,
        )
        .expect("terminal should reattach");
    let snapshot = BASE64
        .decode(second.result.snapshot_base64)
        .expect("snapshot should decode");
    let snapshot = String::from_utf8_lossy(&snapshot);
    assert!(snapshot.contains("terminal-first"));
    assert!(snapshot.contains("terminal-second"));
    assert_eq!(second.result.terminal.rows, 42);
    assert_eq!(second.result.terminal.cols, 132);

    let listed = service
        .list_terminals(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalListParams {
                project_id,
                workspace_id,
            },
        )
        .expect("terminals should list");
    assert_eq!(listed.terminals.len(), 1);
    assert_eq!(listed.terminals[0].id, terminal_id);

    let closed = service
        .close_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalCloseParams {
                terminal_id: terminal_id.clone(),
            },
        )
        .expect("terminal should close");
    assert_eq!(closed.terminal.status, TerminalSessionStatus::Exited);
}

#[test]
fn terminal_rejects_unapproved_spawn_invalid_input_and_cross_principal_access() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let root = tempfile::tempdir().expect("workspace tempdir");
    let (project_id, workspace_id) = open_project_fixture(&service, root.path());
    let params = TerminalSpawnParams {
        project_id,
        workspace_id,
        rows: 24,
        cols: 80,
        user_approved: false,
    };
    assert!(matches!(
        service.spawn_terminal(TEST_OWNER_PRINCIPAL_ID, &params),
        Err(AppServiceError::TerminalApprovalRequired)
    ));

    let spawned = service
        .spawn_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalSpawnParams {
                user_approved: true,
                ..params
            },
        )
        .expect("approved terminal should spawn");
    assert!(matches!(
        service.terminal_input(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalInputParams {
                terminal_id: spawned.terminal.id.clone(),
                data_base64: "not-base64".to_string(),
            }
        ),
        Err(AppServiceError::TerminalInvalidInput)
    ));
    assert!(matches!(
        service.attach_terminal(
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &TerminalAttachParams {
                terminal_id: spawned.terminal.id.clone(),
            },
            43,
        ),
        Err(AppServiceError::TerminalNotFound(_))
    ));
    service
        .close_terminal(
            TEST_OWNER_PRINCIPAL_ID,
            &TerminalCloseParams {
                terminal_id: spawned.terminal.id,
            },
        )
        .expect("terminal should close");
}
