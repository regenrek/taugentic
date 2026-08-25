use std::fmt::Write;

use ta_protocol::wire::{CapsuleRecipe, OutputContractKind};

pub fn render_subagent_tool_description(recipes: &[CapsuleRecipe]) -> String {
    let mut ordered_recipes = recipes.iter().collect::<Vec<_>>();
    ordered_recipes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut description = String::from(
        "Spawn a daemon-owned native child run (subagent / capsule) to execute a scoped objective in isolation. Use when:\n\
\n\
- The work is naturally parallelizable from the current turn\n\
- The subtask benefits from a fresh, focused context window\n\
- A structured typed result (debug, patch, review, test, plan) is desired\n\
- You want to delegate ownership rather than do the work inline\n",
    );

    if ordered_recipes.is_empty() {
        description.push_str(
            "\nAvailable recipes: no recipes registered. Use `outputContract` for custom delegation.\n",
        );
    } else {
        description.push_str("\nAvailable recipes (recommended for typical objectives):\n");
        for recipe in &ordered_recipes {
            writeln!(
                description,
                "- `{}` -> {} -- {}",
                recipe.id,
                contract_result_name(recipe.contract),
                recipe_summary(recipe),
            )
            .expect("writing to String should not fail");
        }
    }

    append_examples(&mut description, &ordered_recipes);
    description.push_str(
        "\nNotes:\n\
- Provide a precise objective. The subagent only sees the objective, not your context.\n\
- recipeId auto-sets the output contract; explicit outputContract that conflicts with the recipe is rejected.\n\
- Subagent runs durably; you receive a typed result + provenance receipt.\n",
    );
    description
}

fn append_examples(description: &mut String, recipes: &[&CapsuleRecipe]) {
    description.push_str("\nExamples:\n\n");
    let mut example_index = 1;

    if let Some(recipe_id) = recipe_id_for_contract(recipes, OutputContractKind::Debug) {
        writeln!(
            description,
            "{}. Investigate a bug:\n   {{ \"objective\": \"Find why login redirect drops the OAuth state\", \"recipeId\": \"{}\" }}\n",
            example_index, recipe_id,
        )
        .expect("writing to String should not fail");
        example_index += 1;
    }

    if let Some(recipe_id) = recipe_id_for_contract(recipes, OutputContractKind::Patch) {
        writeln!(
            description,
            "{}. Apply a fix from a debug result:\n   {{ \"objective\": \"Apply the SameSite=Lax fix to session cookie path /oauth/*\", \"recipeId\": \"{}\" }}\n",
            example_index, recipe_id,
        )
        .expect("writing to String should not fail");
        example_index += 1;
    }

    writeln!(
        description,
        "{}. Custom delegation without recipe:\n   {{ \"objective\": \"Summarize all TODO comments in src/\", \"outputContract\": \"custom\" }}",
        example_index,
    )
    .expect("writing to String should not fail");
}

fn recipe_id_for_contract<'a>(
    recipes: &[&'a CapsuleRecipe],
    contract: OutputContractKind,
) -> Option<&'a str> {
    recipes
        .iter()
        .find(|recipe| recipe.contract == contract)
        .map(|recipe| recipe.id.as_str())
}

fn recipe_summary(recipe: &CapsuleRecipe) -> &str {
    recipe
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| recipe.name.trim())
}

fn contract_result_name(contract: OutputContractKind) -> &'static str {
    match contract {
        OutputContractKind::Debug => "DebugResult",
        OutputContractKind::Patch => "PatchResult",
        OutputContractKind::Review => "ReviewResult",
        OutputContractKind::Test => "TestResult",
        OutputContractKind::Plan => "PlanResult",
        OutputContractKind::Custom => "CustomResult",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_recipes_render_explicit_hint() {
        let description = render_subagent_tool_description(&[]);

        assert!(description.contains("no recipes registered"));
        assert!(!description.contains("Available recipes (recommended for typical objectives):"));
        assert!(description.contains("\"outputContract\": \"custom\""));
    }

    #[test]
    fn synthetic_recipe_appears_in_description() {
        let description = render_subagent_tool_description(&[recipe(
            "custom-review",
            "Custom Review",
            Some("Reviews the selected boundary."),
            OutputContractKind::Review,
        )]);

        assert!(description.contains("`custom-review` -> ReviewResult"));
        assert!(description.contains("Reviews the selected boundary."));
    }

    #[test]
    fn recipe_name_is_summary_fallback() {
        let description = render_subagent_tool_description(&[recipe(
            "name-only",
            "Name Only",
            None,
            OutputContractKind::Plan,
        )]);

        assert!(description.contains("`name-only` -> PlanResult -- Name Only"));
    }

    #[test]
    fn recipe_examples_use_available_recipe_ids() {
        let description = render_subagent_tool_description(&[
            recipe(
                "patch-example",
                "Patch Example",
                Some("Applies a change."),
                OutputContractKind::Patch,
            ),
            recipe(
                "debug-example",
                "Debug Example",
                Some("Investigates a bug."),
                OutputContractKind::Debug,
            ),
        ]);

        assert!(description.contains("\"recipeId\": \"debug-example\""));
        assert!(description.contains("\"recipeId\": \"patch-example\""));
    }

    fn recipe(
        id: &str,
        name: &str,
        description: Option<&str>,
        contract: OutputContractKind,
    ) -> CapsuleRecipe {
        CapsuleRecipe {
            id: id.to_string(),
            name: name.to_string(),
            description: description.map(str::to_string),
            contract,
            prompt_template: "Return a capsule result.".to_string(),
            default_model: None,
        }
    }
}
