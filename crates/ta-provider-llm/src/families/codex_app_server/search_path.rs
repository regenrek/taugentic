use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::CodexLlmClientError;

pub const TAUGENTIC_CODEX_APP_SERVER_BIN_ENV: &str = "TAUGENTIC_CODEX_APP_SERVER_BIN";
pub const CODEX_BINARY_NAME: &str = "codex";

pub fn default_binary() -> PathBuf {
    env::var_os(TAUGENTIC_CODEX_APP_SERVER_BIN_ENV)
        .and_then(non_empty_os_string)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CODEX_BINARY_NAME))
}

pub fn resolve_codex_binary(binary: &Path) -> Result<PathBuf, CodexLlmClientError> {
    if binary.components().count() > 1 {
        return executable_path(binary).ok_or_else(|| {
            CodexLlmClientError::CliUnavailable(format!(
                "codex binary not found or not executable at {}",
                binary.display()
            ))
        });
    }
    search_path_dirs()
        .into_iter()
        .map(|dir| dir.join(binary))
        .find_map(|candidate| executable_path(&candidate))
        .ok_or_else(|| {
            CodexLlmClientError::CliUnavailable(
                "codex binary not found, install via npm install -g @openai/codex".to_string(),
            )
        })
}

fn search_path_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&paths));
    }
    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".npm-global/bin"));
        dirs.push(PathBuf::from(&home).join(".local/bin"));
        dirs.push(PathBuf::from(&home).join(".codex/bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn executable_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() && executable_mode(path) {
        Some(path.to_path_buf())
    } else {
        None
    }
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

fn non_empty_os_string(value: OsString) -> Option<OsString> {
    if value.to_string_lossy().trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
