use std::sync::{Arc, Mutex};

use crate::{
    ApprovalActor, DaemonClientCapabilities, HANDOFF_CLIENT_NAME, SessionId, invalid_params,
};

#[derive(Debug, Default)]
pub(super) struct DaemonRpcSessionState {
    pub(super) initialized: bool,
    pub(super) client_name: Option<String>,
    pub(super) client_credential: Option<String>,
    pub(super) principal_id: Option<String>,
    pub(super) attached_session_id: Option<SessionId>,
}

pub(super) fn validate_client_name(client_name: &str) -> Result<(), crate::JsonRpcErrorObject> {
    if client_name.trim().is_empty() {
        return Err(invalid_params(
            "daemon.initialize requires non-empty clientName",
        ));
    }

    Ok(())
}

pub(super) fn validate_client_capabilities(
    capabilities: &DaemonClientCapabilities,
) -> Result<(), crate::JsonRpcErrorObject> {
    if !capabilities.notifications || !capabilities.event_subscriptions {
        return Err(invalid_params(
            "daemon clients must support notifications and event subscriptions",
        ));
    }

    Ok(())
}

pub(super) fn ensure_initialized(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<(), crate::JsonRpcErrorObject> {
    if session_state
        .lock()
        .expect("daemon rpc session state should not be poisoned")
        .initialized
    {
        return Ok(());
    }

    Err(invalid_params(format!(
        "{method} requires daemon.initialize first"
    )))
}

pub(super) fn require_internal_handoff_client(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<(), crate::JsonRpcErrorObject> {
    ensure_initialized(session_state, method)?;
    let client_name = session_state
        .lock()
        .expect("daemon rpc session state should not be poisoned")
        .client_name
        .clone();
    match client_name.as_deref() {
        Some(HANDOFF_CLIENT_NAME) => Ok(()),
        Some(other) => Err(invalid_params(format!(
            "{method} is reserved for the internal handoff client; got {other}"
        ))),
        None => Err(invalid_params(format!(
            "{method} requires daemon.initialize first"
        ))),
    }
}

pub(super) fn require_attached_session(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<SessionId, crate::JsonRpcErrorObject> {
    let attached_session_id = session_state
        .lock()
        .expect("daemon rpc session state should not be poisoned")
        .attached_session_id
        .clone();
    if let Some(attached_session_id) = attached_session_id {
        return Ok(attached_session_id);
    }

    Err(invalid_params(format!(
        "{method} requires daemon.session.open or daemon.session.attach before use"
    )))
}

pub(super) fn approval_actor_from_session(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<ApprovalActor, crate::JsonRpcErrorObject> {
    let principal_id = require_principal_id(session_state, method)?;
    ApprovalActor::new(principal_id).map_err(|error| invalid_params(error.to_string()))
}

pub(super) fn require_client_name(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<String, crate::JsonRpcErrorObject> {
    ensure_initialized(session_state, method)?;
    let client_name = session_state
        .lock()
        .expect("daemon rpc session state should not be poisoned")
        .client_name
        .clone();
    client_name.ok_or_else(|| invalid_params(format!("{method} requires daemon.initialize first")))
}

pub(super) fn require_principal_id(
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    method: &str,
) -> Result<String, crate::JsonRpcErrorObject> {
    let principal_id = session_state
        .lock()
        .expect("daemon rpc session state should not be poisoned")
        .principal_id
        .clone();
    principal_id.ok_or_else(|| invalid_params(format!("{method} requires daemon.initialize first")))
}
