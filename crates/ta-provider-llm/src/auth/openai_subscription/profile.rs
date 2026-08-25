use ta_auth_openai::{AccountInfo, StoredCredentials};
use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthProfileConnectionState, AuthProfileManagementMode,
    AuthProfileMethodInfo, AuthProfileRef, AuthProfileState,
};

use super::{OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID, OpenAiSubscriptionAuth, profile_lock};
use crate::families::openai::OPENAI_PROVIDER_ID;

#[derive(Debug)]
enum CredentialRuntimeState {
    Loading,
    LoggedOut,
    Connected(AccountInfo),
    Unavailable(String),
}

#[derive(Debug)]
pub(crate) struct ProfileRuntimeState {
    credentials: CredentialRuntimeState,
    pending_login: bool,
    needs_reauth: Option<String>,
    last_error: Option<String>,
}

impl ProfileRuntimeState {
    pub(crate) fn loading() -> Self {
        Self {
            credentials: CredentialRuntimeState::Loading,
            pending_login: false,
            needs_reauth: None,
            last_error: None,
        }
    }

    pub(crate) fn from_credentials(credentials: Result<Option<StoredCredentials>, String>) -> Self {
        let credentials = match credentials {
            Ok(Some(credentials)) => CredentialRuntimeState::Connected(credentials.account),
            Ok(None) => CredentialRuntimeState::LoggedOut,
            Err(error) => CredentialRuntimeState::Unavailable(error),
        };
        Self {
            credentials,
            pending_login: false,
            needs_reauth: None,
            last_error: None,
        }
    }
}

pub(crate) fn current_state(auth: &OpenAiSubscriptionAuth) -> AuthProfileState {
    let runtime = profile_lock(auth);
    state_from_runtime(&runtime)
}

pub(crate) fn record_pending_login(auth: &OpenAiSubscriptionAuth) {
    let mut state = profile_lock(auth);
    state.pending_login = true;
    state.last_error = None;
}

pub(crate) fn record_initial_credentials(
    auth: &OpenAiSubscriptionAuth,
    credentials: Result<Option<StoredCredentials>, String>,
) {
    let mut state = profile_lock(auth);
    if matches!(state.credentials, CredentialRuntimeState::Loading) {
        state.credentials = ProfileRuntimeState::from_credentials(credentials).credentials;
    }
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
    state.credentials = CredentialRuntimeState::Connected(account);
}

pub(crate) fn record_logged_out(auth: &OpenAiSubscriptionAuth) {
    let mut state = profile_lock(auth);
    state.pending_login = false;
    state.needs_reauth = None;
    state.last_error = None;
    state.credentials = CredentialRuntimeState::LoggedOut;
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

fn state_from_runtime(runtime: &ProfileRuntimeState) -> AuthProfileState {
    if runtime.pending_login {
        return profile_state(AuthProfileConnectionState::PendingLogin, None, false, false);
    }
    if let Some(reason) = runtime.needs_reauth.as_ref() {
        let can_logout = matches!(runtime.credentials, CredentialRuntimeState::Connected(_));
        return profile_state(
            AuthProfileConnectionState::Error,
            Some(format!(
                "OpenAI ChatGPT subscription needs re-authentication: {reason}"
            )),
            true,
            can_logout,
        );
    }
    match &runtime.credentials {
        CredentialRuntimeState::Loading => {
            profile_state(AuthProfileConnectionState::Loading, None, false, false)
        }
        CredentialRuntimeState::Connected(account) => profile_state(
            AuthProfileConnectionState::Connected,
            runtime.last_error.clone(),
            false,
            true,
        )
        .with_platform_org_linked(account.organization_id.is_some())
        .without_setup_steps(),
        CredentialRuntimeState::LoggedOut => profile_state(
            AuthProfileConnectionState::LoggedOut,
            runtime.last_error.clone(),
            true,
            false,
        ),
        CredentialRuntimeState::Unavailable(error) => profile_state(
            AuthProfileConnectionState::Error,
            Some(error.clone()),
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
