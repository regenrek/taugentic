use crate::{HostCapabilities, SandboxCapabilities, SecretsBackend};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as imp;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(windows)]
use windows as imp;

pub fn current_capabilities() -> HostCapabilities {
    imp::current_capabilities()
}

pub fn secrets_backend_capability() -> SecretsBackend {
    imp::secrets_backend_capability()
}

pub fn sandbox_capabilities() -> SandboxCapabilities {
    imp::sandbox_capabilities()
}

#[cfg(target_os = "linux")]
pub fn linux_sandbox_helper_path() -> Option<std::path::PathBuf> {
    linux::linux_sandbox_helper_path()
}

#[cfg(windows)]
pub fn windows_sandbox_helper_path() -> Option<std::path::PathBuf> {
    windows::windows_sandbox_helper_path()
}

#[cfg(windows)]
pub fn is_safe_windows_sandbox_helper(path: &std::path::Path) -> bool {
    windows::is_safe_windows_sandbox_helper(path)
}

#[cfg(target_os = "linux")]
pub fn is_safe_bwrap_binary(path: &std::path::Path) -> bool {
    linux::is_safe_bwrap_binary(path)
}
