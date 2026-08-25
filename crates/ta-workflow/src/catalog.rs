use std::collections::{BTreeMap, BTreeSet};

use ta_protocol::wire::{AgentRuntimeModelId, AgentRuntimeStrategyId};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeModelCatalog {
    models_by_provider: BTreeMap<String, BTreeSet<String>>,
}

impl RuntimeModelCatalog {
    pub(crate) fn built_in() -> Self {
        let mut catalog = Self {
            models_by_provider: BTreeMap::new(),
        };
        catalog.add(
            ta_provider_llm::families::codex_app_server::CODEX_PROVIDER_ID,
            Vec::new(),
        );
        catalog.add(
            ta_provider_llm::families::openai::OPENAI_PROVIDER_ID,
            ta_provider_llm::catalog::openai_models(),
        );
        catalog.add(
            ta_provider_llm::families::anthropic::ANTHROPIC_PROVIDER_ID,
            ta_provider_llm::catalog::anthropic_models(),
        );
        for spec in ta_provider_llm::declarative::specs() {
            catalog.models_by_provider.insert(
                spec.id.as_ref().to_string(),
                spec.models
                    .iter()
                    .map(|model| model.id.as_ref().to_string())
                    .collect(),
            );
        }
        catalog
    }

    pub(crate) fn contains_provider(&self, provider: &AgentRuntimeStrategyId) -> bool {
        self.models_by_provider.contains_key(provider.as_str())
    }

    pub(crate) fn contains_model(
        &self,
        provider: &AgentRuntimeStrategyId,
        model: &AgentRuntimeModelId,
    ) -> bool {
        if provider.as_str() == ta_provider_llm::families::codex_app_server::CODEX_PROVIDER_ID {
            // Codex owns a user- and release-specific live catalog. Workflow
            // parsing validates the typed id; runtime selection validates live
            // availability.
            return true;
        }
        self.models_by_provider
            .get(provider.as_str())
            .is_some_and(|models| models.contains(model.as_str()))
    }

    fn add(&mut self, provider: &str, models: Vec<ta_protocol::wire::AgentRuntimeModelRef>) {
        self.models_by_provider.insert(
            provider.to_string(),
            models
                .into_iter()
                .map(|model| model.id.as_str().to_string())
                .collect(),
        );
    }
}
