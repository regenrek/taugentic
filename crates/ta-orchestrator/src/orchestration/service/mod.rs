mod active_execution;
mod run_execution;

#[cfg(test)]
mod tests;

use ta_host_platform::{HostPlatform, detect_current_platform};
use ta_store::EventRecord;
use uuid::Uuid;

pub(crate) use crate::host::event_hub::to_event_cursor;
use crate::host::event_hub::{RuntimeEventHub, RuntimeEventPublisher, RuntimeEventSubscription};
use crate::orchestration::agent_runtime::{
    StrategyRegistry, built_in_agent_runtime_strategies, built_in_runtime_profiles,
};
use crate::{
    AgentRuntimeRuntime, DaemonEventCursor, DaemonEventEnvelope, DaemonEventKind, LaneCapabilities,
    SessionId,
};

pub use run_execution::RuntimeExecutionPaths;
#[allow(unused_imports)]
pub use run_execution::execute_run;
pub(crate) use run_execution::{ProviderRunStart, RunExecutionRuntime};

#[derive(Debug, Clone)]
pub struct RuntimeService {
    #[cfg_attr(not(test), allow(dead_code))]
    pub host_platform: HostPlatform,
    daemon_instance_id: String,
    event_hub: RuntimeEventHub,
    agent_runtime: AgentRuntimeRuntime,
    agent_runtime_strategy_registry: StrategyRegistry,
    run_execution: RunExecutionRuntime,
}

impl RuntimeService {
    pub fn bootstrap() -> Self {
        Self::from_host_platform_with_paths(
            detect_current_platform(),
            RuntimeExecutionPaths::from_current_process(),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_host_platform(host_platform: HostPlatform) -> Self {
        Self::from_host_platform_with_paths(
            host_platform,
            RuntimeExecutionPaths::from_current_process(),
        )
    }

    pub fn from_host_platform_with_paths(
        host_platform: HostPlatform,
        execution_paths: RuntimeExecutionPaths,
    ) -> Self {
        let capabilities = LaneCapabilities::from_host_platform(&host_platform);
        let event_hub = RuntimeEventHub::new();
        let daemon_instance_id = Uuid::new_v4().to_string();
        let event_publisher =
            RuntimeEventPublisher::new(daemon_instance_id.clone(), event_hub.clone());
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                if let Err(error) =
                    ta_provider_llm::auth::openai::initialize_default_subscription_auth(runtime)
                {
                    tracing::warn!(
                        error = %error,
                        "OpenAI ChatGPT subscription auth did not initialize during daemon bootstrap"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "OpenAI ChatGPT subscription auth skipped because daemon bootstrap has no Tokio runtime"
                );
            }
        }
        let agent_runtime_strategy_registry =
            StrategyRegistry::from_registered(built_in_agent_runtime_strategies())
                .expect("provider registry should initialize");
        let agent_runtime =
            AgentRuntimeRuntime::new(built_in_runtime_profiles(&agent_runtime_strategy_registry));
        Self {
            host_platform,
            daemon_instance_id,
            event_hub,
            agent_runtime: agent_runtime.clone(),
            agent_runtime_strategy_registry: agent_runtime_strategy_registry.clone(),
            run_execution: RunExecutionRuntime::new(
                capabilities,
                agent_runtime,
                agent_runtime_strategy_registry,
                event_publisher,
                execution_paths,
            ),
        }
    }

    pub fn daemon_instance_id(&self) -> String {
        self.daemon_instance_id.clone()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn latest_cursor_for_session(&self, session_id: &SessionId) -> Option<DaemonEventCursor> {
        self.event_hub
            .latest_cursor_for_session(&self.daemon_instance_id, session_id)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn publish_record(&self, record: &EventRecord) -> DaemonEventEnvelope {
        self.event_hub.publish(&self.daemon_instance_id, record)
    }

    pub fn subscribe_events(
        &self,
        session_id: &SessionId,
        kinds: &[DaemonEventKind],
        latest_persisted_sequence: Option<u64>,
        after_cursor: Option<&DaemonEventCursor>,
    ) -> RuntimeEventSubscription {
        self.event_hub.subscribe(
            &self.daemon_instance_id,
            session_id,
            kinds,
            latest_persisted_sequence,
            after_cursor,
        )
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count_for_session(&self, session_id: &SessionId) -> usize {
        self.event_hub.subscriber_count_for_session(session_id)
    }

    pub fn capabilities(&self) -> &LaneCapabilities {
        &self.run_execution.capabilities
    }

    pub(crate) fn run_execution_runtime(&self) -> RunExecutionRuntime {
        self.run_execution.clone()
    }

    pub(crate) fn agent_runtime_runtime(&self) -> AgentRuntimeRuntime {
        self.agent_runtime.clone()
    }

    pub(crate) fn agent_runtime_strategy_registry(&self) -> StrategyRegistry {
        self.agent_runtime_strategy_registry.clone()
    }
}
