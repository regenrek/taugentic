use std::collections::{BTreeMap, btree_map::Entry};
use std::sync::{Mutex, OnceLock};

use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthMethodId, AuthProfileConnectionState, AuthProfileId,
    AuthProfileLoginChallenge, AuthProfileLoginMethod, AuthProfileLoginResult,
    AuthProfileLogoutResult, AuthProfileManagementMode, AuthProfileMethodInfo,
    AuthProfilePreferences, AuthProfileRef, AuthProfileState, AuthProfileUsage,
};
use url::Url;

use crate::families::codex_app_server::{
    CODEX_PROVIDER_ID, CodexAppServerClient, CodexLlmClientError, client::CodexAppServerSession,
    run_on_control_thread,
};

const CODEX_CHATGPT_AUTH_METHOD_ID: &str = "codex-chatgpt";

struct PendingCodexLogin {
    session: CodexAppServerSession,
    login_id: String,
}

static PENDING_LOGINS: OnceLock<Mutex<BTreeMap<AuthProfileId, PendingCodexLogin>>> =
    OnceLock::new();

pub(crate) fn login(
    client: &CodexAppServerClient,
    auth_method_id: &AuthMethodId,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, CodexLlmClientError> {
    if auth_method_id.as_str() != CODEX_CHATGPT_AUTH_METHOD_ID {
        return Err(CodexLlmClientError::UnknownAuthProfile(
            "unsupported Codex auth method".to_string(),
        ));
    }
    let client = client.clone();
    let worker_profile_id = auth_profile_id.clone();
    let (session, login_id, authorize_url) = run_on_control_thread(move || {
        let mut session = client.start_control_session_for_profile(&worker_profile_id)?;
        let (login_id, authorize_url) = session.start_chatgpt_login()?;
        Ok((session, login_id, authorize_url))
    })?;
    let authorize_url = Url::parse(&authorize_url).map_err(|_| {
        CodexLlmClientError::Protocol(
            "account/login/start returned an invalid auth URL".to_string(),
        )
    })?;
    if authorize_url.scheme() != "https" {
        return Err(CodexLlmClientError::Protocol(
            "account/login/start returned a non-HTTPS auth URL".to_string(),
        ));
    }

    let mut pending = pending_logins().lock().map_err(|_| {
        CodexLlmClientError::CommandFailed(
            "Codex ChatGPT pending-login registry is unavailable".to_string(),
        )
    })?;
    match pending.entry(auth_profile_id.clone()) {
        Entry::Vacant(entry) => {
            entry.insert(PendingCodexLogin { session, login_id });
        }
        Entry::Occupied(_) => {
            return Err(CodexLlmClientError::InvalidConfig(
                "Codex ChatGPT login is already pending for this profile".to_string(),
            ));
        }
    }

    Ok(AuthProfileLoginResult {
        auth_profile: auth_profile_state(
            auth_profile_id,
            AuthProfileConnectionState::PendingLogin,
            None,
            None,
        ),
        challenge: Some(AuthProfileLoginChallenge {
            auth_profile_id: auth_profile_id.clone(),
            method: AuthProfileLoginMethod::Browser,
            manual_browser_url: None,
            authorize_url: Some(authorize_url.into()),
            user_code: None,
        }),
    })
}

pub(crate) fn complete_login(
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, CodexLlmClientError> {
    let pending = pending_logins()
        .lock()
        .map_err(|_| {
            CodexLlmClientError::CommandFailed(
                "Codex ChatGPT pending-login registry is unavailable".to_string(),
            )
        })?
        .remove(auth_profile_id)
        .ok_or_else(|| {
            CodexLlmClientError::UnknownAuthProfile(
                "Codex ChatGPT login is not pending for this profile".to_string(),
            )
        })?;
    let (account_hint, plan_tier) = run_on_control_thread(move || {
        let PendingCodexLogin {
            mut session,
            login_id,
        } = pending;
        session.wait_for_chatgpt_login(&login_id)?;
        session.read_chatgpt_account()?.ok_or_else(|| {
            CodexLlmClientError::Auth(
                "Codex ChatGPT login completed without a connected account".to_string(),
            )
        })
    })?;
    Ok(AuthProfileLoginResult {
        auth_profile: auth_profile_state(
            auth_profile_id,
            AuthProfileConnectionState::Connected,
            account_hint,
            plan_tier,
        ),
        challenge: None,
    })
}

pub(crate) fn logout(
    client: &CodexAppServerClient,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLogoutResult, CodexLlmClientError> {
    if let Some(pending) = pending_logins()
        .lock()
        .map_err(|_| {
            CodexLlmClientError::CommandFailed(
                "Codex ChatGPT pending-login registry is unavailable".to_string(),
            )
        })?
        .remove(auth_profile_id)
    {
        run_on_control_thread(move || {
            drop(pending);
            Ok(())
        })?;
        return Ok(AuthProfileLogoutResult {
            auth_profile_id: auth_profile_id.clone(),
            disconnected: true,
        });
    }
    let client = client.clone();
    let worker_profile_id = auth_profile_id.clone();
    run_on_control_thread(move || {
        let mut session = client.start_control_session_for_profile(&worker_profile_id)?;
        session.logout_account()
    })?;
    Ok(AuthProfileLogoutResult {
        auth_profile_id: auth_profile_id.clone(),
        disconnected: true,
    })
}

fn pending_logins() -> &'static Mutex<BTreeMap<AuthProfileId, PendingCodexLogin>> {
    PENDING_LOGINS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn auth_profile_state(
    auth_profile_id: &AuthProfileId,
    connection_state: AuthProfileConnectionState,
    account_hint: Option<String>,
    plan_tier: Option<String>,
) -> AuthProfileState {
    AuthProfileState {
        profile: AuthProfileRef {
            id: auth_profile_id.clone(),
            auth_method_id: AuthMethodId::new(CODEX_CHATGPT_AUTH_METHOD_ID)
                .expect("auth method id"),
            provider_id: AgentRuntimeStrategyId::new(CODEX_PROVIDER_ID).expect("provider id"),
            display_name: "Codex ChatGPT".to_string(),
            account_hint,
            plan_tier,
        },
        preferences: AuthProfilePreferences {
            label: "Codex ChatGPT".to_string(),
            order: 0,
            is_default: false,
        },
        usage: AuthProfileUsage::Unavailable,
        connection_state,
        exhaustion: None,
        last_error: None,
        management_mode: AuthProfileManagementMode::Interactive,
        can_login: connection_state != AuthProfileConnectionState::PendingLogin,
        can_logout: true,
        platform_org_linked: None,
        setup_steps: Vec::new(),
        action: None,
        methods: vec![AuthProfileMethodInfo {
            id: "browser".to_string(),
            display_name: "Browser login".to_string(),
            management_mode: AuthProfileManagementMode::Interactive,
        }],
    }
}
