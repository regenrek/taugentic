pub(crate) const DELEGATION_GUIDELINES_HEADING: &str = "# Delegation guidelines";

pub(crate) fn subagent_delegation_guidelines(recipe_count: usize) -> String {
    let recipe_guidance = if recipe_count == 0 {
        "- Use outputContract when no recipe matches or no recipes are registered"
    } else {
        "- Prefer a recipeId when one matches the work; it auto-sets the typed contract"
    };

    format!(
        "{DELEGATION_GUIDELINES_HEADING}\n\n\
You can delegate scoped subtasks to native subagents (capsules) via the \
\"subagent\" tool. Delegate when at least one of these is true:\n\n\
- The subtask is naturally parallelizable from the current step\n\
- A focused, fresh context is more efficient than reusing yours\n\
- You want a typed structured result (debug, patch, review, test, plan)\n\
- The subtask owns a distinct piece of work you do not need to do inline\n\n\
How to delegate effectively:\n\n\
- Pass a precise self-contained objective; the subagent does not see your context\n\
{recipe_guidance}\n\
- For multiple independent subtasks, delegate them in sequence; each run is durable\n\
- Trust the structured result; do not re-do the subagent's work in your turn\n\n\
Avoid delegating when:\n\n\
- The action is a simple tool call you can do directly (read file, run grep)\n\
- You need streaming intermediate state in your own context window\n\
- The work is shorter than the delegation overhead"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidelines_render_core_delegation_signal() {
        let prompt = subagent_delegation_guidelines(3);

        assert!(prompt.starts_with(DELEGATION_GUIDELINES_HEADING));
        assert!(prompt.contains("\"subagent\" tool"));
        assert!(prompt.contains("Delegate when at least one of these is true"));
        assert!(prompt.contains("Trust the structured result"));
        assert!(prompt.contains("Prefer a recipeId"));
    }

    #[test]
    fn guidelines_do_not_render_recipe_list() {
        let prompt = subagent_delegation_guidelines(3);

        assert!(!prompt.contains("Available recipes"));
        assert!(!prompt.contains("debug-native-subagent"));
    }

    #[test]
    fn guidelines_handle_empty_recipe_registry() {
        let prompt = subagent_delegation_guidelines(0);

        assert!(prompt.contains("no recipes are registered"));
        assert!(!prompt.contains("Prefer a recipeId"));
    }
}
