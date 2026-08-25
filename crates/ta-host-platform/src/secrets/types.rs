use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HostSecretKey(String);

impl HostSecretKey {
    pub fn new(value: impl Into<String>) -> Result<Self, HostSecretError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(HostSecretError::EmptyKey);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn account_name(&self, service: &str) -> String {
        format!("{service}/{}", self.0)
    }
}

impl fmt::Debug for HostSecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HostSecretKey")
            .field(&self.0)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HostSecretValue(String);

impl HostSecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, HostSecretError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(HostSecretError::EmptySecret);
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HostSecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostSecretValue(REDACTED)")
    }
}

use super::HostSecretError;
