use ta_protocol::wire::{
    RuntimeExtensionAvailability, RuntimeExtensionDescriptor, RuntimeExtensionId,
    RuntimeExtensionState,
};

use crate::orchestration::agent_runtime::service::AgentRuntimeServiceError;

pub(crate) fn built_in_extensions() -> Vec<RuntimeExtensionState> {
    vec![RuntimeExtensionState {
        descriptor: RuntimeExtensionDescriptor {
            id: RuntimeExtensionId::new("local-shell-tools").expect("extension id"),
            display_name: "Local Shell Tools".to_string(),
            description: "Builtin local shell execution support".to_string(),
        },
        availability: RuntimeExtensionAvailability::Available,
        enabled: true,
        mcp_server: None,
    }]
}

pub(crate) fn set_extension_enabled(
    extensions: &mut [RuntimeExtensionState],
    extension_id: &RuntimeExtensionId,
    enabled: bool,
) -> Result<(), AgentRuntimeServiceError> {
    let extension = extensions
        .iter_mut()
        .find(|extension| extension.descriptor.id == *extension_id)
        .ok_or_else(|| {
            AgentRuntimeServiceError::RuntimeExtensionNotFound(extension_id.as_str().to_string())
        })?;
    extension.enabled = enabled;
    Ok(())
}
