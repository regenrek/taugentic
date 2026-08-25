use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{CatalogModel, CatalogProvider, ModelCatalog, ModelCatalogError};

#[derive(Debug, Deserialize)]
struct UpstreamProvider {
    id: String,
    name: String,
    #[serde(default)]
    models: BTreeMap<String, UpstreamModel>,
}

#[derive(Debug, Deserialize)]
struct UpstreamModel {
    id: String,
    name: String,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    structured_output: bool,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<UpstreamLimit>,
    #[serde(default)]
    cost: Option<UpstreamCost>,
    #[serde(default)]
    modalities: Option<UpstreamModalities>,
}

#[derive(Debug, Deserialize)]
struct UpstreamLimit {
    #[serde(default)]
    context: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct UpstreamCost {
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct UpstreamModalities {
    #[serde(default)]
    input: Vec<String>,
}

pub(crate) fn normalize(value: serde_json::Value) -> Result<ModelCatalog, ModelCatalogError> {
    let upstream = serde_json::from_value::<BTreeMap<String, UpstreamProvider>>(value)?;
    let providers = upstream
        .into_iter()
        .map(|(provider_id, provider)| {
            let models = provider
                .models
                .into_iter()
                .filter(|(_, model)| {
                    model.tool_call && model.status.as_deref() != Some("deprecated")
                })
                .map(|(model_id, model)| {
                    let value = CatalogModel {
                        id: model.id,
                        name: model.name,
                        release_date: model.release_date,
                        context_limit: model.limit.and_then(|limit| limit.context),
                        input_cost_per_million_micros: model
                            .cost
                            .as_ref()
                            .and_then(|cost| dollars_to_micros(cost.input)),
                        output_cost_per_million_micros: model
                            .cost
                            .as_ref()
                            .and_then(|cost| dollars_to_micros(cost.output)),
                        reasoning: model.reasoning,
                        tool_call: model.tool_call,
                        structured_output: model.structured_output,
                        input_modalities: model
                            .modalities
                            .map(|modalities| modalities.input)
                            .unwrap_or_default(),
                    };
                    (model_id, value)
                })
                .collect();
            (
                provider_id,
                CatalogProvider {
                    id: provider.id,
                    name: provider.name,
                    models,
                },
            )
        })
        .collect();
    Ok(ModelCatalog {
        generated_at: "runtime".to_string(),
        source: "models.dev".to_string(),
        providers,
    })
}

fn dollars_to_micros(value: Option<f64>) -> Option<u64> {
    value.map(|value| (value * 1_000_000.0).round() as u64)
}
