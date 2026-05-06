use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::error::AcpClientError;

pub fn resolve(binary_name: &str, env_override_var: &str) -> Result<PathBuf, AcpClientError> {
    resolve_with_env(
        binary_name,
        env_override_var,
        env::var_os(env_override_var),
        search_path_dirs(),
    )
}

fn resolve_with_env(
    binary_name: &str,
    env_override_var: &str,
    env_override: Option<OsString>,
    dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, AcpClientError> {
    if let Some(path) = env_override.and_then(non_empty_os_string) {
        let candidate = PathBuf::from(path);
        if is_executable(&candidate) {
            return Ok(candidate);
        }
        return Err(AcpClientError::InvalidConfig(format!(
            "ACP binary override {env_override_var} points to non-executable path {}",
            candidate.display()
        )));
    }
    resolve_in_dirs(binary_name, dirs)
}

fn non_empty_os_string(value: OsString) -> Option<OsString> {
    if value.to_string_lossy().trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn resolve_in_dirs(
    binary_name: &str,
    dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, AcpClientError> {
    if binary_name.trim().is_empty() {
        return Err(AcpClientError::InvalidConfig(
            "ACP binary name must not be empty".to_string(),
        ));
    }

    dirs.into_iter()
        .map(|dir| dir.join(binary_name))
        .into_iter()
        .find(|candidate| is_executable(candidate))
        .ok_or_else(|| {
            AcpClientError::InvalidConfig(format!(
                "could not resolve ACP binary '{binary_name}' in PATH, npm global bin, ~/.cursor/bin, /opt/homebrew/bin, or /usr/local/bin"
            ))
        })
}

pub fn candidate_binary_paths(command: &str) -> Vec<PathBuf> {
    search_path_dirs()
        .into_iter()
        .map(|dir| dir.join(command))
        .collect()
}

fn search_path_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&paths));
    }

    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".npm-global/bin"));
        dirs.push(PathBuf::from(&home).join(".local/bin"));
        dirs.push(PathBuf::from(&home).join(".cursor/bin"));
    }

    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn is_executable(path: &Path) -> bool {
    path.is_file() && executable_mode(path)
}

#[cfg(unix)]
fn executable_mode(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn executable_mode(path: &Path) -> bool {
    path.exists()
}

pub fn joined_path_with_extra(
    extra: impl IntoIterator<Item = PathBuf>,
) -> Result<OsString, AcpClientError> {
    env::join_paths(extra.into_iter().chain(search_path_dirs())).map_err(|error| {
        AcpClientError::InvalidConfig(format!("failed to build ACP search PATH: {error}"))
    })
}

pub fn path_contains(path: &OsStr, needle: &Path) -> bool {
    env::split_paths(path).any(|candidate| candidate == needle)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn resolve_finds_binary_in_path_tempdir() {
        let dir = unique_dir("search-path");
        fs::create_dir_all(&dir).expect("temp dir");
        let binary = dir.join("fake-acp");
        fs::write(&binary, "#!/bin/sh\nexit 0\n").expect("fake binary");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).expect("chmod");

        let resolved = resolve_in_dirs("fake-acp", [dir.clone()]).expect("binary should resolve");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(resolved, binary);
    }

    #[test]
    fn resolve_prefers_env_override() {
        let dir = unique_dir("search-path-override");
        fs::create_dir_all(&dir).expect("temp dir");
        let override_binary = dir.join("override-acp");
        let path_binary = dir.join("path-acp");
        fs::write(&override_binary, "#!/bin/sh\nexit 0\n").expect("override binary");
        fs::write(&path_binary, "#!/bin/sh\nexit 0\n").expect("path binary");
        fs::set_permissions(&override_binary, fs::Permissions::from_mode(0o755))
            .expect("override chmod");
        fs::set_permissions(&path_binary, fs::Permissions::from_mode(0o755)).expect("path chmod");

        let resolved = resolve_with_env(
            "path-acp",
            "TAUGENTIC_TEST_ACP_BIN",
            Some(override_binary.clone().into_os_string()),
            [dir.clone()],
        )
        .expect("override should resolve");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(resolved, override_binary);
    }

    #[test]
    fn resolve_rejects_non_executable_env_override() {
        let dir = unique_dir("search-path-bad-override");
        fs::create_dir_all(&dir).expect("temp dir");
        let override_binary = dir.join("override-acp");
        fs::write(&override_binary, "#!/bin/sh\nexit 0\n").expect("override binary");

        let error = resolve_with_env(
            "missing-acp",
            "TAUGENTIC_TEST_ACP_BIN",
            Some(override_binary.clone().into_os_string()),
            [dir.clone()],
        )
        .expect_err("bad override should fail fast");
        let _ = fs::remove_dir_all(&dir);

        assert!(
            matches!(error, AcpClientError::InvalidConfig(message) if message.contains("TAUGENTIC_TEST_ACP_BIN"))
        );
    }

    fn unique_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-artifacts/ta-provider-acp")
            .join(format!("{prefix}-{nanos}"))
    }
}
