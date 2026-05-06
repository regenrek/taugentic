#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use ta_host_platform::SandboxKind;

use crate::SandboxBackend;

mod unsupported;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use unsupported::UnsupportedSandboxBackend;

pub fn current_backend() -> Box<dyn SandboxBackend> {
    platform_backend()
}

#[cfg(target_os = "macos")]
fn platform_backend() -> Box<dyn SandboxBackend> {
    Box::new(macos::SeatbeltBackend)
}

#[cfg(target_os = "linux")]
fn platform_backend() -> Box<dyn SandboxBackend> {
    Box::new(linux::LandlockBackend)
}

#[cfg(windows)]
fn platform_backend() -> Box<dyn SandboxBackend> {
    Box::new(windows::AppContainerBackend)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn platform_backend() -> Box<dyn SandboxBackend> {
    Box::new(UnsupportedSandboxBackend::new(
        SandboxKind::Unsupported,
        "no sandbox backend is registered for this target",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SandboxCommand, SandboxError, SandboxKind, SandboxProfile};

    #[test]
    fn unsupported_backend_returns_explicit_error() {
        let backend = UnsupportedSandboxBackend::new(SandboxKind::Unsupported, "not available");
        let profile = SandboxProfile::default();
        let command = SandboxCommand::new("echo", vec!["ok".into()]);

        let error = backend.prepare(&profile, command).expect_err("unsupported");

        assert_eq!(
            error,
            SandboxError::Unsupported {
                kind: SandboxKind::Unsupported,
                reason: "not available",
            }
        );
    }
}
