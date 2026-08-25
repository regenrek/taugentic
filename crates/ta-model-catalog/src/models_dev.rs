use std::time::Duration;

use crate::{ModelCatalog, ModelCatalogError, validate_catalog};

pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone)]
pub struct ModelsDevCatalogSource {
    client: reqwest::Client,
    url: String,
}

impl ModelsDevCatalogSource {
    pub fn new() -> Result<Self, ModelCatalogError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            url: MODELS_DEV_URL.to_string(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_url(url: String) -> Result<Self, ModelCatalogError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            url,
        })
    }

    pub async fn fetch(&self) -> Result<ModelCatalog, ModelCatalogError> {
        let response = self.client.get(&self.url).send().await?;
        if !response.status().is_success() {
            return Err(ModelCatalogError::HttpStatus(response.status()));
        }
        let value = serde_json::from_slice::<serde_json::Value>(&response.bytes().await?)?;
        validate_catalog(crate::generated::normalize(value)?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn fetch_normalizes_a_complete_upstream_snapshot() {
        let server = MockServer::start().await;
        let mut providers = Map::new();
        for provider_id in [
            "anthropic",
            "deepseek",
            "google",
            "groq",
            "openai",
            "openrouter",
            "xai",
        ] {
            providers.insert(provider_id.to_string(), provider(provider_id));
        }
        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(Value::Object(providers)))
            .mount(&server)
            .await;

        let source =
            ModelsDevCatalogSource::with_url(format!("{}/api.json", server.uri())).expect("source");
        let catalog = source.fetch().await.expect("catalog");

        assert_eq!(catalog.providers.len(), 7);
        assert!(
            catalog
                .provider("openai")
                .expect("openai")
                .models
                .contains_key("openai-current")
        );
    }

    fn provider(provider_id: &str) -> Value {
        json!({
            "id": provider_id,
            "name": provider_id,
            "models": {
                format!("{provider_id}-current"): {
                    "id": format!("{provider_id}-current"),
                    "name": "Current",
                    "release_date": "2026-08-25",
                    "tool_call": true,
                    "reasoning": true,
                    "structured_output": true,
                    "limit": { "context": 128000 },
                    "cost": { "input": 1.0, "output": 2.0 },
                    "modalities": { "input": ["text"] }
                }
            }
        })
    }
}
