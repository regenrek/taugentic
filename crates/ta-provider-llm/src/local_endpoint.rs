use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::client::openai_compatible::OpenAiCompatibleAuth;
use crate::client::{require_stream_response, send_request};
use crate::error::LlmClientError;
use crate::http::shared_client;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpointProbeConfig {
    pub base_url: String,
    pub auth: Option<OpenAiCompatibleAuth>,
    pub model: Option<String>,
    pub model_discovery: bool,
    pub test_tool_call: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpointProbeResult {
    pub models: Vec<LocalEndpointModel>,
    pub tools_supported: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpointModel {
    pub id: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelRecord>,
}

#[derive(Debug, Deserialize)]
struct ModelRecord {
    id: String,
}

pub async fn probe_openai_compatible_endpoint(
    config: LocalEndpointProbeConfig,
    cancellation: CancellationToken,
) -> Result<LocalEndpointProbeResult, LlmClientError> {
    let base_url = normalize_base_url(&config.base_url)?;
    let models = if config.model_discovery {
        probe_models(&base_url, config.auth.as_ref(), &cancellation).await?
    } else {
        Vec::new()
    };
    let tools_supported = if config.test_tool_call {
        let model = config
            .model
            .as_deref()
            .or_else(|| models.first().map(|model| model.id.as_str()))
            .ok_or_else(|| {
                LlmClientError::InvalidConfig(
                    "local endpoint tool probe requires a model".to_string(),
                )
            })?;
        Some(probe_tool_call(&base_url, config.auth.as_ref(), model, &cancellation).await?)
    } else if config.model_discovery
        || config
            .model
            .as_deref()
            .is_some_and(|model| !model.trim().is_empty())
    {
        None
    } else {
        return Err(LlmClientError::InvalidConfig(
            "local endpoint probe requires modelDiscovery or a model".to_string(),
        ));
    };
    Ok(LocalEndpointProbeResult {
        models,
        tools_supported,
    })
}

async fn probe_models(
    base_url: &str,
    auth: Option<&OpenAiCompatibleAuth>,
    cancellation: &CancellationToken,
) -> Result<Vec<LocalEndpointModel>, LlmClientError> {
    let builder = apply_auth(shared_client().get(format!("{base_url}/models")), auth);
    let response = send_request(builder, cancellation).await?;
    let payload: ModelsResponse = read_json_response(response, cancellation).await?;
    Ok(payload
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| LocalEndpointModel { id: model.id })
        .collect())
}

async fn probe_tool_call(
    base_url: &str,
    auth: Option<&OpenAiCompatibleAuth>,
    model: &str,
    cancellation: &CancellationToken,
) -> Result<bool, LlmClientError> {
    let body = json!({
        "model": model,
        "stream": false,
        "messages": [{"role": "user", "content": "Call the available test tool."}],
        "tool_choice": "required",
        "tools": [{
            "type": "function",
            "function": {
                "name": "taugentic_test_tool",
                "description": "A connection-test tool.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "ok": {"type": "boolean"}
                    },
                    "required": ["ok"],
                    "additionalProperties": false
                }
            }
        }]
    });
    let builder = apply_auth(
        shared_client()
            .post(format!("{base_url}/chat/completions"))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(
                serde_json::to_vec(&body)
                    .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))?,
            ),
        auth,
    );
    let response = send_request(builder, cancellation).await?;
    let payload: serde_json::Value = read_json_response(response, cancellation).await?;
    Ok(payload
        .pointer("/choices/0/message/tool_calls")
        .and_then(|value| value.as_array())
        .is_some_and(|tool_calls| !tool_calls.is_empty()))
}

async fn read_json_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    cancellation: &CancellationToken,
) -> Result<T, LlmClientError> {
    let response = require_stream_response(response, cancellation).await?;
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(LlmClientError::Cancelled("local endpoint probe cancelled while reading response".to_string())),
        payload = response.text() => {
            let payload = payload.map_err(|error| LlmClientError::Network(error.to_string()))?;
            serde_json::from_str::<T>(&payload)
                .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))
        },
    }
}

fn apply_auth(
    mut builder: reqwest::RequestBuilder,
    auth: Option<&OpenAiCompatibleAuth>,
) -> reqwest::RequestBuilder {
    if let Some(auth) = auth {
        match auth {
            OpenAiCompatibleAuth::BearerEnv(var) => {
                if let Ok(token) = std::env::var(var) {
                    builder = builder.bearer_auth(token);
                }
            }
            OpenAiCompatibleAuth::BearerStatic(token) => {
                builder = builder.bearer_auth(token.as_ref());
            }
        }
    }
    builder
}

fn normalize_base_url(base_url: &str) -> Result<String, LlmClientError> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(LlmClientError::InvalidConfig(
            "local endpoint base URL is empty".to_string(),
        ));
    }
    Ok(base_url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manual_model_probe_does_not_require_model_discovery() {
        let result = probe_openai_compatible_endpoint(
            LocalEndpointProbeConfig {
                base_url: "http://127.0.0.1:9/v1".to_string(),
                auth: None,
                model: Some("manual-model".to_string()),
                model_discovery: false,
                test_tool_call: false,
            },
            CancellationToken::new(),
        )
        .await
        .expect("manual model should not call /models");

        assert!(result.models.is_empty());
        assert_eq!(result.tools_supported, None);
    }

    #[tokio::test]
    async fn probe_requires_model_when_discovery_is_disabled() {
        let error = probe_openai_compatible_endpoint(
            LocalEndpointProbeConfig {
                base_url: "http://127.0.0.1:9/v1".to_string(),
                auth: None,
                model: None,
                model_discovery: false,
                test_tool_call: false,
            },
            CancellationToken::new(),
        )
        .await
        .expect_err("missing model should be rejected");

        assert!(
            matches!(error, LlmClientError::InvalidConfig(message) if message.contains("modelDiscovery"))
        );
    }
}
