mod generated;
mod models_dev;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use ta_protocol::wire::{AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeStrategyId};

pub use models_dev::{MODELS_DEV_URL, ModelsDevCatalogSource};

const EMBEDDED_CATALOG_JSON: &str = include_str!("../generated/catalog.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCatalog {
    pub generated_at: String,
    pub source: String,
    pub providers: BTreeMap<String, CatalogProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    pub models: BTreeMap<String, CatalogModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub release_date: Option<String>,
    pub context_limit: Option<u64>,
    pub input_cost_per_million_micros: Option<u64>,
    pub output_cost_per_million_micros: Option<u64>,
    pub reasoning: bool,
    pub tool_call: bool,
    pub structured_output: bool,
    pub input_modalities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModelCatalogStore {
    inner: Arc<RwLock<ModelCatalog>>,
}

impl ModelCatalogStore {
    pub fn embedded() -> Result<Self, ModelCatalogError> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ModelCatalog::embedded()?)),
        })
    }

    pub fn snapshot(&self) -> ModelCatalog {
        self.inner
            .read()
            .expect("model catalog store must not be poisoned")
            .clone()
    }

    pub fn replace(&self, catalog: ModelCatalog) -> Result<(), ModelCatalogError> {
        let catalog = validate_catalog(catalog)?;
        *self
            .inner
            .write()
            .expect("model catalog store must not be poisoned") = catalog;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelCatalogError {
    #[error("model catalog JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("model catalog provider {0} is missing")]
    ProviderMissing(String),
    #[error("model catalog provider {0} has no supported models")]
    ProviderEmpty(String),
    #[error("model catalog request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("model catalog endpoint returned HTTP {0}")]
    HttpStatus(reqwest::StatusCode),
}

impl ModelCatalog {
    pub fn embedded() -> Result<Self, ModelCatalogError> {
        let catalog = serde_json::from_str(EMBEDDED_CATALOG_JSON)?;
        validate_catalog(catalog)
    }

    pub fn provider(&self, provider_id: &str) -> Option<&CatalogProvider> {
        self.providers.get(provider_id)
    }

    pub fn models(&self, provider_id: &str) -> Vec<AgentRuntimeModelRef> {
        self.provider(provider_id)
            .map(|provider| provider.models.values().map(model_ref).collect())
            .unwrap_or_default()
    }

    pub fn contains_model(
        &self,
        provider_id: &AgentRuntimeStrategyId,
        model_id: &AgentRuntimeModelId,
    ) -> bool {
        self.provider(provider_id.as_str())
            .is_some_and(|provider| provider.models.contains_key(model_id.as_str()))
    }

    pub fn default_model(&self, provider_id: &str) -> Option<AgentRuntimeModelId> {
        let provider = self.provider(provider_id)?;
        provider
            .models
            .values()
            .max_by(|left, right| default_rank(left).cmp(&default_rank(right)))
            .and_then(|model| AgentRuntimeModelId::new(&model.id).ok())
    }
}

fn default_rank(model: &CatalogModel) -> (bool, &str, bool, std::cmp::Reverse<usize>, &str) {
    (
        model.tool_call,
        model.release_date.as_deref().unwrap_or_default(),
        model.reasoning,
        std::cmp::Reverse(model.id.len()),
        model.id.as_str(),
    )
}

fn model_ref(model: &CatalogModel) -> AgentRuntimeModelRef {
    AgentRuntimeModelRef {
        id: AgentRuntimeModelId::new(&model.id).expect("generated model id must be valid"),
        display_name: model.name.clone(),
        context_limit: model.context_limit,
        input_cost_per_million_micros: model.input_cost_per_million_micros,
        output_cost_per_million_micros: model.output_cost_per_million_micros,
        reasoning: model.reasoning,
        tool_call: model.tool_call,
        structured_output: model.structured_output,
        input_modalities: model.input_modalities.clone(),
    }
}

fn validate_catalog(catalog: ModelCatalog) -> Result<ModelCatalog, ModelCatalogError> {
    for provider_id in [
        "anthropic",
        "deepseek",
        "google",
        "groq",
        "openai",
        "openrouter",
        "xai",
    ] {
        let provider = catalog
            .provider(provider_id)
            .ok_or_else(|| ModelCatalogError::ProviderMissing(provider_id.to_string()))?;
        if provider.models.is_empty() {
            return Err(ModelCatalogError::ProviderEmpty(provider_id.to_string()));
        }
        let ids = provider
            .models
            .values()
            .map(|model| &model.id)
            .collect::<BTreeSet<_>>();
        if ids.len() != provider.models.len() {
            return Err(ModelCatalogError::ProviderEmpty(provider_id.to_string()));
        }
    }
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_contains_current_frontier_and_multi_provider_models() {
        let catalog = ModelCatalog::embedded().expect("embedded catalog");
        let openai = catalog.provider("openai").expect("OpenAI provider");
        for model_id in ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"] {
            assert!(openai.models.contains_key(model_id));
        }
        for provider_id in [
            "anthropic",
            "google",
            "deepseek",
            "groq",
            "openrouter",
            "xai",
        ] {
            assert!(!catalog.models(provider_id).is_empty(), "{provider_id}");
        }
    }

    #[test]
    fn default_model_is_derived_from_current_catalog_metadata() {
        let catalog = ModelCatalog::embedded().expect("embedded catalog");
        let default = catalog.default_model("openai").expect("OpenAI default");
        assert!(catalog.contains_model(
            &AgentRuntimeStrategyId::new("openai").expect("provider id"),
            &default,
        ));
    }

    #[test]
    fn invalid_replacement_does_not_mutate_the_active_catalog() {
        let store = ModelCatalogStore::embedded().expect("store");
        let before = store.snapshot();
        let mut invalid = before.clone();
        invalid.providers.remove("openai");

        assert!(store.replace(invalid).is_err());
        assert_eq!(store.snapshot(), before);
    }
}
