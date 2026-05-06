use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};

use ta_protocol::wire::AuthProfileId;

use super::{
    AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, StoredCredentials,
    backends::memory::MemoryCredentialStore, default_store,
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
    let store = default_store()?;

    #[cfg(target_os = "macos")]
    assert_eq!(store.backend_name(), "macos-keychain");

    #[cfg(target_os = "linux")]
    assert!(
        matches!(store.backend_name(), "linux-secret-service" | "memory"),
        "unexpected backend: {}",
        store.backend_name()
    );

    #[cfg(target_os = "windows")]
    assert_eq!(store.backend_name(), "windows-credential-manager");

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    assert_eq!(store.backend_name(), "memory");

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

#[cfg(target_os = "macos")]
#[test]
fn macos_keychain_round_trips_credentials() -> Result<(), Box<dyn Error>> {
    use super::backends::macos::MacosKeychainStore;

    let service = unique_service_name("macos-keychain")?;
    let store = MacosKeychainStore::new(service);
    native_round_trip(&store)
}

#[cfg(target_os = "linux")]
#[test]
fn linux_secret_service_round_trips_credentials_when_available() -> Result<(), Box<dyn Error>> {
    use super::backends::linux::LinuxSecretServiceStore;

    let service = unique_service_name("linux-secret-service")?;
    let store = match LinuxSecretServiceStore::available(service) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("skipping Secret Service credential test: {error}");
            return Ok(());
        }
    };
    native_round_trip(&store)
}

#[cfg(target_os = "windows")]
#[test]
fn windows_credential_manager_round_trips_credentials() -> Result<(), Box<dyn Error>> {
    use super::backends::windows::WindowsCredentialManagerStore;

    let service = unique_service_name("windows-credential-manager")?;
    let store = WindowsCredentialManagerStore::new(service);
    native_round_trip(&store)
}

fn native_round_trip(store: &dyn CredentialStore) -> Result<(), Box<dyn Error>> {
    let key = credential_key("openai_chatgpt_native_test")?;
    let credentials = stored_credentials("acct_native", "native@example.com", 9);

    if let Err(error) = store.delete(&key) {
        eprintln!(
            "skipping {} credential test during cleanup: {error}",
            store.backend_name()
        );
        return Ok(());
    }
    if let Err(error) = store.store(&key, &credentials) {
        eprintln!(
            "skipping {} credential test during store: {error}",
            store.backend_name()
        );
        return Ok(());
    }
    assert_eq!(store.load(&key)?, Some(credentials));
    store.delete(&key)?;
    assert_eq!(store.load(&key)?, None);
    Ok(())
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

fn unique_service_name(label: &str) -> Result<String, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!("taugentic.openai.oauth.test.{label}.{timestamp}"))
}
