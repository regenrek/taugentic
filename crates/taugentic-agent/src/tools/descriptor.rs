use serde_json::Value;
use ta_protocol::wire::ApprovalScope;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub approval_scope: Option<ApprovalScope>,
    pub read_only: bool,
    pub parallel_safe: bool,
}
