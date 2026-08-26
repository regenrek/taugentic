use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use sha2::{Digest, Sha256};
use ta_jsonrpc::{ClientConfig, JsonRpcClientError, SocketAddress};
use ta_protocol::wire::{DAEMON_DEFAULT_SOCKET_NAME, SessionAuthority, SessionId};

pub fn load_client_credential(config: &ClientConfig, client_name: &str) -> Option<String> {
    let path = client_credential_file_path(config, client_name);
    load_persisted_secret(&path, parse_client_credential)
}

pub fn store_client_credential(
    config: &ClientConfig,
    client_name: &str,
    client_credential: &str,
) -> Result<(), JsonRpcClientError> {
    let base_dir = client_credential_base_dir(config);
    let storage_dir = client_credential_storage_dir(config);
    let path = storage_dir.join(client_credential_file_name(client_name));
    prepare_private_directory(&base_dir)?;
    prepare_private_directory(&storage_dir)?;
    write_private_file(&path, client_credential)
}

pub fn load_session_authority(
    config: &ClientConfig,
    client_name: &str,
    session_id: &SessionId,
) -> Option<SessionAuthority> {
    let path = session_authority_file_path(config, client_name, session_id);
    load_persisted_secret(&path, |authority| {
        let authority = authority.trim();
        if authority.is_empty() {
            None
        } else {
            SessionAuthority::new(authority.to_string()).ok()
        }
    })
}

pub fn store_session_authority(
    config: &ClientConfig,
    client_name: &str,
    session_id: &SessionId,
    session_authority: &SessionAuthority,
) -> Result<(), JsonRpcClientError> {
    let base_dir = session_authority_base_dir(config);
    let socket_dir = session_authority_socket_dir(config);
    let storage_dir = session_authority_storage_dir(config, client_name);
    let path = storage_dir.join(session_authority_file_name(session_id));
    prepare_private_directory(&base_dir)?;
    prepare_private_directory(&socket_dir)?;
    prepare_private_directory(&storage_dir)?;
    write_private_file(&path, session_authority.as_str())
}

pub fn remove_session_authority(
    config: &ClientConfig,
    client_name: &str,
    session_id: &SessionId,
) -> Result<(), JsonRpcClientError> {
    let path = session_authority_file_path(config, client_name, session_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(JsonRpcClientError::Read(io_context(error, &path))),
    }
}

pub fn remove_client_session_authorities(
    config: &ClientConfig,
    client_name: &str,
) -> Result<(), JsonRpcClientError> {
    let path = session_authority_storage_dir(config, client_name);
    match fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(JsonRpcClientError::Read(io_context(error, &path))),
    }
}

fn client_credential_file_path(config: &ClientConfig, client_name: &str) -> PathBuf {
    client_credential_storage_dir(config).join(client_credential_file_name(client_name))
}

fn session_authority_file_path(
    config: &ClientConfig,
    client_name: &str,
    session_id: &SessionId,
) -> PathBuf {
    session_authority_storage_dir(config, client_name).join(session_authority_file_name(session_id))
}

fn client_credential_base_dir(config: &ClientConfig) -> PathBuf {
    if socket_name(config) == DAEMON_DEFAULT_SOCKET_NAME {
        return default_config_base_dir()
            .join("taugentic")
            .join("daemon-clients");
    }

    match &config.socket_address {
        SocketAddress::Unix(path) => path
            .parent()
            .map(|parent| parent.join("taugentic-client-credentials"))
            .unwrap_or_else(|| std::env::temp_dir().join("taugentic-client-credentials")),
        SocketAddress::NamedPipe(_) => std::env::temp_dir().join("taugentic-client-credentials"),
    }
}

fn client_credential_storage_dir(config: &ClientConfig) -> PathBuf {
    client_credential_base_dir(config).join(socket_name(config))
}

fn client_credential_file_name(client_name: &str) -> String {
    format!("{}.credential", client_storage_key(client_name))
}

fn session_authority_base_dir(config: &ClientConfig) -> PathBuf {
    if socket_name(config) == DAEMON_DEFAULT_SOCKET_NAME {
        return default_config_base_dir()
            .join("taugentic")
            .join("daemon-session-authorities");
    }

    match &config.socket_address {
        SocketAddress::Unix(path) => path
            .parent()
            .map(|parent| parent.join("taugentic-session-authorities"))
            .unwrap_or_else(|| std::env::temp_dir().join("taugentic-session-authorities")),
        SocketAddress::NamedPipe(_) => std::env::temp_dir().join("taugentic-session-authorities"),
    }
}

fn session_authority_socket_dir(config: &ClientConfig) -> PathBuf {
    session_authority_base_dir(config).join(socket_name(config))
}

fn session_authority_storage_dir(config: &ClientConfig, client_name: &str) -> PathBuf {
    session_authority_socket_dir(config).join(client_storage_key(client_name))
}

fn session_authority_file_name(session_id: &SessionId) -> String {
    format!("{}.authority", session_storage_key(session_id))
}

fn default_config_base_dir() -> PathBuf {
    match std::env::consts::OS {
        "macos" | "darwin" => normalized_env_path(std::env::var_os("HOME"))
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
            .unwrap_or_else(|| std::env::temp_dir().join("taugentic")),
        "windows" => normalized_env_path(std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .or_else(|| {
                normalized_env_path(std::env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData").join("Roaming"))
            })
            .unwrap_or_else(|| std::env::temp_dir().join("taugentic")),
        _ => normalized_env_path(std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)
            .or_else(|| {
                normalized_env_path(std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .map(|home| home.join(".config"))
            })
            .unwrap_or_else(|| std::env::temp_dir().join("taugentic")),
    }
}

fn normalized_env_path(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn socket_name(config: &ClientConfig) -> String {
    match &config.socket_address {
        SocketAddress::Unix(path) => path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| DAEMON_DEFAULT_SOCKET_NAME.to_string()),
        SocketAddress::NamedPipe(name) => name.clone(),
    }
}

fn client_storage_key(client_name: &str) -> String {
    stable_storage_key(client_name.trim())
}

fn session_storage_key(session_id: &SessionId) -> String {
    stable_storage_key(session_id.as_str())
}

fn stable_storage_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn parse_client_credential(credential: &str) -> Option<String> {
    let credential = credential.trim();
    if credential.len() < 32 || !credential.is_ascii() {
        return None;
    }
    if credential.chars().any(char::is_whitespace) {
        return None;
    }
    Some(credential.to_string())
}

fn load_persisted_secret<T>(path: &Path, parse: impl FnOnce(&str) -> Option<T>) -> Option<T> {
    let stored = fs::read_to_string(path).ok()?;
    let parsed = parse(&stored);
    if parsed.is_none() {
        let _ = fs::remove_file(path);
    }
    parsed
}

fn io_context(error: io::Error, path: &Path) -> io::Error {
    io::Error::new(
        error.kind(),
        format!(
            "failed to persist daemon client credential at {}: {error}",
            path.display()
        ),
    )
}

fn prepare_private_directory(path: &Path) -> Result<(), JsonRpcClientError> {
    fs::create_dir_all(path).map_err(|error| JsonRpcClientError::Write(io_context(error, path)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| JsonRpcClientError::Write(io_context(error, path)))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, contents: &str) -> Result<(), JsonRpcClientError> {
    let temporary_path = private_temporary_path(path);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut temporary = options
        .open(&temporary_path)
        .map_err(|error| JsonRpcClientError::Write(io_context(error, &temporary_path)))?;
    temporary
        .write_all(contents.as_bytes())
        .map_err(|error| JsonRpcClientError::Write(io_context(error, &temporary_path)))?;
    temporary
        .sync_all()
        .map_err(|error| JsonRpcClientError::Write(io_context(error, &temporary_path)))?;
    drop(temporary);
    fs::rename(&temporary_path, path)
        .map_err(|error| JsonRpcClientError::Write(io_context(error, path)))?;
    sync_parent_directory(path)?;
    Ok(())
}

fn private_temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("secret");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn sync_parent_directory(path: &Path) -> Result<(), JsonRpcClientError> {
    let parent = path.parent().ok_or_else(|| {
        JsonRpcClientError::Write(io_context(
            io::Error::other("secret path has no parent directory"),
            path,
        ))
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| JsonRpcClientError::Write(io_context(error, parent)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(socket_name: &str) -> ClientConfig {
        ClientConfig {
            service_name: "ta-daemon-client-tests".to_string(),
            socket_address: SocketAddress::Unix(
                std::env::temp_dir().join(format!("{socket_name}.sock")),
            ),
            io_timeout: std::time::Duration::from_secs(1),
        }
    }

    #[test]
    fn client_credential_path_hashes_client_name_for_path_safety() {
        let config = test_config("daemon-client-path-safety");
        let path = client_credential_file_path(&config, "../../evil/client");
        let expected_file_name = format!("{}.credential", client_storage_key("../../evil/client"));

        assert!(path.starts_with(std::env::temp_dir().join("taugentic-client-credentials")));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(expected_file_name.as_str()),
        );
        assert!(!path.to_string_lossy().contains("../../evil/client"));
    }

    #[test]
    fn session_authority_path_hashes_client_name_and_session_id_for_path_safety() {
        let config = test_config("daemon-session-authority-path-safety");
        let session_id = SessionId::new("../session/../../owned".to_string()).expect("session id");
        let path = session_authority_file_path(&config, "../client/../../owned", &session_id);
        let expected_file_name = format!("{}.authority", session_storage_key(&session_id));

        assert!(path.starts_with(std::env::temp_dir().join("taugentic-session-authorities")));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(expected_file_name.as_str()),
        );
        assert!(!path.to_string_lossy().contains("../client/../../owned"));
        assert!(!path.to_string_lossy().contains("../session/../../owned"));
    }

    #[test]
    fn load_client_credential_purges_invalid_persisted_file() {
        let config = test_config("daemon-client-invalid-credential");
        let path = client_credential_file_path(&config, "cli-client");
        prepare_private_directory(path.parent().expect("credential dir"))
            .expect("credential dir should exist");
        fs::write(&path, "short").expect("invalid credential should persist");

        assert_eq!(load_client_credential(&config, "cli-client"), None);
        assert!(!path.exists());
    }

    #[test]
    fn load_session_authority_purges_invalid_persisted_file() {
        let config = test_config("daemon-session-invalid-authority");
        let session_id = SessionId::new("session-123".to_string()).expect("session id");
        let path = session_authority_file_path(&config, "cli-client", &session_id);
        prepare_private_directory(path.parent().expect("authority dir"))
            .expect("authority dir should exist");
        fs::write(&path, "   ").expect("invalid authority should persist");

        assert_eq!(
            load_session_authority(&config, "cli-client", &session_id),
            None
        );
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn stored_client_credential_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("daemon-client-private-permissions");
        store_client_credential(&config, "cli-client", "credential-secret")
            .expect("credential should persist");
        let path = client_credential_file_path(&config, "cli-client");

        assert_eq!(
            fs::metadata(path.parent().expect("credential dir"))
                .expect("credential dir metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("credential file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn stored_session_authority_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let config = test_config("daemon-session-private-permissions");
        let session_id = SessionId::new("session-123".to_string()).expect("session id");
        let authority = SessionAuthority::new("session-authority-1session-authority-1".to_string())
            .expect("session authority");
        store_session_authority(&config, "cli-client", &session_id, &authority)
            .expect("session authority should persist");
        let path = session_authority_file_path(&config, "cli-client", &session_id);

        assert_eq!(
            fs::metadata(path.parent().expect("authority dir"))
                .expect("authority dir metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("authority file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn failed_pre_rename_write_preserves_existing_secret() {
        let config = test_config("daemon-client-atomic-preservation");
        let path = client_credential_file_path(&config, "cli-client");
        prepare_private_directory(path.parent().expect("credential dir"))
            .expect("credential dir should exist");
        fs::write(&path, "credential-beforecredential-beforecredential-before")
            .expect("old credential should exist");
        let temporary_path = private_temporary_path(&path);
        fs::write(&temporary_path, "block temporary creation").expect("temp blocker should exist");

        let error = write_private_file(&path, "credential-aftercredential-aftercredential-after")
            .expect_err("temporary collision should fail before rename");

        assert!(matches!(error, JsonRpcClientError::Write(_)));
        assert_eq!(
            fs::read_to_string(&path).expect("old credential should remain readable"),
            "credential-beforecredential-beforecredential-before"
        );
        fs::remove_file(temporary_path).expect("temporary blocker should clean up");
    }
}
