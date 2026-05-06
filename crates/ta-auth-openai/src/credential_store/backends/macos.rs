use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::credential_store::{
    CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
};

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

impl CredentialStore for MacosKeychainStore {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        let account = key.account_name(&self.service);
        let payload = serde_json::to_vec(creds)
            .map_err(|error| CredentialStoreError::serialization("encode", error))?;
        set_generic_password(&self.service, &account, &payload)
            .map_err(|error| CredentialStoreError::encrypt_failed(BACKEND_NAME, error))
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        let account = key.account_name(&self.service);
        match get_generic_password(&self.service, &account) {
            Ok(payload) => serde_json::from_slice(&payload)
                .map(Some)
                .map_err(|error| CredentialStoreError::serialization("decode", error)),
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(CredentialStoreError::decrypt_failed(BACKEND_NAME, error)),
        }
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        let account = key.account_name(&self.service);
        match delete_generic_password(&self.service, &account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(CredentialStoreError::io_error("delete", error)),
        }
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }
}
