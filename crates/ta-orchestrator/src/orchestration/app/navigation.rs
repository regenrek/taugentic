use ta_protocol::wire::{
    ConversationPlacement, DaemonNavigationIntent, NavigationAgentRow, NavigationConversation,
    NavigationSnapshot, ProjectId, SessionStatus, SpaceId,
};
use ta_store::{
    NavigationConversationMetadata, NavigationState, PersistenceStore, SessionApprovalQuery,
};
use uuid::Uuid;

use super::{AppService, AppServiceError, sanitize_session_owner_principal_id};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn navigation_snapshot(
        &self,
        owner_principal_id: &str,
        search: Option<&str>,
    ) -> Result<NavigationSnapshot, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        let state = store.navigation_state(&owner_principal_id)?;
        let sessions = store
            .sessions()?
            .into_iter()
            .filter(|session| session.owner_principal_id == owner_principal_id)
            .collect::<Vec<_>>();
        let search = search
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let session_ids = sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut conversation_metadata = state
            .conversations
            .into_iter()
            .filter(|item| session_ids.contains(&item.session_id))
            .collect::<Vec<_>>();
        for session in &sessions {
            if !conversation_metadata
                .iter()
                .any(|item| item.session_id == session.id)
            {
                conversation_metadata.push(NavigationConversationMetadata {
                    session_id: session.id.clone(),
                    placement: ConversationPlacement::Standalone,
                    archived: false,
                    pinned: false,
                });
            }
        }
        let conversations = conversation_metadata
            .into_iter()
            .filter_map(|item| {
                let session = sessions
                    .iter()
                    .find(|session| session.id == item.session_id)?;
                search
                    .as_ref()
                    .is_none_or(|needle| session.title.to_lowercase().contains(needle))
                    .then_some(NavigationConversation {
                        session_id: item.session_id,
                        title: session.title.clone(),
                        status: session.status,
                        placement: item.placement,
                        archived: item.archived,
                        pinned: item.pinned,
                    })
            })
            .collect::<Vec<_>>();
        let agents = sessions
            .into_iter()
            .filter(|session| {
                search
                    .as_ref()
                    .is_none_or(|needle| session.title.to_lowercase().contains(needle))
            })
            .map(|session| {
                let awaiting_approval = store
                    .approvals_for_session(&SessionApprovalQuery {
                        session_id: session.id.clone(),
                        run_id: None,
                        approval_id: None,
                    })
                    .map(|items| !items.is_empty())?;
                Ok(NavigationAgentRow {
                    session_id: session.id,
                    title: session.title,
                    active: matches!(session.status, SessionStatus::Running),
                    awaiting_approval,
                })
            })
            .collect::<Result<Vec<_>, ta_store::StoreError>>()?;
        let visible_project_ids = conversations
            .iter()
            .filter_map(|item| match &item.placement {
                ConversationPlacement::Project { project_id } => Some(project_id),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        let projects = state
            .projects
            .into_iter()
            .filter(|project| {
                search.is_none()
                    || visible_project_ids.contains(&project.id)
                    || project
                        .title
                        .to_lowercase()
                        .contains(search.as_ref().expect("search exists"))
            })
            .collect::<Vec<_>>();
        let space_ids = projects
            .iter()
            .filter_map(|project| project.space_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let spaces = state
            .spaces
            .into_iter()
            .filter(|space| {
                search.is_none()
                    || space_ids.contains(&space.id)
                    || space
                        .title
                        .to_lowercase()
                        .contains(search.as_ref().expect("search exists"))
            })
            .collect();
        Ok(NavigationSnapshot {
            spaces,
            projects,
            conversations,
            agents,
        })
    }

    pub fn apply_navigation_intent(
        &self,
        owner_principal_id: &str,
        intent: DaemonNavigationIntent,
    ) -> Result<NavigationSnapshot, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let mut state = store.navigation_state(&owner_principal_id)?;
        match intent {
            DaemonNavigationIntent::CreateSpace { title } => {
                state.spaces.push(ta_protocol::wire::NavigationSpace {
                    id: SpaceId::new(format!("space-{}", Uuid::new_v4().simple()))
                        .expect("generated id"),
                    title: navigation_title(title)?,
                });
            }
            DaemonNavigationIntent::CreateProject {
                space_id,
                title,
                workspace_ids,
            } => {
                if let Some(space_id) = &space_id {
                    if !state.spaces.iter().any(|space| space.id == *space_id) {
                        return Err(AppServiceError::SessionNotFound(
                            space_id.as_str().to_string(),
                        ));
                    }
                }
                validate_workspaces(&*store, &workspace_ids)?;
                state.projects.push(ta_protocol::wire::NavigationProject {
                    id: ProjectId::new(format!("project-{}", Uuid::new_v4().simple()))
                        .expect("generated id"),
                    space_id,
                    title: navigation_title(title)?,
                    workspace_ids,
                });
            }
            DaemonNavigationIntent::SetProjectWorkspaces {
                project_id,
                workspace_ids,
            } => {
                validate_workspaces(&*store, &workspace_ids)?;
                let project = state
                    .projects
                    .iter_mut()
                    .find(|project| project.id == project_id)
                    .ok_or_else(|| {
                        AppServiceError::SessionNotFound(project_id.as_str().to_string())
                    })?;
                project.workspace_ids = workspace_ids;
            }
            DaemonNavigationIntent::PlaceConversation {
                session_id,
                placement,
            } => {
                let session = owned_session(&*store, &owner_principal_id, &session_id)?;
                validate_conversation_placement(&state, &placement, &session.workspace_id)?;
                upsert_conversation(&mut state, session_id, placement);
            }
            DaemonNavigationIntent::SetPinned { session_id, pinned } => {
                validate_owned_session(&*store, &owner_principal_id, &session_id)?;
                ensure_conversation(&mut state, session_id.clone());
                let item = navigation_conversation_mut(&mut state, &session_id)?;
                item.pinned = pinned;
            }
            DaemonNavigationIntent::SetArchived {
                session_id,
                archived,
            } => {
                validate_owned_session(&*store, &owner_principal_id, &session_id)?;
                ensure_conversation(&mut state, session_id.clone());
                let item = navigation_conversation_mut(&mut state, &session_id)?;
                item.archived = archived;
            }
            DaemonNavigationIntent::CloseTemporaryConversation { session_id } => {
                let item = state
                    .conversations
                    .iter()
                    .find(|item| item.session_id == session_id)
                    .ok_or_else(|| {
                        AppServiceError::SessionNotFound(session_id.as_str().to_string())
                    })?;
                if !matches!(item.placement, ConversationPlacement::Temporary) {
                    return Err(AppServiceError::SessionNotFound(
                        session_id.as_str().to_string(),
                    ));
                }
                if !store.delete_temporary_session(&owner_principal_id, &session_id)? {
                    return Err(AppServiceError::SessionNotFound(
                        session_id.as_str().to_string(),
                    ));
                }
                drop(store);
                return self.navigation_snapshot(&owner_principal_id, None);
            }
        }
        store.save_navigation_state(&owner_principal_id, state)?;
        drop(store);
        self.navigation_snapshot(&owner_principal_id, None)
    }
}

fn navigation_title(value: String) -> Result<String, AppServiceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppServiceError::EmptySessionTitle);
    }
    Ok(value.to_string())
}

fn validate_workspaces<S: PersistenceStore>(
    store: &S,
    workspace_ids: &[ta_protocol::wire::WorkspaceId],
) -> Result<(), AppServiceError> {
    for workspace_id in workspace_ids {
        if store.workspace(workspace_id)?.is_none() {
            return Err(AppServiceError::WorkspaceNotFound(
                workspace_id.as_str().to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_owned_session<S: PersistenceStore>(
    store: &S,
    owner_principal_id: &str,
    session_id: &ta_protocol::wire::SessionId,
) -> Result<(), AppServiceError> {
    if store
        .session(session_id)?
        .is_some_and(|session| session.owner_principal_id == owner_principal_id)
    {
        Ok(())
    } else {
        Err(AppServiceError::SessionNotFound(
            session_id.as_str().to_string(),
        ))
    }
}

fn owned_session<S: PersistenceStore>(
    store: &S,
    owner_principal_id: &str,
    session_id: &ta_protocol::wire::SessionId,
) -> Result<ta_store::SessionProjection, AppServiceError> {
    store
        .session(session_id)?
        .filter(|session| session.owner_principal_id == owner_principal_id)
        .ok_or_else(|| AppServiceError::SessionNotFound(session_id.as_str().to_string()))
}

pub(super) fn validate_conversation_placement(
    state: &NavigationState,
    placement: &ConversationPlacement,
    workspace_id: &ta_protocol::wire::WorkspaceId,
) -> Result<(), AppServiceError> {
    let ConversationPlacement::Project { project_id } = placement else {
        return Ok(());
    };
    let project = state
        .projects
        .iter()
        .find(|project| project.id == *project_id)
        .ok_or_else(|| AppServiceError::SessionNotFound(project_id.as_str().to_string()))?;
    if !project.workspace_ids.contains(workspace_id) {
        return Err(AppServiceError::WorkspaceNotFound(
            workspace_id.as_str().to_string(),
        ));
    }
    Ok(())
}

fn navigation_conversation_mut<'a>(
    state: &'a mut NavigationState,
    session_id: &ta_protocol::wire::SessionId,
) -> Result<&'a mut NavigationConversationMetadata, AppServiceError> {
    state
        .conversations
        .iter_mut()
        .find(|item| item.session_id == *session_id)
        .ok_or_else(|| AppServiceError::SessionNotFound(session_id.as_str().to_string()))
}

pub(super) fn upsert_conversation(
    state: &mut NavigationState,
    session_id: ta_protocol::wire::SessionId,
    placement: ConversationPlacement,
) {
    if let Some(item) = state
        .conversations
        .iter_mut()
        .find(|item| item.session_id == session_id)
    {
        item.placement = placement;
        return;
    }
    state.conversations.push(NavigationConversationMetadata {
        session_id,
        placement,
        archived: false,
        pinned: false,
    });
}

fn ensure_conversation(state: &mut NavigationState, session_id: ta_protocol::wire::SessionId) {
    if !state
        .conversations
        .iter()
        .any(|item| item.session_id == session_id)
    {
        upsert_conversation(state, session_id, ConversationPlacement::Standalone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::app::OpenSessionRequest;
    use ta_store::NavigationRepository;

    const OWNER: &str = "navigation-test-owner";

    #[test]
    fn navigation_derives_agent_rows_and_persists_only_navigation_metadata() {
        let service = AppService::bootstrap().expect("service");
        let session = service
            .open_session(
                "navigation-test",
                OWNER,
                &OpenSessionRequest {
                    title: "Durable conversation".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session");
        let created = service
            .apply_navigation_intent(
                OWNER,
                DaemonNavigationIntent::CreateSpace {
                    title: "Product".to_string(),
                },
            )
            .expect("space");
        let space_id = created.spaces[0].id.clone();
        let project = service
            .apply_navigation_intent(
                OWNER,
                DaemonNavigationIntent::CreateProject {
                    space_id: Some(space_id),
                    title: "Desktop".to_string(),
                    workspace_ids: vec![ta_store::default_test_workspace_id()],
                },
            )
            .expect("project");
        let project_id = project.projects[0].id.clone();
        let snapshot = service
            .apply_navigation_intent(
                OWNER,
                DaemonNavigationIntent::PlaceConversation {
                    session_id: session.id.clone(),
                    placement: ConversationPlacement::Project { project_id },
                },
            )
            .expect("placement");
        assert_eq!(snapshot.agents[0].session_id, session.id);
        assert!(matches!(
            snapshot.conversations[0].placement,
            ConversationPlacement::Project { .. }
        ));
        assert_eq!(snapshot.conversations[0].title, "Durable conversation");
        assert_eq!(snapshot.conversations[0].status, SessionStatus::Idle);
    }

    #[test]
    fn navigation_supports_ungrouped_projects_without_persisting_session_fields() {
        let service = AppService::bootstrap().expect("service");
        let session = service
            .open_session(
                "navigation-test",
                OWNER,
                &OpenSessionRequest {
                    title: "Ungrouped conversation".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session");
        let snapshot = service
            .apply_navigation_intent(
                OWNER,
                DaemonNavigationIntent::CreateProject {
                    space_id: None,
                    title: "Loose work".to_string(),
                    workspace_ids: vec![ta_store::default_test_workspace_id()],
                },
            )
            .expect("ungrouped project");
        let project_id = snapshot.projects[0].id.clone();
        assert_eq!(snapshot.projects[0].space_id, None);

        let snapshot = service
            .apply_navigation_intent(
                OWNER,
                DaemonNavigationIntent::PlaceConversation {
                    session_id: session.id.clone(),
                    placement: ConversationPlacement::Project { project_id },
                },
            )
            .expect("placement");
        assert_eq!(snapshot.conversations[0].title, "Ungrouped conversation");
        assert_eq!(snapshot.conversations[0].status, SessionStatus::Idle);

        let store = service.store.lock().expect("store");
        let state = store.navigation_state(OWNER).expect("navigation state");
        let encoded = serde_json::to_value(state).expect("navigation state JSON");
        assert!(encoded["conversations"][0].get("title").is_none());
        assert!(encoded["conversations"][0].get("status").is_none());
    }
}
