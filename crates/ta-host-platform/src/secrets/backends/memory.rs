use std::{collections::HashMap, sync::Mutex};

use crate::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};

#[derive(Default)]
pub(crate) struct MemoryHostSecretStore {
    secrets: Mutex<HashMap<HostSecretKey, HostSecretValue>>,
}

impl HostSecretStore for MemoryHostSecretStore {
    fn store_secret(
        &self,
        key: HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        self.secrets
            .lock()
            .map_err(|_| {
                HostSecretError::backend_unavailable(
                    self.backend_name(),
                    "memory host secret store lock poisoned",
                )
            })?
            .insert(key, value.clone());
        Ok(())
    }

    fn load_secret(&self, key: HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
        let secrets = self.secrets.lock().map_err(|_| {
            HostSecretError::backend_unavailable(
                self.backend_name(),
                "memory host secret store lock poisoned",
            )
        })?;
        Ok(secrets.get(&key).cloned())
    }

    fn delete_secret(&self, key: HostSecretKey) -> Result<(), HostSecretError> {
        self.secrets
            .lock()
            .map_err(|_| {
                HostSecretError::backend_unavailable(
                    self.backend_name(),
                    "memory host secret store lock poisoned",
                )
            })?
            .remove(&key);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}
