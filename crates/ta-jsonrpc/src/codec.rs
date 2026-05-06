use thiserror::Error;

use crate::JsonRpcMessage;

#[derive(Debug, Error)]
pub enum JsonLineCodecError {
    #[error("failed to encode or decode JSON-RPC message: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonLineCodec;

impl JsonLineCodec {
    pub fn encode_message(&self, message: &JsonRpcMessage) -> Result<String, JsonLineCodecError> {
        let mut line = serde_json::to_string(message)?;
        line.push('\n');
        Ok(line)
    }

    pub fn decode_message(&self, line: &str) -> Result<JsonRpcMessage, JsonLineCodecError> {
        Ok(serde_json::from_str::<JsonRpcMessage>(line.trim_end())?)
    }
}
