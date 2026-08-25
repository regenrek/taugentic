use std::sync::Arc;

use ta_host_platform::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};

use crate::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
};

pub(crate) struct HostCredentialStore {
    store: Arc<dyn HostSecretStore>,
}

impl HostCredentialStore {
    pub(crate) fn new(store: Arc<dyn HostSecretStore>) -> Self {
        Self { store }
    }

    fn host_key(key: &CredentialKey) -> Result<HostSecretKey, CredentialStoreError> {
        HostSecretKey::new(key.as_str())
            .map_err(|error| CredentialStoreError::serialization("address", error))
    }
}

impl CredentialStore for HostCredentialStore {
    fn store(
        &self,
        key: &CredentialKey,
        credentials: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        let key = Self::host_key(key)?;
        let payload = serde_json::to_string(credentials)
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        let value = HostSecretValue::new(payload)
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        self.store
            .store_secret(&key, &value)
            .map_err(map_host_secret_error)
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        let key = Self::host_key(key)?;
        let Some(value) = self
            .store
            .load_secret(&key)
            .map_err(map_host_secret_error)?
        else {
            return Ok(None);
        };
        serde_json::from_str(value.expose_secret())
            .map(Some)
            .map_err(|error| CredentialStoreError::serialization("decode", error))
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        let key = Self::host_key(key)?;
        self.store
            .delete_secret(&key)
            .map_err(map_host_secret_error)
    }

    fn backend_name(&self) -> &'static str {
        self.store.backend_name()
    }
}

pub(crate) fn map_host_secret_error(error: HostSecretError) -> CredentialStoreError {
    match error {
        HostSecretError::InvalidServiceName
        | HostSecretError::EmptyKey
        | HostSecretError::EmptySecret => CredentialStoreError::serialization("host-secret", error),
        HostSecretError::BackendUnavailable { backend, reason } => {
            CredentialStoreError::backend_unavailable(backend, reason)
        }
        HostSecretError::NotFound => CredentialStoreError::NotFound,
        HostSecretError::EncryptFailed { backend, reason } => {
            CredentialStoreError::encrypt_failed(backend, reason)
        }
        HostSecretError::DecryptFailed { backend, reason } => {
            CredentialStoreError::decrypt_failed(backend, reason)
        }
        HostSecretError::IoError { operation, reason } => {
            CredentialStoreError::io_error(operation, reason)
        }
    }
}
