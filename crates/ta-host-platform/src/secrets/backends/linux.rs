use std::collections::HashMap;

use secret_service::{EncryptionType, Error as SecretServiceError, blocking::SecretService};

use crate::{
    HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue, secrets::SECRET_CONTENT_TYPE,
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
}

impl HostSecretStore for LinuxSecretServiceStore {
    fn store_secret(
        &self,
        key: HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        let account = key.account_name(&self.service);
        let service = self.service.clone();
        let payload = value.expose_secret().as_bytes().to_vec();
        with_collection(move |collection| {
            collection.create_item(
                &account,
                attributes(&service, &account),
                &payload,
                true,
                SECRET_CONTENT_TYPE,
            )?;
            Ok(())
        })
    }

    fn load_secret(&self, key: HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
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
            Some(payload) => String::from_utf8(payload)
                .map_err(|error| HostSecretError::decrypt_failed(BACKEND_NAME, error))
                .and_then(HostSecretValue::new)
                .map(Some),
            None => Ok(None),
        }
    }

    fn delete_secret(&self, key: HostSecretKey) -> Result<(), HostSecretError> {
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
) -> Result<T, HostSecretError>
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
            return Err(HostSecretError::backend_unavailable(
                BACKEND_NAME,
                "default collection is locked",
            ));
        }
        operation(&collection).map_err(map_secret_service_error)
    })
    .join()
    .map_err(|_| {
        HostSecretError::backend_unavailable(BACKEND_NAME, "secret-service worker panicked")
    })?
}

fn connect() -> Result<SecretService<'static>, HostSecretError> {
    SecretService::connect(EncryptionType::Dh).map_err(map_secret_service_error)
}

fn attributes<'a>(service: &'a str, account: &'a str) -> HashMap<&'a str, &'a str> {
    HashMap::from([(ATTR_SERVICE, service), (ATTR_ACCOUNT, account)])
}

fn map_secret_service_error(error: SecretServiceError) -> HostSecretError {
    match error {
        SecretServiceError::NoResult => HostSecretError::NotFound,
        SecretServiceError::Locked
        | SecretServiceError::Prompt
        | SecretServiceError::Unavailable => {
            HostSecretError::backend_unavailable(BACKEND_NAME, error)
        }
        SecretServiceError::Crypto(_) => HostSecretError::decrypt_failed(BACKEND_NAME, error),
        SecretServiceError::Zbus(_) | SecretServiceError::ZbusFdo(_) => {
            HostSecretError::io_error("dbus", error)
        }
        _ => HostSecretError::backend_unavailable(BACKEND_NAME, error),
    }
}
