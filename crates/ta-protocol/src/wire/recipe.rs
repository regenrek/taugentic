use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::wire::OutputContractKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CapsuleRecipe {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub contract: OutputContractKind,
    pub prompt_template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sandbox_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Error)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum RecipeValidationError {
    #[error("recipe id is empty")]
    EmptyId,
    #[error("recipe name is empty")]
    EmptyName,
    #[error("prompt template is empty")]
    EmptyTemplate,
    #[error("default model is empty")]
    EmptyDefaultModel,
    #[error("default sandbox profile is empty")]
    EmptyDefaultSandboxProfile,
    #[error("recipe id contains invalid characters: {value}")]
    InvalidIdCharacters { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS, Error)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum RecipeResolutionError {
    #[error("unknown recipe id: {recipe_id}")]
    UnknownRecipeId { recipe_id: String },
    #[error(
        "recipe {recipe_id} requires {recipe_contract:?} output contract, got {request_contract:?}"
    )]
    RecipeContractConflict {
        recipe_id: String,
        recipe_contract: OutputContractKind,
        request_contract: OutputContractKind,
    },
}

impl CapsuleRecipe {
    pub fn validate(&self) -> Result<(), RecipeValidationError> {
        if self.id.trim().is_empty() {
            return Err(RecipeValidationError::EmptyId);
        }
        if !self.id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        }) {
            return Err(RecipeValidationError::InvalidIdCharacters {
                value: self.id.clone(),
            });
        }
        if self.name.trim().is_empty() {
            return Err(RecipeValidationError::EmptyName);
        }
        if self.prompt_template.trim().is_empty() {
            return Err(RecipeValidationError::EmptyTemplate);
        }
        if self
            .default_model
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(RecipeValidationError::EmptyDefaultModel);
        }
        if self
            .default_sandbox_profile
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(RecipeValidationError::EmptyDefaultSandboxProfile);
        }
        Ok(())
    }
}
