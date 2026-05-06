use std::{error::Error, fmt};

#[derive(Clone, PartialEq, Eq)]
pub enum CredentialStoreError {
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
    Serialization {
        operation: &'static str,
        reason: String,
    },
}

impl CredentialStoreError {
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

    pub(crate) fn serialization(operation: &'static str, reason: impl ToString) -> Self {
        Self::Serialization {
            operation,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Debug for CredentialStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Serialization { operation, .. } => formatter
                .debug_struct("Serialization")
                .field("operation", operation)
                .field("reason", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for CredentialStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable { backend, .. } => {
                write!(formatter, "credential backend unavailable: {backend}")
            }
            Self::NotFound => formatter.write_str("credential not found"),
            Self::EncryptFailed { backend, .. } => {
                write!(formatter, "credential encryption failed: {backend}")
            }
            Self::DecryptFailed { backend, .. } => {
                write!(formatter, "credential decryption failed: {backend}")
            }
            Self::IoError { operation, .. } => {
                write!(formatter, "credential store I/O failed during {operation}")
            }
            Self::Serialization { operation, .. } => {
                write!(
                    formatter,
                    "credential serialization failed during {operation}"
                )
            }
        }
    }
}

impl Error for CredentialStoreError {}
