pub(super) struct BuiltinRecipeSource {
    pub(super) path: &'static str,
    pub(super) contents: &'static str,
}

pub(super) const BUILTIN_RECIPE_SOURCES: [BuiltinRecipeSource; 5] = [
    BuiltinRecipeSource {
        path: "crates/ta-orchestrator/recipes/builtin/debug-agent.toml",
        contents: include_str!("../../../recipes/builtin/debug-agent.toml"),
    },
    BuiltinRecipeSource {
        path: "crates/ta-orchestrator/recipes/builtin/patch-agent.toml",
        contents: include_str!("../../../recipes/builtin/patch-agent.toml"),
    },
    BuiltinRecipeSource {
        path: "crates/ta-orchestrator/recipes/builtin/review-agent.toml",
        contents: include_str!("../../../recipes/builtin/review-agent.toml"),
    },
    BuiltinRecipeSource {
        path: "crates/ta-orchestrator/recipes/builtin/test-agent.toml",
        contents: include_str!("../../../recipes/builtin/test-agent.toml"),
    },
    BuiltinRecipeSource {
        path: "crates/ta-orchestrator/recipes/builtin/plan-agent.toml",
        contents: include_str!("../../../recipes/builtin/plan-agent.toml"),
    },
];
