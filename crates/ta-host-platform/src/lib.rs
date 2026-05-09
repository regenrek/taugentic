mod detect;
mod paths;
mod platform;
mod secrets;
mod types;

pub use detect::*;
pub use paths::{canonical_realpath, taugentic_user_recipe_dir, taugentic_workflow_file_path};
#[cfg(target_os = "linux")]
pub use platform::is_safe_bwrap_binary;
#[cfg(windows)]
pub use platform::is_safe_windows_sandbox_helper;
#[cfg(target_os = "linux")]
pub use platform::linux_sandbox_helper_path;
#[cfg(windows)]
pub use platform::windows_sandbox_helper_path;
pub use platform::{current_capabilities, sandbox_capabilities, secrets_backend_capability};
pub use secrets::{
    HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue, default_host_secret_store,
};
pub use types::*;
