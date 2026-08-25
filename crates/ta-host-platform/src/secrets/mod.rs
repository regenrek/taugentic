use std::sync::Arc;

mod backends;
mod error;
mod types;

pub use error::HostSecretError;
pub use types::{HostSecretKey, HostSecretValue};

#[cfg(target_os = "linux")]
pub(crate) const SECRET_CONTENT_TYPE: &str = "text/plain";

pub trait HostSecretStore: Send + Sync {
    fn store_secret(
        &self,
        key: &HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError>;
    fn load_secret(&self, key: &HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError>;
    fn delete_secret(&self, key: &HostSecretKey) -> Result<(), HostSecretError>;
    fn backend_name(&self) -> &'static str;
}

pub fn default_host_secret_store(
    service: impl Into<String>,
) -> Result<Arc<dyn HostSecretStore>, HostSecretError> {
    let service = service.into();
    if service.trim().is_empty() {
        return Err(HostSecretError::InvalidServiceName);
    }

    #[cfg(target_os = "macos")]
    {
        match crate::secrets_backend_capability() {
            crate::SecretsBackend::Keychain => {
                Ok(Arc::new(backends::macos::MacosKeychainStore::new(service)))
            }
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
                backends::linux::LinuxSecretServiceStore::new(service),
            )),
            backend => Err(HostSecretError::backend_unavailable(
                "linux-secret-service",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(target_os = "windows")]
    {
        match crate::secrets_backend_capability() {
            crate::SecretsBackend::CredentialManager => Ok(Arc::new(
                backends::windows::WindowsCredentialManagerStore::new(service),
            )),
            backend => Err(HostSecretError::backend_unavailable(
                "windows-credential-manager",
                format!("ta-host-platform selected unsupported backend {backend:?}"),
            )),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = service;
        Err(HostSecretError::backend_unavailable(
            "host-secret-store",
            "secure host secret storage is unsupported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_secret_identifiers() {
        assert_eq!(HostSecretKey::new("  "), Err(HostSecretError::EmptyKey));
        assert!(matches!(
            default_host_secret_store("  "),
            Err(HostSecretError::InvalidServiceName)
        ));
    }

    #[test]
    fn platform_owns_service_and_key_account_addressing() -> Result<(), HostSecretError> {
        let key = HostSecretKey::new("openai_chatgpt")?;
        assert_eq!(
            key.account_name("taugentic.openai.oauth"),
            "taugentic.openai.oauth/openai_chatgpt"
        );
        Ok(())
    }

    #[test]
    fn memory_store_round_trips_secret_values() -> Result<(), HostSecretError> {
        let store = backends::memory::MemoryHostSecretStore::default();
        let key = HostSecretKey::new("work_source.github/github_pat")?;
        let value = HostSecretValue::new("ghp_test")?;

        store.store_secret(&key, &value)?;

        assert_eq!(
            store
                .load_secret(&key)?
                .map(|secret| secret.expose_secret().to_string()),
            Some("ghp_test".to_string())
        );
        store.delete_secret(&key)?;
        assert!(store.load_secret(&key)?.is_none());
        Ok(())
    }
}
