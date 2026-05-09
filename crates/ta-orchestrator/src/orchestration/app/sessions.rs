use std::time::{SystemTime, UNIX_EPOCH};

use ta_store::{CommitSessionOpen, PersistenceStore};
use uuid::Uuid;

use crate::{ListSessionsQuery, SessionAuthority, SessionSummary};

use super::{
    AppService, AppServiceError, AttachSessionResult, OpenSessionRequest, OpenSessionResult,
    SessionPrincipalResolution, project_session_summary,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    /// Insert or replace a workspace projection. Slice 3 layers the full
    /// canonicalization + trust validation pipeline on top of this primitive
    /// from the `daemon.workspace.open` handler. Tests use it directly to
    /// seed fixtures.
    #[allow(dead_code)]
    pub fn upsert_workspace(
        &self,
        workspace: ta_store::WorkspaceProjection,
    ) -> Result<ta_store::WorkspaceProjection, AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.upsert_workspace(workspace)?)
    }

    /// Lookup a workspace projection. Slice 3 wires this into the
    /// `daemon.workspace.get` handler.
    #[allow(dead_code)]
    pub fn workspace(
        &self,
        workspace_id: &ta_protocol::wire::WorkspaceId,
    ) -> Result<Option<ta_store::WorkspaceProjection>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.workspace(workspace_id)?)
    }

    /// List all workspaces. Slice 3 wires this into the
    /// `daemon.workspace.list` handler.
    #[allow(dead_code)]
    pub fn workspaces(&self) -> Result<Vec<ta_store::WorkspaceProjection>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.workspaces()?)
    }

    pub fn list_sessions(
        &self,
        owner_client_name: &str,
        owner_principal_id: &str,
        _: &ListSessionsQuery,
    ) -> Result<Vec<SessionSummary>, AppServiceError> {
        let _owner_client_name = sanitize_session_owner_client_name(owner_client_name)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store
            .sessions()?
            .into_iter()
            .filter(|session| session.owner_principal_id == owner_principal_id)
            .map(project_session_summary)
            .collect())
    }

    pub fn open_session(
        &self,
        owner_client_name: &str,
        owner_principal_id: &str,
        request: &OpenSessionRequest,
    ) -> Result<OpenSessionResult, AppServiceError> {
        let title = request.title.trim();
        if title.is_empty() {
            return Err(AppServiceError::EmptySessionTitle);
        }
        let owner_client_name = sanitize_session_owner_client_name(owner_client_name)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let session_authority = issue_session_authority();

        let session = ta_store::SessionProjection {
            id: crate::SessionId::new(format!("session-{}", Uuid::new_v4().simple()))
                .expect("generated session id should be valid"),
            owner_client_name,
            owner_principal_id,
            current_session_authority_hash: hash_secret(session_authority.as_str()),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: title.to_string(),
            status: crate::SessionStatus::Idle,
            workspace_id: request.workspace_id.clone(),
        };
        let summary = project_session_summary(session.clone());

        let mut store = self.store.lock().expect("app store should not be poisoned");
        if store.workspace(&request.workspace_id)?.is_none() {
            return Err(AppServiceError::WorkspaceNotFound(
                request.workspace_id.as_str().to_string(),
            ));
        }
        store.commit_session_open(CommitSessionOpen {
            session,
            occurred_at_ms: current_time_ms(),
        })?;
        Ok(OpenSessionResult {
            session: summary,
            session_authority,
        })
    }

    pub fn get_session(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<Option<SessionSummary>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.session(session_id)?.map(project_session_summary))
    }

    pub fn attach_session(
        &self,
        owner_client_name: &str,
        owner_principal_id: &str,
        session_id: &crate::SessionId,
        session_authority: &SessionAuthority,
    ) -> Result<AttachSessionResult, AppServiceError> {
        let _owner_client_name = sanitize_session_owner_client_name(owner_client_name)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let presented_authority_hash = hash_secret(session_authority.as_str());
        let next_session_authority = issue_session_authority();
        let next_authority_hash = hash_secret(next_session_authority.as_str());
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let Some(existing) = store.session(session_id)? else {
            return Err(AppServiceError::SessionNotFound(
                session_id.as_str().to_owned(),
            ));
        };
        if existing.owner_principal_id != owner_principal_id {
            return Err(AppServiceError::SessionNotFound(
                session_id.as_str().to_owned(),
            ));
        }
        store
            .rotate_session_authority(
                session_id,
                &owner_principal_id,
                &presented_authority_hash,
                &next_authority_hash,
            )?
            .map(|session| AttachSessionResult {
                session: project_session_summary(session),
                session_authority: next_session_authority,
            })
            .ok_or_else(|| {
                AppServiceError::SessionAuthorityRejected(session_id.as_str().to_owned())
            })
    }

    pub fn resolve_or_issue_session_principal(
        &self,
        owner_client_name: &str,
        presented_client_credential: Option<&str>,
    ) -> Result<SessionPrincipalResolution, AppServiceError> {
        let owner_client_name = sanitize_session_owner_client_name(owner_client_name)?;
        let mut store = self.store.lock().expect("app store should not be poisoned");

        if let Some(client_credential) = presented_client_credential {
            let client_credential = sanitize_client_credential(client_credential)?;
            let credential_hash = hash_secret(&client_credential);
            if let Some(principal) = store.principal_by_credential_hash(&credential_hash)? {
                return Ok(SessionPrincipalResolution {
                    client_name: principal.client_name,
                    principal_id: principal.id,
                    client_credential,
                });
            }
        }

        let client_credential = issue_client_credential();
        let principal = ta_store::PrincipalProjection {
            id: format!("principal-{}", Uuid::new_v4().simple()),
            client_name: owner_client_name.clone(),
            credential_hash: hash_secret(&client_credential),
        };
        let principal_id = principal.id.clone();
        store.save_principal(principal)?;
        Ok(SessionPrincipalResolution {
            client_name: owner_client_name,
            principal_id,
            client_credential,
        })
    }
}

pub(super) fn sanitize_session_owner_client_name(
    owner_client_name: &str,
) -> Result<String, AppServiceError> {
    let owner_client_name = owner_client_name.trim();
    if owner_client_name.is_empty() {
        Err(AppServiceError::EmptySessionOwnerClientName)
    } else {
        Ok(owner_client_name.to_string())
    }
}

pub(super) fn sanitize_session_owner_principal_id(
    owner_principal_id: &str,
) -> Result<String, AppServiceError> {
    let owner_principal_id = owner_principal_id.trim();
    if owner_principal_id.is_empty() {
        Err(AppServiceError::EmptySessionOwnerPrincipalId)
    } else {
        Ok(owner_principal_id.to_string())
    }
}

fn sanitize_client_credential(client_credential: &str) -> Result<String, AppServiceError> {
    let client_credential = client_credential.trim();
    if client_credential.len() < 32 || !client_credential.is_ascii() {
        return Err(AppServiceError::InvalidClientCredentialLength);
    }
    if client_credential.chars().any(char::is_whitespace) {
        return Err(AppServiceError::InvalidClientCredentialWhitespace);
    }
    Ok(client_credential.to_string())
}

fn issue_client_credential() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn issue_session_authority() -> SessionAuthority {
    SessionAuthority::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ))
    .expect("generated session authority should be valid")
}

fn hash_secret(secret: &str) -> String {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_millis() as u64
}
