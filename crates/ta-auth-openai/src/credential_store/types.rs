use std::fmt;

use serde::{Deserialize, Serialize};
use ta_protocol::wire::AuthProfileId;

use crate::TokenSet;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialKey(AuthProfileId);

impl CredentialKey {
    pub fn new(id: AuthProfileId) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for CredentialKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CredentialKey")
            .field(&self.as_str())
            .finish()
    }
}

impl From<AuthProfileId> for CredentialKey {
    fn from(value: AuthProfileId) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_id: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_tier: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub token_set: TokenSet,
    pub account: AccountInfo,
    pub stored_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<u64>,
}

impl fmt::Debug for StoredCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredCredentials")
            .field("token_set", &self.token_set)
            .field("account", &self.account)
            .field("stored_at", &self.stored_at)
            .field("last_refreshed_at", &self.last_refreshed_at)
            .finish()
    }
}
