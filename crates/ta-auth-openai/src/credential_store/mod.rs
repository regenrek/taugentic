//! Secure credential storage for OpenAI ChatGPT subscription OAuth.
//!
//! Service name: [`SERVICE_NAME`] (`"taugentic.openai.oauth"`).
//! Account names use `format!("{}/{}", service, credential_key.as_str())`, for
//! example `"taugentic.openai.oauth/auth-openai-chatgpt"`.
//! Stored payloads are JSON-serialized [`StoredCredentials`]. Taugentic relies on
//! [`ta_host_platform`] as the sole OS credential-store owner and does not add a
//! second encryption layer.

use std::sync::Arc;

mod backends;
mod error;
mod types;

#[cfg(test)]
mod tests;

pub use error::CredentialStoreError;
pub use types::{AccountInfo, CredentialKey, StoredCredentials};

pub const SERVICE_NAME: &str = "taugentic.openai.oauth";
#[cfg(target_os = "linux")]
pub(crate) const PAYLOAD_CONTENT_TYPE: &str = "application/json";

pub trait CredentialStore: Send + Sync {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError>;
    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError>;
    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError>;
    fn backend_name(&self) -> &'static str;
}

pub fn default_store() -> Result<Arc<dyn CredentialStore>, CredentialStoreError> {
    let store = ta_host_platform::default_host_secret_store(SERVICE_NAME)
        .map_err(backends::host::map_host_secret_error)?;
    Ok(Arc::new(backends::host::HostCredentialStore::new(store)))
}
