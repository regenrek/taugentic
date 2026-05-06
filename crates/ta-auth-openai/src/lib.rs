//! Native OpenAI ChatGPT subscription OAuth support.
//!
//! The crate owns browser PKCE login, callback handling, token exchanges, and
//! secure credential storage for ChatGPT subscription credentials.

pub mod browser;
pub mod client;
pub mod config;
pub mod credential_store;
pub mod error;
pub mod flow;
pub mod oauth;
pub mod refresh;
pub mod token;

#[cfg(test)]
mod flow_tests;

pub use config::{OAuthConfig, default_chatgpt_subscription_config};
pub use credential_store::{
    AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
    default_store,
};
pub use error::OAuthError;
pub use flow::{CompletionHandle, OAuthCode, OpenAiOAuthFlow, TokenSet};
pub use oauth::pkce;
pub use oauth::server as callback_server;
pub use refresh::{
    AccessToken, RefreshPolicy, TokenLifecycleEvent, TokenManager, TokenManagerHandle,
    TokenRefreshError, token_manager_handle,
};
