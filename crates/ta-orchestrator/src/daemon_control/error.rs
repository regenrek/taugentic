use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonControlConfigError {
    #[error("invalid daemon runtime mode in {env_var}; expected local or background")]
    InvalidRuntimeMode {
        env_var: &'static str,
        value: String,
    },
    #[error("invalid daemon runtime mode in persisted config {path}; expected local or background")]
    InvalidRuntimeModeFile { path: PathBuf, value: String },
    #[error("failed to read persisted daemon runtime mode from {path}")]
    ReadRuntimeModeFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Error)]
pub enum BackgroundServiceControlError {
    #[error(transparent)]
    RuntimeMode(#[from] DaemonControlConfigError),
    #[error("background service mode is not supported on this platform")]
    UnsupportedPlatform,
    #[error("failed to resolve user home directory for background service control")]
    MissingHomeDirectory,
    #[error("failed to determine daemon log directory for background service control")]
    MissingLogDirectory,
    #[error("failed to create background service directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read background service file {path}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write background service file {path}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove background service file {path}")]
    RemoveFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse runtime control ownership file {path}")]
    ParseOwnershipFile {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse runtime control state file {path}")]
    ParseControlPlaneFile {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize runtime control state file {path}")]
    SerializeControlPlaneFile {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("timed out waiting for runtime control mutation lock {path}")]
    MutationLockTimeout { path: PathBuf },
    #[error("{command} failed: {detail}")]
    CommandFailed {
        command: &'static str,
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use super::DaemonControlConfigError;
    use std::path::PathBuf;

    #[test]
    fn invalid_runtime_mode_file_display_does_not_echo_raw_value() {
        let error = DaemonControlConfigError::InvalidRuntimeModeFile {
            path: PathBuf::from("/tmp/runtime-control.json"),
            value: "raw-secret-token".to_string(),
        };
        let rendered = error.to_string();

        assert!(rendered.contains("invalid daemon runtime mode in persisted config"));
        assert!(rendered.contains("/tmp/runtime-control.json"));
        assert!(rendered.contains("expected local or background"));
        assert!(!rendered.contains("raw-secret-token"));
    }
}
