use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use ta_host_platform::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};
use ta_protocol::wire::AuthProfileId;

use super::{
    AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
    backends::{host::HostCredentialStore, memory::MemoryCredentialStore},
    default_store,
};
use crate::TokenSet;

#[test]
fn memory_backend_round_trips_credentials() -> Result<(), Box<dyn Error>> {
    let store = MemoryCredentialStore::default();
    let key = credential_key("openai_chatgpt")?;
    let credentials = stored_credentials("acct_1", "user@example.com", 1);

    assert_eq!(store.load(&key)?, None);

    store.store(&key, &credentials)?;
    assert_eq!(store.load(&key)?, Some(credentials.clone()));

    store.delete(&key)?;
    assert_eq!(store.load(&key)?, None);
    Ok(())
}

#[test]
fn memory_backend_overwrites_credentials() -> Result<(), Box<dyn Error>> {
    let store = MemoryCredentialStore::default();
    let key = credential_key("openai_chatgpt")?;
    let first = stored_credentials("acct_1", "user@example.com", 1);
    let second = stored_credentials("acct_2", "other@example.com", 2);

    store.store(&key, &first)?;
    store.store(&key, &second)?;

    assert_eq!(store.load(&key)?, Some(second));
    Ok(())
}

#[test]
fn default_store_uses_expected_backend() -> Result<(), Box<dyn Error>> {
    match ta_host_platform::secrets_backend_capability() {
        ta_host_platform::SecretsBackend::Keychain => {
            assert_eq!(default_store()?.backend_name(), "macos-keychain");
        }
        ta_host_platform::SecretsBackend::SecretService => {
            assert_eq!(default_store()?.backend_name(), "linux-secret-service");
        }
        ta_host_platform::SecretsBackend::CredentialManager => {
            assert_eq!(
                default_store()?.backend_name(),
                "windows-credential-manager"
            );
        }
        ta_host_platform::SecretsBackend::None => {
            assert!(matches!(
                default_store(),
                Err(CredentialStoreError::BackendUnavailable { .. })
            ));
        }
    }

    Ok(())
}

#[test]
fn host_adapter_owns_openai_serialization_and_addressing() -> Result<(), Box<dyn Error>> {
    let host = Arc::new(TestHostSecretStore::default());
    let store = HostCredentialStore::new(host.clone());
    let key = credential_key("openai_chatgpt")?;
    let credentials = stored_credentials("acct_1", "user@example.com", 1);

    store.store(&key, &credentials)?;
    assert_eq!(host.last_key()?.as_deref(), Some("openai_chatgpt"));
    assert_eq!(store.load(&key)?, Some(credentials));
    store.delete(&key)?;
    assert_eq!(store.load(&key)?, None);
    Ok(())
}

#[test]
fn credential_store_error_redacts_token_material() {
    let secret = "access_token=secret-token refresh_token=refresh-secret";
    let error = CredentialStoreError::backend_unavailable("memory", secret);

    assert!(!error.to_string().contains("secret-token"));
    assert!(!format!("{error:?}").contains("refresh-secret"));
}

#[test]
fn stored_credentials_debug_redacts_token_material() {
    let credentials = stored_credentials("acct_1", "user@example.com", 1);
    let debug = format!("{credentials:?}");

    assert!(!debug.contains("access-token-1"));
    assert!(!debug.contains("refresh-token-1"));
    assert!(!debug.contains("id-token-1"));
    assert!(!debug.contains("api-access-token-1"));
}

fn credential_key(value: &str) -> Result<CredentialKey, Box<dyn Error>> {
    Ok(CredentialKey::new(AuthProfileId::new(value.to_string())?))
}

fn stored_credentials(account_id: &str, email: &str, suffix: u64) -> StoredCredentials {
    StoredCredentials {
        token_set: TokenSet {
            access_token: format!("access-token-{suffix}"),
            refresh_token: format!("refresh-token-{suffix}"),
            id_token: Some(format!("id-token-{suffix}")),
            expires_in: Some(3600),
            scope: Some("openid profile email offline_access".to_string()),
            api_access_token: Some(format!("api-access-token-{suffix}")),
            account_info: None,
        },
        account: AccountInfo {
            account_id: account_id.to_string(),
            email: email.to_string(),
            organization_id: None,
            plan_tier: Some("plus".to_string()),
        },
        stored_at: suffix,
        last_refreshed_at: None,
    }
}

#[derive(Default)]
struct TestHostSecretStore {
    entry: Mutex<Option<(HostSecretKey, HostSecretValue)>>,
}

impl TestHostSecretStore {
    fn last_key(&self) -> Result<Option<String>, HostSecretError> {
        self.entry
            .lock()
            .map_err(|_| host_lock_error())
            .map(|entry| entry.as_ref().map(|(key, _)| key.as_str().to_string()))
    }
}

impl HostSecretStore for TestHostSecretStore {
    fn store_secret(
        &self,
        key: &HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        *self.entry.lock().map_err(|_| host_lock_error())? = Some((key.clone(), value.clone()));
        Ok(())
    }

    fn load_secret(&self, key: &HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
        self.entry
            .lock()
            .map_err(|_| host_lock_error())
            .map(|entry| {
                entry
                    .as_ref()
                    .filter(|(stored_key, _)| stored_key == key)
                    .map(|(_, value)| value.clone())
            })
    }

    fn delete_secret(&self, key: &HostSecretKey) -> Result<(), HostSecretError> {
        let mut entry = self.entry.lock().map_err(|_| host_lock_error())?;
        if entry
            .as_ref()
            .is_some_and(|(stored_key, _)| stored_key == key)
        {
            *entry = None;
        }
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "test-host-secret-store"
    }
}

fn host_lock_error() -> HostSecretError {
    HostSecretError::IoError {
        operation: "test-host-secret-lock",
        reason: "lock poisoned".to_string(),
    }
}
