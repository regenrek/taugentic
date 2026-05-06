use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};

const BACKEND_NAME: &str = "macos-keychain";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25_300;

pub(crate) struct MacosKeychainStore {
    service: String,
}

impl MacosKeychainStore {
    pub(crate) fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl HostSecretStore for MacosKeychainStore {
    fn store_secret(
        &self,
        key: HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        let account = key.account_name(&self.service);
        set_generic_password(&self.service, &account, value.expose_secret().as_bytes())
            .map_err(|error| HostSecretError::encrypt_failed(BACKEND_NAME, error))
    }

    fn load_secret(&self, key: HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
        let account = key.account_name(&self.service);
        match get_generic_password(&self.service, &account) {
            Ok(payload) => String::from_utf8(payload)
                .map_err(|error| HostSecretError::decrypt_failed(BACKEND_NAME, error))
                .and_then(HostSecretValue::new)
                .map(Some),
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(HostSecretError::decrypt_failed(BACKEND_NAME, error)),
        }
    }

    fn delete_secret(&self, key: HostSecretKey) -> Result<(), HostSecretError> {
        let account = key.account_name(&self.service);
        match delete_generic_password(&self.service, &account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(HostSecretError::io_error("delete", error)),
        }
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
}
