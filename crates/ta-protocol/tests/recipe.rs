use serde_json::json;
use ta_protocol::wire::{CapsuleRecipe, OutputContractKind, RecipeValidationError};

fn valid_recipe() -> CapsuleRecipe {
    CapsuleRecipe {
        id: "debug_patch".to_string(),
        name: "Debug Patch".to_string(),
        description: Some("Investigates a root cause and proposes a patch.".to_string()),
        contract: OutputContractKind::Debug,
        prompt_template: "Find the root cause and propose a patch.".to_string(),
        default_model: Some("claude-sonnet".to_string()),
        default_sandbox_profile: Some("workspace-write".to_string()),
    }
}

#[test]
fn roundtrip_serialization_preserves_camel_case() {
    let recipe = valid_recipe();

    let value = serde_json::to_value(&recipe).expect("recipe should serialize");
    assert_eq!(
        value,
        json!({
            "id": "debug_patch",
            "name": "Debug Patch",
            "description": "Investigates a root cause and proposes a patch.",
            "contract": "debug",
            "promptTemplate": "Find the root cause and propose a patch.",
            "defaultModel": "claude-sonnet",
            "defaultSandboxProfile": "workspace-write"
        })
    );
    assert!(value.get("prompt_template").is_none(), "{value}");
    assert!(value.get("description").is_some(), "{value}");
    assert!(value.get("default_model").is_none(), "{value}");
    assert!(value.get("default_sandbox_profile").is_none(), "{value}");

    let decoded: CapsuleRecipe =
        serde_json::from_value(value).expect("recipe should deserialize from camelCase json");
    assert_eq!(decoded, recipe);
}

#[test]
fn validate_rejects_empty_id() {
    let mut recipe = valid_recipe();
    recipe.id = "  ".to_string();

    assert_eq!(recipe.validate(), Err(RecipeValidationError::EmptyId));
}

#[test]
fn validate_rejects_invalid_id_chars() {
    let mut recipe = valid_recipe();
    recipe.id = "debug patch".to_string();

    assert_eq!(
        recipe.validate(),
        Err(RecipeValidationError::InvalidIdCharacters {
            value: "debug patch".to_string()
        })
    );
}

#[test]
fn validate_rejects_empty_name() {
    let mut recipe = valid_recipe();
    recipe.name = "  ".to_string();

    assert_eq!(recipe.validate(), Err(RecipeValidationError::EmptyName));
}

#[test]
fn validate_rejects_empty_template() {
    let mut recipe = valid_recipe();
    recipe.prompt_template = "  ".to_string();

    assert_eq!(recipe.validate(), Err(RecipeValidationError::EmptyTemplate));
}

#[test]
fn validate_accepts_minimal_valid_recipe() {
    let recipe = CapsuleRecipe {
        id: "review-plan_1".to_string(),
        name: "Review Plan".to_string(),
        description: None,
        contract: OutputContractKind::Review,
        prompt_template: "Review the implementation plan.".to_string(),
        default_model: None,
        default_sandbox_profile: None,
    };

    recipe.validate().expect("minimal recipe should validate");
}

#[test]
fn recipe_validation_error_serializes_camel_case() {
    let error = RecipeValidationError::InvalidIdCharacters {
        value: "debug patch".to_string(),
    };

    let value = serde_json::to_value(error).expect("recipe validation error should serialize");

    assert_eq!(
        value,
        json!({
            "kind": "invalidIdCharacters",
            "value": {
                "value": "debug patch"
            }
        })
    );
}
