use ta_protocol::wire::{AgentRuntimeModelId, OutputContractKind, RecipeResolutionError};

use super::RecipeRegistry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegateRecipeResolutionRequest {
    pub objective: String,
    pub output_contract: Option<OutputContractKind>,
    pub model_id: Option<AgentRuntimeModelId>,
    pub sandbox_profile: Option<String>,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDelegateRecipeRequest {
    pub objective: String,
    pub output_contract: Option<OutputContractKind>,
    pub model_id: Option<AgentRuntimeModelId>,
    pub sandbox_profile: Option<String>,
    pub recipe_id: Option<String>,
}

pub fn resolve_delegate_recipe(
    registry: &RecipeRegistry,
    request: DelegateRecipeResolutionRequest,
) -> Result<ResolvedDelegateRecipeRequest, RecipeResolutionError> {
    let Some(recipe_id) = request.recipe_id else {
        return Ok(ResolvedDelegateRecipeRequest {
            objective: request.objective,
            output_contract: request.output_contract,
            model_id: request.model_id,
            sandbox_profile: request.sandbox_profile,
            recipe_id: None,
        });
    };

    let recipe =
        registry
            .get(&recipe_id)
            .ok_or_else(|| RecipeResolutionError::UnknownRecipeId {
                recipe_id: recipe_id.clone(),
            })?;
    if let Some(request_contract) = request.output_contract
        && request_contract != recipe.contract
    {
        return Err(RecipeResolutionError::RecipeContractConflict {
            recipe_id,
            recipe_contract: recipe.contract,
            request_contract,
        });
    }

    Ok(ResolvedDelegateRecipeRequest {
        objective: format!("{}\n\n{}", recipe.prompt_template, request.objective),
        output_contract: Some(recipe.contract),
        model_id: request.model_id.or_else(|| {
            recipe
                .default_model
                .as_deref()
                .map(model_id_from_valid_recipe)
        }),
        sandbox_profile: request
            .sandbox_profile
            .or_else(|| recipe.default_sandbox_profile.clone()),
        recipe_id: Some(recipe_id),
    })
}

fn model_id_from_valid_recipe(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value.to_string())
        .expect("recipe validation should reject empty default model ids")
}
