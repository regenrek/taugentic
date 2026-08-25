use async_trait::async_trait;
use serde_json::json;
use ta_protocol::wire::AuthProfileId;
use tokio_util::sync::CancellationToken;

use super::{
    EventParser, HttpLlmStream, LlmClient, LlmStream, StreamEvent, StreamMessage, StreamRequest,
    StreamRole, StreamTool, map_provider_error, map_provider_events_with_metadata,
    require_stream_response, send_request,
};
use crate::auth::openai::OpenAiAuthRoute;
use crate::auth::openai::{self, OpenAiAuth};
use crate::error::LlmClientError;
use crate::families::openai::OPENAI_API_KEY_ENV_VAR;
use crate::formats::openai_responses;
use crate::http::shared_client;
use tracing::instrument;
use uuid::Uuid;

const OPENAI_ORGANIZATION_HEADER: &str = "OpenAI-Organization";
const OPENAI_CHATGPT_ACCOUNT_HEADER: &str = "ChatGPT-Account-ID";
const OPENAI_RESPONSES_ORIGINATOR_HEADER: &str = "originator";
const OPENAI_RESPONSES_ORIGINATOR: &str = "taugentic_rs";
const OPENAI_CHATGPT_RESPONSES_ORIGINATOR: &str = "codex_cli_rs";
const OPENAI_RESPONSES_SESSION_HEADER: &str = "session_id";

#[derive(Clone)]
pub struct OpenAiResponsesClient {
    http: reqwest::Client,
    base_url_override: Option<String>,
    auth: OpenAiAuth,
    model: String,
    session_id: String,
}

impl OpenAiResponsesClient {
    pub fn with_auth_base_url_override_for_test(
        base_url: impl Into<String>,
        auth: OpenAiAuth,
        model: impl Into<String>,
    ) -> Result<Self, LlmClientError> {
        Self::build(Some(base_url), auth, model)
    }

    fn build(
        base_url_override: Option<impl Into<String>>,
        auth: OpenAiAuth,
        model: impl Into<String>,
    ) -> Result<Self, LlmClientError> {
        let model = normalized_model(model.into())?;
        Ok(Self {
            http: shared_client(),
            base_url_override: base_url_override.map(|base_url| trim_base_url(base_url.into())),
            auth,
            model,
            session_id: Uuid::new_v4().to_string(),
        })
    }

    pub fn from_env(model: impl Into<String>) -> Result<Self, LlmClientError> {
        let api_key = std::env::var(OPENAI_API_KEY_ENV_VAR).map_err(|_| {
            LlmClientError::CredentialsMissing(openai::openai_api_key_profile_error("is not set"))
        })?;
        if api_key.trim().is_empty() {
            return Err(LlmClientError::CredentialsMissing(
                openai::openai_api_key_profile_error("is empty"),
            ));
        }
        Self::from_auth(OpenAiAuth::ApiKey { key: api_key }, model)
    }

    pub fn from_auth_profile(
        model: impl Into<String>,
        auth_profile_id: Option<&AuthProfileId>,
    ) -> Result<Self, LlmClientError> {
        Self::from_auth(openai::auth_for_profile(auth_profile_id)?, model)
    }

    fn from_auth(auth: OpenAiAuth, model: impl Into<String>) -> Result<Self, LlmClientError> {
        Self::build(None::<String>, auth, model)
    }

    async fn send(
        &self,
        request: StreamRequest,
        cancellation: &CancellationToken,
    ) -> Result<reqwest::Response, LlmClientError> {
        let route = self.route_for_request(self.auth.route().await?);
        let body = self.body_for_route(&request, &route)?;
        let response = self.send_once(body, route, cancellation).await?;
        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            return require_stream_response(response, cancellation).await;
        }
        let Some(refreshed_route) = self.auth.force_refresh_route().await? else {
            return require_stream_response(response, cancellation).await;
        };
        let refreshed_route = self.route_for_request(refreshed_route);
        let body = self.body_for_route(&request, &refreshed_route)?;
        let retry_response = self.send_once(body, refreshed_route, cancellation).await?;
        require_stream_response(retry_response, cancellation).await
    }

    fn body_for_route(
        &self,
        request: &StreamRequest,
        route: &OpenAiAuthRoute,
    ) -> Result<Vec<u8>, LlmClientError> {
        let chatgpt_backend = route.chatgpt_account_id().is_some();
        if chatgpt_backend && response_instructions(request).is_empty() {
            return Err(LlmClientError::InvalidConfig(
                "OpenAI ChatGPT Responses route requires a system prompt for instructions"
                    .to_string(),
            ));
        }
        serde_json::to_vec(&responses_body(
            request,
            &self.model,
            chatgpt_backend,
            &self.session_id,
        ))
        .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))
    }

    fn route_for_request(&self, route: OpenAiAuthRoute) -> OpenAiAuthRoute {
        match &self.base_url_override {
            Some(base_url) => route.with_base_url(base_url.clone()),
            None => route,
        }
    }

    async fn send_once(
        &self,
        body: Vec<u8>,
        route: OpenAiAuthRoute,
        cancellation: &CancellationToken,
    ) -> Result<reqwest::Response, LlmClientError> {
        tracing::debug!(
            openai.route = route.label_for_logs(),
            "sending OpenAI Responses request"
        );
        let mut request = self
            .http
            .post(format!("{}/responses", route.base_url()))
            .bearer_auth(route.bearer_token())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                OPENAI_RESPONSES_ORIGINATOR_HEADER,
                if route.chatgpt_account_id().is_some() {
                    OPENAI_CHATGPT_RESPONSES_ORIGINATOR
                } else {
                    OPENAI_RESPONSES_ORIGINATOR
                },
            )
            .header(OPENAI_RESPONSES_SESSION_HEADER, &self.session_id)
            .body(body);
        if let Some(organization_id) = route
            .organization_id()
            .filter(|organization_id| !organization_id.trim().is_empty())
        {
            request = request.header(OPENAI_ORGANIZATION_HEADER, organization_id);
        }
        if let Some(account_id) = route
            .chatgpt_account_id()
            .filter(|account_id| !account_id.trim().is_empty())
        {
            request = request.header(OPENAI_CHATGPT_ACCOUNT_HEADER, account_id);
        }
        send_request(request, cancellation).await
    }
}

#[async_trait]
impl LlmClient for OpenAiResponsesClient {
    #[instrument(level = "debug", skip_all, fields(provider = "openai_responses"))]
    async fn start_stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<Box<dyn LlmStream>, LlmClientError> {
        if cancellation.is_cancelled() {
            return Err(LlmClientError::Cancelled(
                "stream cancelled before request".to_string(),
            ));
        }
        let response = self.send(request, &cancellation).await?;
        Ok(Box::new(HttpLlmStream::new_requiring_turn_completed(
            response,
            parse_event as EventParser,
            cancellation,
        )))
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }
}

fn responses_body(
    request: &StreamRequest,
    model: &str,
    chatgpt_backend: bool,
    session_id: &str,
) -> serde_json::Value {
    let mut body = json!({
        "model": model,
        "input": request
            .messages
            .iter()
            .filter(|message| !chatgpt_backend || !matches!(message.role, StreamRole::System))
            .map(response_message)
            .collect::<Vec<_>>(),
        "stream": true,
    });
    if chatgpt_backend {
        let instructions = response_instructions(request);
        if !instructions.is_empty() {
            body["instructions"] = json!(instructions);
        }
        body["tools"] = json!(request.tools.iter().map(response_tool).collect::<Vec<_>>());
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(true);
        body["reasoning"] = serde_json::Value::Null;
        body["store"] = json!(false);
        body["include"] = json!([]);
        body["prompt_cache_key"] = json!(session_id);
    } else if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(response_tool).collect::<Vec<_>>());
    }
    if let Some(id) = request
        .provider_session_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .filter(|_| !chatgpt_backend)
    {
        body["previous_response_id"] = json!(id);
    }
    body
}

fn response_instructions(request: &StreamRequest) -> String {
    request
        .messages
        .iter()
        .filter(|message| matches!(message.role, StreamRole::System))
        .map(|message| message.content.as_str())
        .filter(|content| !content.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn response_message(message: &StreamMessage) -> serde_json::Value {
    match message.role {
        StreamRole::Tool => json!({
            "role": "user",
            "content": [{"type": "input_text", "text": format!("tool result {}: {}", message.tool_call_id.as_deref().unwrap_or("unknown"), message.content)}],
        }),
        StreamRole::Assistant => json!({
            "role": "assistant",
            "content": [{"type": "output_text", "text": message.content}],
        }),
        StreamRole::System | StreamRole::User => json!({
            "role": role_name(&message.role),
            "content": [{"type": "input_text", "text": message.content}],
        }),
    }
}

fn response_tool(tool: &StreamTool) -> serde_json::Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.input_schema,
    })
}

fn parse_event(data: &str) -> Result<Vec<StreamEvent>, LlmClientError> {
    openai_responses::stream_events(data)
        .map(|events| map_provider_events_with_metadata(events, "openai"))
        .map_err(map_provider_error)
}

fn role_name(role: &StreamRole) -> &'static str {
    match role {
        StreamRole::System => "system",
        StreamRole::User => "user",
        StreamRole::Assistant => "assistant",
        StreamRole::Tool => "user",
    }
}

fn trim_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn normalized_model(model: String) -> Result<String, LlmClientError> {
    if model.trim().is_empty() {
        return Err(LlmClientError::InvalidConfig(
            "OpenAI Responses model is empty".to_string(),
        ));
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ta_auth_openai::client::{FormField, OAuthHttpClient, OAuthHttpFuture, OAuthHttpResponse};
    use ta_auth_openai::{
        AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, OAuthConfig,
        RefreshPolicy, StoredCredentials, TokenManager, TokenSet,
    };
    use ta_protocol::wire::AuthProfileId;
    use wiremock::Request;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::auth::openai::OpenAiAuth;
    use crate::auth::openai_subscription::OpenAiSubscriptionAuth;
    use crate::client::StopReason;
    use crate::families::openai::OPENAI_CHATGPT_AUTH_PROFILE_ID;

    #[test]
    fn platform_responses_body_keeps_current_message_shape() {
        let body = responses_body(
            &StreamRequest {
                model: String::new(),
                messages: vec![
                    StreamMessage::system("platform system"),
                    StreamMessage::user("hello"),
                ],
                tools: Vec::new(),
                provider_session_id: Some("resp_previous".to_string()),
            },
            "gpt-default",
            false,
            "session-ignored",
        );

        assert_eq!(
            body,
            json!({
                "model": "gpt-default",
                "input": [
                    {
                        "role": "system",
                        "content": [{"type": "input_text", "text": "platform system"}],
                    },
                    {
                        "role": "user",
                        "content": [{"type": "input_text", "text": "hello"}],
                    },
                ],
                "stream": true,
                "previous_response_id": "resp_previous",
            })
        );
        let body = serde_json::to_string(&body).expect("body should serialize");
        assert_eq!(
            body,
            r#"{"input":[{"content":[{"text":"platform system","type":"input_text"}],"role":"system"},{"content":[{"text":"hello","type":"input_text"}],"role":"user"}],"model":"gpt-default","previous_response_id":"resp_previous","stream":true}"#
        );
    }

    #[test]
    fn chatgpt_responses_body_matches_codex_backend_shape() {
        let body = responses_body(
            &StreamRequest {
                model: String::new(),
                messages: vec![
                    StreamMessage::system("base instructions"),
                    StreamMessage::user("hello"),
                ],
                tools: Vec::new(),
                provider_session_id: Some("resp_previous".to_string()),
            },
            "gpt-default",
            true,
            "session-123",
        );
        let body = serde_json::to_string(&body).expect("body should serialize");

        assert_eq!(
            body,
            r#"{"include":[],"input":[{"content":[{"text":"hello","type":"input_text"}],"role":"user"}],"instructions":"base instructions","model":"gpt-default","parallel_tool_calls":true,"prompt_cache_key":"session-123","reasoning":null,"store":false,"stream":true,"tool_choice":"auto","tools":[]}"#
        );
    }

    #[test]
    fn chatgpt_responses_body_combines_system_messages_as_instructions() {
        let body = responses_body(
            &StreamRequest {
                model: "gpt-explicit".to_string(),
                messages: vec![
                    StreamMessage::system("base instructions"),
                    StreamMessage::system("extra system"),
                    StreamMessage::user("hello"),
                ],
                tools: Vec::new(),
                provider_session_id: None,
            },
            "gpt-default",
            true,
            "session-123",
        );

        assert_eq!(
            body["instructions"],
            json!("base instructions\n\nextra system")
        );
        assert_eq!(body["input"].as_array().map(Vec::len), Some(1));
    }

    #[tokio::test]
    async fn platform_responses_stream_completed_event_finishes_turn() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer platform-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "event: response.output_text.delta\n",
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"pong\"}\n\n",
                        "event: response.completed\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_platform\"}}\n\n",
                    )),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::ApiKey {
                key: "platform-key".to_string(),
            },
            "gpt-test",
        )
        .expect("client");

        let mut stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![StreamMessage::user("Reply with: pong")],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await
            .expect("stream");

        assert_eq!(
            stream.next_event().await.expect("text event"),
            Some(StreamEvent::AssistantTextDelta("pong".to_string()))
        );
        assert_eq!(
            stream.next_event().await.expect("completed event"),
            Some(StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            })
        );
    }

    #[tokio::test]
    async fn responses_stream_errors_when_completed_event_is_missing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "event: response.output_text.delta\n",
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"pong\"}\n\n",
                    )),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::ApiKey {
                key: "platform-key".to_string(),
            },
            "gpt-test",
        )
        .expect("client");

        let mut stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![StreamMessage::user("Reply with: pong")],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await
            .expect("stream");

        assert_eq!(
            stream.next_event().await.expect("text event"),
            Some(StreamEvent::AssistantTextDelta("pong".to_string()))
        );
        let error = stream
            .next_event()
            .await
            .expect_err("missing response.completed should fail");
        assert!(matches!(
            error,
            LlmClientError::Network(ref message)
                if message == "stream closed before response.completed"
        ));
    }

    #[tokio::test]
    async fn chatgpt_responses_stream_completed_event_finishes_turn() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer oauth-access"))
            .and(header(OPENAI_CHATGPT_ACCOUNT_HEADER, "acct_test"))
            .and(header(
                OPENAI_RESPONSES_ORIGINATOR_HEADER,
                OPENAI_CHATGPT_RESPONSES_ORIGINATOR,
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "event: response.output_text.delta\n",
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"pong\"}\n\n",
                        "event: response.completed\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_chatgpt\"}}\n\n",
                    )),
            )
            .expect(1)
            .mount(&server)
            .await;

        let store = Arc::new(TestStore::with_credentials(subscription_only_credentials(
            "oauth-access",
            "refresh",
        )));
        let auth = subscription_auth(store, Arc::new(ScriptedHttp::with_responses([])));
        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::Subscription { auth },
            "gpt-test",
        )
        .expect("client");

        let mut stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![
                        StreamMessage::system("base instructions"),
                        StreamMessage::user("Reply with: pong"),
                    ],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await
            .expect("stream");

        assert_eq!(
            stream.next_event().await.expect("text event"),
            Some(StreamEvent::AssistantTextDelta("pong".to_string()))
        );
        assert_eq!(
            stream.next_event().await.expect("completed event"),
            Some(StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            })
        );
    }

    #[tokio::test]
    async fn retries_once_after_unauthorized_with_forced_subscription_refresh() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer old-access"))
            .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer fresh-access"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let store = Arc::new(TestStore::with_credentials(stored_credentials(
            "old-access",
            "refresh",
        )));
        let http = Arc::new(ScriptedHttp::with_responses([OAuthHttpResponse {
            status: 200,
            body: r#"{"access_token":"fresh-oauth-access","api_access_token":"fresh-access","refresh_token":"fresh-refresh","expires_in":3600}"#
                .to_string(),
        }]));
        let auth = subscription_auth(store, http.clone());
        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::Subscription { auth },
            "gpt-test",
        )
        .expect("client");

        let stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![
                        StreamMessage::system("base instructions"),
                        StreamMessage::user("hello"),
                    ],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await;

        assert!(stream.is_ok());
        assert_eq!(http.post_count(), 1);
    }

    #[tokio::test]
    async fn sends_openai_organization_header_for_subscription_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer api-access"))
            .and(header(OPENAI_ORGANIZATION_HEADER, "org_123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let store = Arc::new(TestStore::with_credentials(
            stored_credentials("api-access", "refresh").with_organization("org_123"),
        ));
        let auth = subscription_auth(store, Arc::new(ScriptedHttp::with_responses([])));
        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::Subscription { auth },
            "gpt-test",
        )
        .expect("client");

        let stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![StreamMessage::user("hello")],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await;

        assert!(stream.is_ok());
    }

    #[tokio::test]
    async fn omits_openai_organization_header_without_platform_org() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer api-access"))
            .and(|request: &Request| !request.headers.contains_key(OPENAI_ORGANIZATION_HEADER))
            .and(|request: &Request| !request.headers.contains_key(OPENAI_CHATGPT_ACCOUNT_HEADER))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let store = Arc::new(TestStore::with_credentials(stored_credentials(
            "api-access",
            "refresh",
        )));
        let auth = subscription_auth(store, Arc::new(ScriptedHttp::with_responses([])));
        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::Subscription { auth },
            "gpt-test",
        )
        .expect("client");

        let stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![StreamMessage::user("hello")],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await;

        assert!(stream.is_ok());
    }

    #[tokio::test]
    async fn subscription_without_api_access_token_uses_chatgpt_backend_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer oauth-access"))
            .and(header(OPENAI_CHATGPT_ACCOUNT_HEADER, "acct_test"))
            .and(header(
                OPENAI_RESPONSES_ORIGINATOR_HEADER,
                OPENAI_CHATGPT_RESPONSES_ORIGINATOR,
            ))
            .and(|request: &Request| {
                request
                    .headers
                    .get(OPENAI_RESPONSES_SESSION_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .and(|request: &Request| {
                let Ok(body) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
                    return false;
                };
                body["instructions"].as_str() == Some("base instructions")
                    && body["input"].as_array().map(Vec::len) == Some(1)
                    && body["tools"].as_array().is_some_and(Vec::is_empty)
                    && body["tool_choice"].as_str() == Some("auto")
                    && body["parallel_tool_calls"].as_bool() == Some(true)
                    && body["reasoning"].is_null()
                    && body["store"].as_bool() == Some(false)
                    && body["include"].as_array().is_some_and(Vec::is_empty)
                    && body["prompt_cache_key"].as_str().is_some_and(|value| {
                        request
                            .headers
                            .get(OPENAI_RESPONSES_SESSION_HEADER)
                            .and_then(|header| header.to_str().ok())
                            == Some(value)
                    })
                    && body.get("previous_response_id").is_none()
            })
            .and(|request: &Request| !request.headers.contains_key(OPENAI_ORGANIZATION_HEADER))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let store = Arc::new(TestStore::with_credentials(subscription_only_credentials(
            "oauth-access",
            "refresh",
        )));
        let auth = subscription_auth(store, Arc::new(ScriptedHttp::with_responses([])));
        let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
            server.uri(),
            OpenAiAuth::Subscription { auth },
            "gpt-test",
        )
        .expect("client");

        let stream = client
            .start_stream(
                StreamRequest {
                    model: String::new(),
                    messages: vec![
                        StreamMessage::system("base instructions"),
                        StreamMessage::user("hello"),
                    ],
                    tools: Vec::new(),
                    provider_session_id: None,
                },
                CancellationToken::new(),
            )
            .await;

        assert!(stream.is_ok());
    }

    #[tokio::test]
    async fn subscription_only_auth_profile_without_api_key_builds_chatgpt_route() {
        let store = Arc::new(TestStore::with_credentials(subscription_only_credentials(
            "oauth-access",
            "refresh",
        )));
        let auth = OpenAiAuth::Subscription {
            auth: subscription_auth(store, Arc::new(ScriptedHttp::with_responses([]))),
        };

        let client = OpenAiResponsesClient::from_auth(auth, "gpt-test")
            .expect("subscription-only ChatGPT auth should build client");
        let route = client.auth.route().await.expect("route");
        assert_eq!(route.base_url(), openai::OPENAI_CHATGPT_RESPONSES_BASE_URL);
        assert_eq!(route.bearer_token(), "oauth-access");
        assert_eq!(route.chatgpt_account_id(), Some("acct_test"));
    }

    fn subscription_auth(store: Arc<TestStore>, http: Arc<ScriptedHttp>) -> OpenAiSubscriptionAuth {
        let store: Arc<dyn CredentialStore> = store;
        let http: Arc<dyn OAuthHttpClient> = http;
        let config = OAuthConfig {
            auth_url: url::Url::parse("https://auth.example.test/oauth/authorize")
                .expect("auth url"),
            token_url: url::Url::parse("https://auth.example.test/oauth/token").expect("token url"),
            revoke_url: url::Url::parse("https://auth.example.test/oauth/revoke")
                .expect("revoke url"),
            client_id: "test-client".to_string(),
            scopes: vec!["openid".to_string()],
            redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
            callback_ports: vec![0],
            callback_timeout: std::time::Duration::from_secs(5),
            originator: None,
            allowed_workspace_id: None,
        };
        let key = CredentialKey::new(
            AuthProfileId::new(OPENAI_CHATGPT_AUTH_PROFILE_ID).expect("auth id"),
        );
        let manager = Arc::new(TokenManager::new(
            Arc::clone(&store),
            Arc::clone(&http),
            config.clone(),
            RefreshPolicy::default(),
        ));
        OpenAiSubscriptionAuth::from_parts(
            tokio::runtime::Handle::current(),
            store,
            http,
            config,
            key,
            manager,
            Arc::new(|url| ta_auth_openai::browser::BrowserLaunch::Manual {
                authorize_url: url.clone(),
                reason: "test".to_string(),
            }),
        )
    }

    fn stored_credentials(access_token: &str, refresh_token: &str) -> StoredCredentials {
        StoredCredentials {
            token_set: TokenSet {
                access_token: access_token.to_string(),
                refresh_token: refresh_token.to_string(),
                id_token: None,
                expires_in: Some(3600),
                scope: Some("openid".to_string()),
                api_access_token: Some(access_token.to_string()),
                account_info: None,
            },
            account: AccountInfo {
                account_id: "acct_test".to_string(),
                email: "user@example.test".to_string(),
                organization_id: None,
                plan_tier: Some("plus".to_string()),
            },
            stored_at: now_unix_seconds(),
            last_refreshed_at: None,
        }
    }

    fn subscription_only_credentials(access_token: &str, refresh_token: &str) -> StoredCredentials {
        let mut credentials = stored_credentials(access_token, refresh_token);
        credentials.token_set.api_access_token = None;
        credentials
    }

    trait StoredCredentialsExt {
        fn with_organization(self, organization_id: &str) -> Self;
    }

    impl StoredCredentialsExt for StoredCredentials {
        fn with_organization(mut self, organization_id: &str) -> Self {
            self.account.organization_id = Some(organization_id.to_string());
            self
        }
    }

    fn now_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_secs()
    }

    struct TestStore {
        credentials: Mutex<Option<StoredCredentials>>,
    }

    impl TestStore {
        fn with_credentials(credentials: StoredCredentials) -> Self {
            Self {
                credentials: Mutex::new(Some(credentials)),
            }
        }
    }

    impl CredentialStore for TestStore {
        fn store(
            &self,
            _key: &CredentialKey,
            credentials: &StoredCredentials,
        ) -> Result<(), CredentialStoreError> {
            *self.credentials.lock().expect("store lock") = Some(credentials.clone());
            Ok(())
        }

        fn load(
            &self,
            _key: &CredentialKey,
        ) -> Result<Option<StoredCredentials>, CredentialStoreError> {
            Ok(self.credentials.lock().expect("store lock").clone())
        }

        fn delete(&self, _key: &CredentialKey) -> Result<(), CredentialStoreError> {
            *self.credentials.lock().expect("store lock") = None;
            Ok(())
        }

        fn backend_name(&self) -> &'static str {
            "test"
        }
    }

    struct ScriptedHttp {
        responses: Mutex<VecDeque<OAuthHttpResponse>>,
        post_count: AtomicUsize,
    }

    impl ScriptedHttp {
        fn with_responses<const N: usize>(responses: [OAuthHttpResponse; N]) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                post_count: AtomicUsize::new(0),
            }
        }

        fn post_count(&self) -> usize {
            self.post_count.load(Ordering::SeqCst)
        }
    }

    impl OAuthHttpClient for ScriptedHttp {
        fn post_form<'a>(
            &'a self,
            _url: &'a url::Url,
            _fields: &'a [FormField],
        ) -> OAuthHttpFuture<'a> {
            Box::pin(async move {
                self.post_count.fetch_add(1, Ordering::SeqCst);
                self.responses
                    .lock()
                    .expect("responses lock")
                    .pop_front()
                    .ok_or_else(|| {
                        ta_auth_openai::OAuthError::HttpTransport("no response".to_string())
                    })
            })
        }
    }
}
