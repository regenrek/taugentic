mod account;
pub(crate) mod client;
mod events;
mod framing;
pub(crate) mod launch;
mod models;
mod policy;
mod process;
mod search_path;
mod types;

pub use client::{CodexAppServerClient, CodexAppServerInput};
pub use events::{CodexAppServerEvent, CodexToolCallOutcome};
pub use models::{CodexModelCatalog, model_catalog};
pub use search_path::TAUGENTIC_CODEX_APP_SERVER_BIN_ENV;
pub use types::*;

pub fn login(
    auth_method_id: &ta_protocol::wire::AuthMethodId,
    auth_profile_id: &ta_protocol::wire::AuthProfileId,
) -> CodexLoginResult {
    crate::auth::codex_oauth::login(
        &CodexAppServerClient::default(),
        auth_method_id,
        auth_profile_id,
    )
}

pub fn complete_login(auth_profile_id: &ta_protocol::wire::AuthProfileId) -> CodexLoginResult {
    crate::auth::codex_oauth::complete_login(auth_profile_id)
}

pub fn logout(auth_profile_id: &ta_protocol::wire::AuthProfileId) -> CodexLogoutResult {
    crate::auth::codex_oauth::logout(&CodexAppServerClient::default(), auth_profile_id)
}
