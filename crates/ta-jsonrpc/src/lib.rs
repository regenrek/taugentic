pub mod client;
pub mod codec;
pub mod runtime;
pub mod server;
pub mod socket;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub use client::*;
pub use codec::*;
pub use runtime::{
    DEFAULT_OUTBOUND_QUEUE_DEPTH, DEFAULT_PERSISTENT_CONNECTION_POLL_INTERVAL,
    JsonRpcConnectionAdapter, JsonRpcConnectionLoopEvent, JsonRpcConnectionLoopOutcome,
    JsonRpcConnectionRuntime, JsonRpcHandlerFuture, JsonRpcHandlerResult, JsonRpcRequestHandler,
    JsonRpcRequestProcessingContext, JsonRpcServerSession, JsonRpcSessionError, OutboundQueueError,
    PersistentPollController, ProcessedJsonRpcMessage, enqueue_outbound_message,
    process_jsonrpc_request, run_jsonrpc_connection_loop, should_enter_persistent_poll_mode,
};
pub use server::*;
pub use socket::*;

pub const JSONRPC_VERSION: &str = "2.0";

pub const PARSE_ERROR_CODE: i64 = -32_700;
pub const INVALID_REQUEST_ERROR_CODE: i64 = -32_600;
pub const METHOD_NOT_FOUND_ERROR_CODE: i64 = -32_601;
pub const INVALID_PARAMS_ERROR_CODE: i64 = -32_602;
pub const INTERNAL_ERROR_CODE: i64 = -32_603;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => formatter.write_str(value),
            Self::Integer(value) => value.fmt(formatter),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
    Error(JsonRpcError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest {
    #[serde(
        serialize_with = "serialize_jsonrpc_version",
        deserialize_with = "deserialize_jsonrpc_version"
    )]
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcNotification {
    #[serde(
        serialize_with = "serialize_jsonrpc_version",
        deserialize_with = "deserialize_jsonrpc_version"
    )]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcResponse {
    #[serde(
        serialize_with = "serialize_jsonrpc_version",
        deserialize_with = "deserialize_jsonrpc_version"
    )]
    pub jsonrpc: String,
    pub id: RequestId,
    pub result: Value,
}

impl JsonRpcResponse {
    pub fn new(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcError {
    #[serde(
        serialize_with = "serialize_jsonrpc_version",
        deserialize_with = "deserialize_jsonrpc_version"
    )]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    pub error: JsonRpcErrorObject,
}

impl JsonRpcError {
    pub fn new(id: Option<RequestId>, error: JsonRpcErrorObject) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcErrorObject {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(PARSE_ERROR_CODE, message)
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(INVALID_REQUEST_ERROR_CODE, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            METHOD_NOT_FOUND_ERROR_CODE,
            format!("method not found: {method}"),
        )
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS_ERROR_CODE, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR_CODE, message)
    }
}

fn serialize_jsonrpc_version<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value != JSONRPC_VERSION {
        return Err(serde::ser::Error::custom(format!(
            "jsonrpc must be {JSONRPC_VERSION}, got {value}"
        )));
    }

    serializer.serialize_str(value)
}

fn deserialize_jsonrpc_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value != JSONRPC_VERSION {
        return Err(serde::de::Error::custom(format!(
            "jsonrpc must be {JSONRPC_VERSION}, got {value}"
        )));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        INVALID_REQUEST_ERROR_CODE, JSONRPC_VERSION, JsonRpcError, JsonRpcErrorObject,
        JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId,
    };

    #[test]
    fn rejects_non_2_0_jsonrpc_version() {
        let error = serde_json::from_value::<JsonRpcMessage>(json!({
            "jsonrpc": "1.0",
            "id": 1,
            "method": "daemon.status",
            "params": {}
        }))
        .expect_err("non-2.0 jsonrpc version should fail");

        assert!(
            error.to_string().contains("did not match any variant"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_hybrid_response_error_message() {
        let error = serde_json::from_value::<JsonRpcMessage>(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "ok": true },
            "error": {
                "code": INVALID_REQUEST_ERROR_CODE,
                "message": "bad request"
            }
        }))
        .expect_err("hybrid response/error payload should fail");

        assert!(
            error.to_string().contains("did not match any variant"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_unknown_request_fields() {
        let error = serde_json::from_value::<JsonRpcMessage>(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "daemon.status",
            "params": {},
            "extra": true
        }))
        .expect_err("request with unknown fields should fail");

        assert!(
            error.to_string().contains("did not match any variant"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn constructors_still_emit_valid_messages() {
        let request = JsonRpcRequest::new(RequestId::Integer(1), "daemon.status", Some(json!({})));
        let response = JsonRpcResponse::new(RequestId::Integer(1), json!({ "ok": true }));
        let error = JsonRpcError::new(
            Some(RequestId::Integer(1)),
            JsonRpcErrorObject::new(INVALID_REQUEST_ERROR_CODE, "bad request"),
        );

        let request_json = serde_json::to_value(JsonRpcMessage::Request(request))
            .expect("request should serialize");
        let response_json = serde_json::to_value(JsonRpcMessage::Response(response))
            .expect("response should serialize");
        let error_json =
            serde_json::to_value(JsonRpcMessage::Error(error)).expect("error should serialize");

        assert_eq!(request_json["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(response_json["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(error_json["jsonrpc"], JSONRPC_VERSION);
    }
}
