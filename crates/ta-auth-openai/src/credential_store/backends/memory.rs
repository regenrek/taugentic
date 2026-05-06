use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
};

#[derive(Clone, Default)]
pub(crate) struct MemoryCredentialStore {
    credentials: Arc<Mutex<HashMap<CredentialKey, StoredCredentials>>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        self.credentials
            .lock()
            .map_err(|_| {
                CredentialStoreError::backend_unavailable(
                    self.backend_name(),
                    "memory credential store lock poisoned",
                )
            })?
            .insert(key.clone(), creds.clone());
        Ok(())
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        let credentials = self.credentials.lock().map_err(|_| {
            CredentialStoreError::backend_unavailable(
                self.backend_name(),
                "memory credential store lock poisoned",
            )
        })?;
        Ok(credentials.get(key).cloned())
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        self.credentials
            .lock()
            .map_err(|_| {
                CredentialStoreError::backend_unavailable(
                    self.backend_name(),
                    "memory credential store lock poisoned",
                )
            })?
            .remove(key);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}
