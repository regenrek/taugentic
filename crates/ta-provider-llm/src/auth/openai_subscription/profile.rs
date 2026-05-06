use ta_auth_openai::{AccountInfo, StoredCredentials};
use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthProfileConnectionState, AuthProfileManagementMode,
    AuthProfileMethodInfo, AuthProfileRef, AuthProfileState,
};

use super::{
    OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID, OpenAiSubscriptionAuth, map_store_error,
    profile_lock,
};
use crate::families::openai::OPENAI_PROVIDER_ID;

#[derive(Debug, Default)]
pub(crate) struct ProfileRuntimeState {
    pending_login: bool,
    needs_reauth: Option<String>,
    last_error: Option<String>,
    last_account: Option<AccountInfo>,
}

pub(crate) fn current_state(auth: &OpenAiSubscriptionAuth) -> AuthProfileState {
    let credentials = auth
        .inner
        .store
        .load(auth.key())
        .map_err(map_store_error)
        .map_err(|error| error.to_string());
    let runtime = profile_lock(auth);
    match credentials {
        Ok(credentials) => state_from_credentials(credentials.as_ref(), &runtime),
        Err(error) => profile_state(AuthProfileConnectionState::Error, Some(error), true, false),
    }
}

pub(crate) fn record_pending_login(auth: &OpenAiSubscriptionAuth) {
    let mut state = profile_lock(auth);
    state.pending_login = true;
    state.last_error = None;
}

pub(crate) fn record_login_failed(auth: &OpenAiSubscriptionAuth, message: String) {
    let mut state = profile_lock(auth);
    state.pending_login = false;
    state.last_error = Some(message);
}

pub(crate) fn record_connected(auth: &OpenAiSubscriptionAuth, account: AccountInfo) {
    let mut state = profile_lock(auth);
    state.pending_login = false;
    state.needs_reauth = None;
    state.last_error = None;
    state.last_account = Some(account);
}

pub(crate) fn record_logged_out(auth: &OpenAiSubscriptionAuth) {
    let mut state = profile_lock(auth);
    state.pending_login = false;
    state.needs_reauth = None;
    state.last_error = None;
    state.last_account = None;
}

pub(crate) fn record_needs_reauth(auth: &OpenAiSubscriptionAuth, reason: String) {
    let mut state = profile_lock(auth);
    state.pending_login = false;
    state.needs_reauth = Some(reason);
}

pub(crate) fn record_refresh_failed(auth: &OpenAiSubscriptionAuth, message: String) {
    let mut state = profile_lock(auth);
    state.last_error = Some(message);
}

fn state_from_credentials(
    credentials: Option<&StoredCredentials>,
    runtime: &ProfileRuntimeState,
) -> AuthProfileState {
    if runtime.pending_login {
        return profile_state(AuthProfileConnectionState::PendingLogin, None, false, false);
    }
    if let Some(reason) = runtime.needs_reauth.as_ref() {
        return profile_state(
            AuthProfileConnectionState::Error,
            Some(format!(
                "OpenAI ChatGPT subscription needs re-authentication: {reason}"
            )),
            true,
            credentials.is_some(),
        );
    }
    match credentials {
        Some(credentials) => profile_state(
            AuthProfileConnectionState::Connected,
            runtime.last_error.clone(),
            false,
            true,
        )
        .with_platform_org_linked(credentials.account.organization_id.is_some())
        .without_setup_steps(),
        None => profile_state(
            AuthProfileConnectionState::LoggedOut,
            runtime.last_error.clone(),
            true,
            false,
        ),
    }
}

fn profile_state(
    connection_state: AuthProfileConnectionState,
    last_error: Option<String>,
    can_login: bool,
    can_logout: bool,
) -> AuthProfileState {
    AuthProfileState {
        profile: AuthProfileRef {
            id: ta_protocol::wire::AuthProfileId::new(OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID)
                .expect("OpenAI ChatGPT auth profile id"),
            provider_id: AgentRuntimeStrategyId::new(OPENAI_PROVIDER_ID).expect("provider id"),
            display_name: "OpenAI ChatGPT Subscription".to_string(),
        },
        connection_state,
        last_error,
        management_mode: AuthProfileManagementMode::Interactive,
        can_login,
        can_logout,
        platform_org_linked: None,
        setup_steps: vec![
            "Use Login to authorize your OpenAI ChatGPT subscription in the browser".to_string(),
        ],
        action: None,
        methods: vec![AuthProfileMethodInfo {
            id: "browser".to_string(),
            display_name: "Browser OAuth".to_string(),
            management_mode: AuthProfileManagementMode::Interactive,
        }],
    }
}

trait ConnectedState {
    fn with_platform_org_linked(self, linked: bool) -> Self;
    fn without_setup_steps(self) -> Self;
}

impl ConnectedState for AuthProfileState {
    fn with_platform_org_linked(mut self, linked: bool) -> Self {
        self.platform_org_linked = Some(linked);
        self
    }

    fn without_setup_steps(mut self) -> Self {
        self.setup_steps.clear();
        self
    }
}
