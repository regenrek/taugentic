use serde::{Deserialize, Serialize};
use ta_protocol::wire::ApprovalScope;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub scope: ApprovalScope,
    pub label: String,
}

impl Operation {
    pub fn new(scope: ApprovalScope, label: impl Into<String>) -> Self {
        Self {
            scope,
            label: label.into(),
        }
    }
}
