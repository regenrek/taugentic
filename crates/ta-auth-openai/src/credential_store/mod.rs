//! Secure credential storage for OpenAI ChatGPT subscription OAuth.
//!
//! Service name: [`SERVICE_NAME`] (`"taugentic.openai.oauth"`).
//! Account names use `format!("{}/{}", service, credential_key.as_str())`, for
//! example `"taugentic.openai.oauth/openai_chatgpt"`.
//! Stored payloads are JSON-serialized [`StoredCredentials`]. Taugentic relies on
//! the OS credential store for encryption at rest and does not add a second
//! encryption layer.

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
    #[cfg(target_os = "macos")]
    {
        match ta_host_platform::secrets_backend_capability() {
            ta_host_platform::SecretsBackend::Keychain => Ok(Arc::new(
                backends::macos::MacosKeychainStore::new(SERVICE_NAME),
            )),
            backend => Err(CredentialStoreError::backend_unavailable(
                "macos-keychain",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(target_os = "linux")]
    {
        match ta_host_platform::secrets_backend_capability() {
            ta_host_platform::SecretsBackend::SecretService => Ok(Arc::new(
                backends::linux::LinuxSecretServiceStore::new(SERVICE_NAME),
            )),
            backend => {
                tracing::warn!(
                    backend = "linux-secret-service",
                    selected_backend = ?backend,
                    fallback = "memory",
                    "secure credential backend unavailable; using non-durable in-memory credentials"
                );
                Ok(Arc::new(backends::memory::MemoryCredentialStore::default()))
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match ta_host_platform::secrets_backend_capability() {
            ta_host_platform::SecretsBackend::CredentialManager => Ok(Arc::new(
                backends::windows::WindowsCredentialManagerStore::new(SERVICE_NAME),
            )),
            backend => Err(CredentialStoreError::backend_unavailable(
                "windows-credential-manager",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        tracing::warn!(
            fallback = "memory",
            "secure credential backend unsupported on this platform; using non-durable in-memory credentials"
        );
        Ok(Arc::new(backends::memory::MemoryCredentialStore::default()))
    }
}
