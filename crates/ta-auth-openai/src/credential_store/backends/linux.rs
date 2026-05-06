use std::collections::HashMap;

use secret_service::{EncryptionType, Error as SecretServiceError, blocking::SecretService};

use crate::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreError, PAYLOAD_CONTENT_TYPE, StoredCredentials,
};

const BACKEND_NAME: &str = "linux-secret-service";
const ATTR_SERVICE: &str = "service";
const ATTR_ACCOUNT: &str = "account";

pub(crate) struct LinuxSecretServiceStore {
    service: String,
}

impl LinuxSecretServiceStore {
    pub(crate) fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn available(service: impl Into<String>) -> Result<Self, CredentialStoreError> {
        let store = Self::new(service);
        store.probe()?;
        Ok(store)
    }

    #[cfg(test)]
    fn probe(&self) -> Result<(), CredentialStoreError> {
        with_collection(|_| Ok(()))
    }
}

impl CredentialStore for LinuxSecretServiceStore {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        let account = key.account_name(&self.service);
        let payload = serde_json::to_vec(creds)
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        let service = self.service.clone();
        with_collection(move |collection| {
            collection.create_item(
                &account,
                attributes(&service, &account),
                &payload,
                true,
                PAYLOAD_CONTENT_TYPE,
            )?;
            Ok(())
        })
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        let account = key.account_name(&self.service);
        let service = self.service.clone();
        let payload = with_collection(move |collection| {
            let mut items = collection.search_items(attributes(&service, &account))?;
            if items.is_empty() {
                return Ok(None);
            }
            items.remove(0).get_secret().map(Some)
        })?;

        match payload {
            Some(payload) => serde_json::from_slice(&payload)
                .map(Some)
                .map_err(|error| CredentialStoreError::serialization("decode", error)),
            None => Ok(None),
        }
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        let account = key.account_name(&self.service);
        let service = self.service.clone();
        with_collection(move |collection| {
            for item in collection.search_items(attributes(&service, &account))? {
                item.delete()?;
            }
            Ok(())
        })
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
}

fn with_collection<T>(
    operation: impl FnOnce(&secret_service::blocking::Collection<'_>) -> Result<T, SecretServiceError>
    + Send
    + 'static,
) -> Result<T, CredentialStoreError>
where
    T: Send + 'static,
{
    std::thread::spawn(move || {
        let secret_service = connect()?;
        let collection = secret_service
            .get_default_collection()
            .or_else(|_| secret_service.get_any_collection())
            .map_err(map_secret_service_error)?;
        if collection.is_locked().map_err(map_secret_service_error)? {
            return Err(CredentialStoreError::backend_unavailable(
                BACKEND_NAME,
                "default collection is locked",
            ));
        }
        operation(&collection).map_err(map_secret_service_error)
    })
    .join()
    .map_err(|_| {
        CredentialStoreError::backend_unavailable(BACKEND_NAME, "secret-service worker panicked")
    })?
}

fn connect() -> Result<SecretService<'static>, CredentialStoreError> {
    SecretService::connect(EncryptionType::Dh).map_err(map_secret_service_error)
}

fn attributes<'a>(service: &'a str, account: &'a str) -> HashMap<&'a str, &'a str> {
    HashMap::from([(ATTR_SERVICE, service), (ATTR_ACCOUNT, account)])
}

fn map_secret_service_error(error: SecretServiceError) -> CredentialStoreError {
    match error {
        SecretServiceError::NoResult => CredentialStoreError::NotFound,
        SecretServiceError::Locked
        | SecretServiceError::Prompt
        | SecretServiceError::Unavailable => {
            CredentialStoreError::backend_unavailable(BACKEND_NAME, error)
        }
        SecretServiceError::Crypto(_) => CredentialStoreError::decrypt_failed(BACKEND_NAME, error),
        SecretServiceError::Zvariant(_) => CredentialStoreError::serialization("dbus", error),
        SecretServiceError::Zbus(_) | SecretServiceError::ZbusFdo(_) => {
            CredentialStoreError::io_error("dbus", error)
        }
        _ => CredentialStoreError::backend_unavailable(BACKEND_NAME, error),
    }
}
