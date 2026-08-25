use std::{error::Error, fmt};

#[derive(Clone, PartialEq, Eq)]
pub enum HostSecretError {
    InvalidServiceName,
    EmptyKey,
    EmptySecret,
    BackendUnavailable {
        backend: &'static str,
        reason: String,
    },
    NotFound,
    EncryptFailed {
        backend: &'static str,
        reason: String,
    },
    DecryptFailed {
        backend: &'static str,
        reason: String,
    },
    IoError {
        operation: &'static str,
        reason: String,
    },
}

impl HostSecretError {
    pub(crate) fn backend_unavailable(backend: &'static str, reason: impl ToString) -> Self {
        Self::BackendUnavailable {
            backend,
            reason: reason.to_string(),
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(crate) fn encrypt_failed(backend: &'static str, reason: impl ToString) -> Self {
        Self::EncryptFailed {
            backend,
            reason: reason.to_string(),
        }
    }

    pub(crate) fn decrypt_failed(backend: &'static str, reason: impl ToString) -> Self {
        Self::DecryptFailed {
            backend,
            reason: reason.to_string(),
        }
    }

    pub(crate) fn io_error(operation: &'static str, reason: impl ToString) -> Self {
        Self::IoError {
            operation,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Debug for HostSecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidServiceName => formatter.write_str("InvalidServiceName"),
            Self::EmptyKey => formatter.write_str("EmptyKey"),
            Self::EmptySecret => formatter.write_str("EmptySecret"),
            Self::BackendUnavailable { backend, .. } => formatter
                .debug_struct("BackendUnavailable")
                .field("backend", backend)
                .field("reason", &"<redacted>")
                .finish(),
            Self::NotFound => formatter.write_str("NotFound"),
            Self::EncryptFailed { backend, .. } => formatter
                .debug_struct("EncryptFailed")
                .field("backend", backend)
                .field("reason", &"<redacted>")
                .finish(),
            Self::DecryptFailed { backend, .. } => formatter
                .debug_struct("DecryptFailed")
                .field("backend", backend)
                .field("reason", &"<redacted>")
                .finish(),
            Self::IoError { operation, .. } => formatter
                .debug_struct("IoError")
                .field("operation", operation)
                .field("reason", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for HostSecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidServiceName => formatter.write_str("host secret service name is empty"),
            Self::EmptyKey => formatter.write_str("host secret key is empty"),
            Self::EmptySecret => formatter.write_str("host secret is empty"),
            Self::BackendUnavailable { backend, .. } => {
                write!(formatter, "host secret backend unavailable: {backend}")
            }
            Self::NotFound => formatter.write_str("host secret not found"),
            Self::EncryptFailed { backend, .. } => {
                write!(formatter, "host secret encryption failed: {backend}")
            }
            Self::DecryptFailed { backend, .. } => {
                write!(formatter, "host secret decryption failed: {backend}")
            }
            Self::IoError { operation, .. } => {
                write!(formatter, "host secret store I/O failed during {operation}")
            }
        }
    }
}

impl Error for HostSecretError {}
