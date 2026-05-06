use std::fs;

use ta_orchestrator::{RecipeRegistry, RecipeRegistryError, RegistryLoadOutcome};
use ta_protocol::wire::OutputContractKind;
use taugentic_agent::tools::render_subagent_tool_description;

const BUILTIN_RECIPES: [(&str, OutputContractKind); 5] = [
    ("debug-agent", OutputContractKind::Debug),
    ("patch-agent", OutputContractKind::Patch),
    ("review-agent", OutputContractKind::Review),
    ("test-agent", OutputContractKind::Test),
    ("plan-agent", OutputContractKind::Plan),
];

#[test]
fn load_builtin_loads_all_five_recipes() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");
    let mut ids = registry.ids().collect::<Vec<_>>();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "debug-agent",
            "patch-agent",
            "plan-agent",
            "review-agent",
            "test-agent"
        ]
    );
}

#[test]
fn each_builtin_recipe_validates() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    for (id, _) in BUILTIN_RECIPES {
        registry
            .get(id)
            .expect("built-in recipe should exist")
            .validate()
            .expect("built-in recipe should validate");
    }
}

#[test]
fn each_builtin_recipe_uses_correct_contract_kind() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    for (id, expected_contract) in BUILTIN_RECIPES {
        let recipe = registry.get(id).expect("built-in recipe should exist");
        assert_eq!(recipe.contract, expected_contract, "contract for {id}");
    }
}

#[test]
fn builtin_recipes_have_descriptions_for_tool_projection() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    for (id, _) in BUILTIN_RECIPES {
        let recipe = registry.get(id).expect("built-in recipe should exist");
        assert!(
            recipe
                .description
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty()),
            "description for {id}"
        );
    }
}

#[test]
fn builtin_recipes_render_in_subagent_tool_description() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");
    let recipes = registry.recipes().into_iter().cloned().collect::<Vec<_>>();
    let description = render_subagent_tool_description(&recipes);

    for (id, _) in BUILTIN_RECIPES {
        let recipe = registry.get(id).expect("built-in recipe should exist");
        assert!(description.contains(id), "recipe id {id}");
        assert!(
            description.contains(
                recipe
                    .description
                    .as_deref()
                    .expect("built-in recipe description")
            ),
            "description for {id}"
        );
    }
}

#[test]
fn lookup_returns_none_for_unknown_id() {
    let registry = RecipeRegistry::load_builtin().expect("built-in recipes should load");

    assert!(registry.get("missing-agent").is_none());
}

#[test]
fn user_dir_recipe_loads() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    let recipe_path = user_dir.path().join("custom-agent.toml");
    fs::write(&recipe_path, custom_recipe("custom-agent", "Custom Agent"))
        .expect("write user recipe");

    let outcome = load_user_recipes(&user_dir);

    assert!(outcome.diagnostics.is_empty());
    let recipe = outcome
        .registry
        .get("custom-agent")
        .expect("custom user recipe should load");
    assert_eq!(recipe.contract, OutputContractKind::Custom);
    assert!(outcome.registry.get("debug-agent").is_some());
}

#[test]
fn user_collision_with_builtin_skips_user_with_diagnostic() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        user_dir.path().join("debug-agent.toml"),
        custom_recipe("debug-agent", "User Debug Agent"),
    )
    .expect("write colliding user recipe");

    let outcome = load_user_recipes(&user_dir);

    assert_eq!(outcome.diagnostics.len(), 1);
    let diagnostic = &outcome.diagnostics[0];
    assert_eq!(diagnostic.path, user_dir.path().join("debug-agent.toml"));
    assert!(matches!(
        &diagnostic.error,
        RecipeRegistryError::DuplicateId(id) if id == "debug-agent"
    ));
    assert_eq!(
        outcome
            .registry
            .get("debug-agent")
            .expect("built-in recipe should remain")
            .name,
        "Debug Agent"
    );
}

#[test]
fn corrupt_user_toml_returns_diagnostic_not_error() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    let malformed_path = user_dir.path().join("malformed.toml");
    fs::write(&malformed_path, "id = [").expect("write malformed user recipe");

    let outcome = load_user_recipes(&user_dir);

    assert!(outcome.registry.get("debug-agent").is_some());
    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(matches!(
        &outcome.diagnostics[0].error,
        RecipeRegistryError::TomlParse { path, .. } if *path == malformed_path
    ));
}

#[test]
fn multiple_corrupt_files_collect_all_diagnostics() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    let malformed_path = user_dir.path().join("malformed.toml");
    let invalid_path = user_dir.path().join("invalid.toml");
    fs::write(&malformed_path, "id = [").expect("write malformed user recipe");
    fs::write(
        &invalid_path,
        r#"
id = "invalid-agent"
name = ""
contract = "custom"
promptTemplate = "Return a custom capsule result."
"#,
    )
    .expect("write invalid user recipe");

    let outcome = load_user_recipes(&user_dir);
    let diagnostic_paths = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.path.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        diagnostic_paths,
        [invalid_path.clone(), malformed_path.clone()]
    );
    assert!(matches!(
        &outcome.diagnostics[0].error,
        RecipeRegistryError::InvalidRecipe { id, .. } if id == "invalid-agent"
    ));
    assert!(matches!(
        &outcome.diagnostics[1].error,
        RecipeRegistryError::TomlParse { path, .. } if *path == malformed_path
    ));
}

#[test]
fn corrupt_user_file_alongside_valid_loads_valid_one() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    let valid_path = user_dir.path().join("custom-agent.toml");
    let malformed_path = user_dir.path().join("malformed.toml");
    fs::write(&valid_path, custom_recipe("custom-agent", "Custom Agent"))
        .expect("write valid user recipe");
    fs::write(&malformed_path, "id = [").expect("write malformed user recipe");

    let outcome = load_user_recipes(&user_dir);

    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(matches!(
        &outcome.diagnostics[0].error,
        RecipeRegistryError::TomlParse { path, .. } if *path == malformed_path
    ));
    assert!(outcome.registry.get("custom-agent").is_some());
    assert!(outcome.registry.get("debug-agent").is_some());
}

#[test]
fn user_dir_listing_io_error_still_propagates() {
    let user_dir = tempfile::tempdir().expect("temp dir");
    let not_a_directory = user_dir.path().join("not-a-directory.toml");
    fs::write(&not_a_directory, "").expect("write non-directory path");

    let error = RecipeRegistry::load_with_user_dir(Some(&not_a_directory))
        .expect_err("directory listing failure should remain hard error");

    assert!(matches!(
        error,
        RecipeRegistryError::Io { path, .. } if path == not_a_directory
    ));
}

fn load_user_recipes(user_dir: &tempfile::TempDir) -> RegistryLoadOutcome {
    RecipeRegistry::load_with_user_dir(Some(user_dir.path())).expect("registry should load")
}

fn custom_recipe(id: &str, name: &str) -> String {
    format!(
        r#"
id = "{id}"
name = "{name}"
contract = "custom"
promptTemplate = "Return a custom capsule result."
"#
    )
}
