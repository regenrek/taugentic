use std::sync::Arc;

use crate::client::OAuthHttpClient;
use crate::{CredentialStore, OAuthConfig};

mod error;
mod lifecycle;
mod manager;
mod policy;

#[cfg(test)]
mod tests;

pub use error::TokenRefreshError;
pub use lifecycle::TokenLifecycleEvent;
pub use manager::{AccessToken, TokenManager};
pub use policy::RefreshPolicy;

pub type TokenManagerHandle = Arc<TokenManager>;

pub fn token_manager_handle(
    store: Arc<dyn CredentialStore>,
    http: Arc<dyn OAuthHttpClient>,
    config: OAuthConfig,
    policy: RefreshPolicy,
) -> TokenManagerHandle {
    Arc::new(TokenManager::new(store, http, config, policy))
}

pub(crate) use lifecycle::TokenLifecycleBroadcaster;
