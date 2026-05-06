use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostSecretKey {
    namespace: &'static str,
    name: &'static str,
}

impl HostSecretKey {
    pub const WORK_SOURCE_GITHUB_PAT: Self = Self {
        namespace: "work_source.github",
        name: "github_pat",
    };

    pub const fn namespace(&self) -> &'static str {
        self.namespace
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn account_name(&self, service: &str) -> String {
        format!("{service}/{}/{}", self.namespace, self.name)
    }
}

impl fmt::Debug for HostSecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostSecretKey")
            .field("namespace", &self.namespace)
            .field("name", &self.name)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HostSecretValue(String);

impl HostSecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, HostSecretError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(HostSecretError::EmptySecret {
                key: HostSecretKey::WORK_SOURCE_GITHUB_PAT,
            });
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
