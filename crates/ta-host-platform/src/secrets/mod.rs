use std::sync::Arc;

mod backends;
mod error;
mod types;

pub use error::HostSecretError;
pub use types::{HostSecretKey, HostSecretValue};

pub const HOST_SECRET_SERVICE_NAME: &str = "taugentic.host.secrets";
#[cfg(target_os = "linux")]
pub(crate) const SECRET_CONTENT_TYPE: &str = "text/plain";

pub trait HostSecretStore: Send + Sync {
    fn store_secret(
        &self,
        key: HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError>;
    fn load_secret(&self, key: HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError>;
    fn delete_secret(&self, key: HostSecretKey) -> Result<(), HostSecretError>;
    fn backend_name(&self) -> &'static str;
}

pub fn default_host_secret_store() -> Result<Arc<dyn HostSecretStore>, HostSecretError> {
    #[cfg(target_os = "macos")]
    {
        match crate::secrets_backend_capability() {
            crate::SecretsBackend::Keychain => Ok(Arc::new(
                backends::macos::MacosKeychainStore::new(HOST_SECRET_SERVICE_NAME),
            )),
            backend => Err(HostSecretError::backend_unavailable(
                "macos-keychain",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(target_os = "linux")]
    {
        match crate::secrets_backend_capability() {
            crate::SecretsBackend::SecretService => Ok(Arc::new(
                backends::linux::LinuxSecretServiceStore::new(HOST_SECRET_SERVICE_NAME),
            )),
            backend => {
                tracing::warn!(
                    backend = "linux-secret-service",
                    selected_backend = ?backend,
                    fallback = "memory",
                    "secure host secret backend unavailable; using non-durable in-memory secrets"
                );
                Ok(Arc::new(backends::memory::MemoryHostSecretStore::default()))
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match crate::secrets_backend_capability() {
            crate::SecretsBackend::CredentialManager => Ok(Arc::new(
                backends::windows::WindowsCredentialManagerStore::new(HOST_SECRET_SERVICE_NAME),
            )),
            backend => Err(HostSecretError::backend_unavailable(
                "windows-credential-manager",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        tracing::warn!(
            fallback = "memory",
            "secure host secret backend unsupported; using non-durable in-memory secrets"
        );
        Ok(Arc::new(backends::memory::MemoryHostSecretStore::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_source_github_pat_key_has_stable_account_name() {
        assert_eq!(
            HostSecretKey::WORK_SOURCE_GITHUB_PAT.account_name(HOST_SECRET_SERVICE_NAME),
            "taugentic.host.secrets/work_source.github/github_pat"
        );
    }

    #[test]
    fn memory_store_round_trips_secret_values() -> Result<(), HostSecretError> {
        let store = backends::memory::MemoryHostSecretStore::default();
        let key = HostSecretKey::WORK_SOURCE_GITHUB_PAT;
        let value = HostSecretValue::new("ghp_test")?;

        store.store_secret(key, &value)?;

        assert_eq!(
            store
                .load_secret(key)?
                .map(|secret| secret.expose_secret().to_string()),
            Some("ghp_test".to_string())
        );
        store.delete_secret(key)?;
        assert!(store.load_secret(key)?.is_none());
        Ok(())
    }
}
