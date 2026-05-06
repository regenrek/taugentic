use ta_protocol::wire::RuntimeProfileSummary;

use crate::orchestration::agent_runtime::strategy_registry::StrategyRegistry;

pub(crate) fn built_in_runtime_profiles(registry: &StrategyRegistry) -> Vec<RuntimeProfileSummary> {
    registry.default_runtime_profiles()
}
