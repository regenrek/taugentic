use serde::{Deserialize, Serialize};
use ta_protocol::wire::{CodeHostAccount, CodeHostAccountId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeHostAccountProjection {
    pub owner_principal_id: String,
    pub account: CodeHostAccount,
}

impl CodeHostAccountProjection {
    pub fn id(&self) -> &CodeHostAccountId {
        &self.account.id
    }
}
