use ta_orchestrator::{
    DelegateRecipeResolutionRequest, RecipeRegistry, ResolvedDelegateRecipeRequest,
    resolve_delegate_recipe,
};
use ta_protocol::wire::{AgentRuntimeModelId, OutputContractKind, RecipeResolutionError};

fn request(
    objective: &str,
    recipe_id: Option<&str>,
    output_contract: Option<OutputContractKind>,
) -> DelegateRecipeResolutionRequest {
    DelegateRecipeResolutionRequest {
        objective: objective.to_string(),
        output_contract,
        model_id: None,
        recipe_id: recipe_id.map(str::to_string),
    }
}

fn resolve(request: DelegateRecipeResolutionRequest) -> ResolvedDelegateRecipeRequest {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");
    resolve_delegate_recipe(&registry, request).expect("recipe should resolve")
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

#[test]
fn delegate_with_known_recipe_resolves_contract() {
    let resolved = resolve(request("Review focused files", Some("review-agent"), None));

    assert_eq!(resolved.output_contract, Some(OutputContractKind::Review));
}

#[test]
fn delegate_with_known_recipe_prepends_template() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");
    let template = registry
        .get("review-agent")
        .expect("review recipe should exist")
        .prompt_template
        .clone();
    let resolved = resolve_delegate_recipe(
        &registry,
        request("Review focused files", Some("review-agent"), None),
    )
    .expect("recipe should resolve");

    assert_eq!(
        resolved.objective,
        format!("{template}\n\nReview focused files")
    );
}

#[test]
fn delegate_with_known_recipe_uses_default_model_when_omitted() {
    let resolved = resolve(request("Review focused files", Some("review-agent"), None));

    assert_eq!(
        resolved.model_id,
        Some(model_id("claude-4.6-sonnet-medium-thinking"))
    );
}

#[test]
fn delegate_with_known_recipe_caller_overrides_model() {
    let mut request = request("Review focused files", Some("review-agent"), None);
    request.model_id = Some(model_id("caller-model"));

    let resolved = resolve(request);

    assert_eq!(resolved.model_id, Some(model_id("caller-model")));
}

#[test]
fn delegate_with_unknown_recipe_returns_typed_error() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    let error = resolve_delegate_recipe(
        &registry,
        request("Review focused files", Some("missing-agent"), None),
    )
    .expect_err("unknown recipe should fail");

    assert_eq!(
        error,
        RecipeResolutionError::UnknownRecipeId {
            recipe_id: "missing-agent".to_string()
        }
    );
}

#[test]
fn delegate_with_recipe_and_conflicting_explicit_contract_returns_typed_error() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    let error = resolve_delegate_recipe(
        &registry,
        request(
            "Review focused files",
            Some("review-agent"),
            Some(OutputContractKind::Patch),
        ),
    )
    .expect_err("contract conflict should fail");

    assert_eq!(
        error,
        RecipeResolutionError::RecipeContractConflict {
            recipe_id: "review-agent".to_string(),
            recipe_contract: OutputContractKind::Review,
            request_contract: OutputContractKind::Patch,
        }
    );
    assert_eq!(
        serde_json::to_value(&error).expect("error should serialize"),
        serde_json::json!({
            "kind": "recipeContractConflict",
            "recipeId": "review-agent",
            "recipeContract": "review",
            "requestContract": "patch"
        })
    );
}

#[test]
fn delegate_without_recipe_id_proceeds_unchanged() {
    let mut request = request(
        "Use explicit caller fields",
        None,
        Some(OutputContractKind::Custom),
    );
    request.model_id = Some(model_id("caller-model"));

    let resolved = resolve(request);

    assert_eq!(
        resolved,
        ResolvedDelegateRecipeRequest {
            objective: "Use explicit caller fields".to_string(),
            output_contract: Some(OutputContractKind::Custom),
            model_id: Some(model_id("caller-model")),
            recipe_id: None,
        }
    );
}
