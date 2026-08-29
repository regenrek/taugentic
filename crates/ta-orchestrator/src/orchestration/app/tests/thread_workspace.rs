use super::*;
use ta_protocol::wire::{
    ThreadWorkspaceMutation, ThreadWorkspaceQuery, ThreadWorkspaceUpdateCommand,
};

#[test]
fn thread_workspace_fresh_projection_and_updates_are_store_owned() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Thread workspace");
    let fresh = service
        .thread_workspace(&session.id, &ThreadWorkspaceQuery {})
        .expect("fresh projection");
    assert_eq!(fresh.session_id, session.id);
    assert!(
        fresh.goal.is_empty()
            && fresh.plan.is_empty()
            && fresh.notes.is_empty()
            && fresh.recap.is_empty()
    );

    for mutation in [
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
    ] {
        service
            .update_thread_workspace(&session.id, &ThreadWorkspaceUpdateCommand { mutation })
            .expect("update persists");
    }
    let projection = service
        .thread_workspace(&session.id, &ThreadWorkspaceQuery {})
        .expect("stored projection");
    assert_eq!(
        (
            projection.goal.as_str(),
            projection.plan.as_str(),
            projection.notes.as_str(),
            projection.recap.as_str()
        ),
        ("goal", "plan", "notes", "recap")
    );
    assert_eq!(projection.work_log.len(), 4);
}
