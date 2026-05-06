use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AcpClientError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("process failed: {0}")]
    ProcessFailed(String),
    #[error("process failed: ACP request {request_id} failed: {detail}")]
    JsonRpcRequestFailed {
        request_id: u64,
        code: Option<i64>,
        detail: String,
    },
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("ACP JSON-RPC error {code}: {message}")]
    JsonRpc { code: i64, message: String },
}

impl AcpClientError {
    pub fn is_method_not_found(&self) -> bool {
        matches!(
            self,
            Self::JsonRpcRequestFailed {
                code: Some(-32601),
                ..
            }
        )
    }
}
