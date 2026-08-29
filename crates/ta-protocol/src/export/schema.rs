use std::{fs, path::Path};

use schemars::{JsonSchema, schema_for};
use serde_json::json;

use crate::wire::*;

use super::schema_core::write_core_schemas;
use super::schema_core_runtime::build_core_runtime_json_schemas;
use super::{PROTOCOL_VERSION, ProtocolExportError};

pub fn export_json_schemas(shared_package_dir: &Path) -> Result<(), ProtocolExportError> {
    let schema_dir = shared_package_dir.join("generated/schema");
    fs::create_dir_all(&schema_dir)?;

    write_core_schemas(&schema_dir)?;
    write_agent_runtime_schemas(&schema_dir)?;
    write_protocol_version_schema(&schema_dir)?;

    Ok(())
}

pub(super) fn build_runtime_json_schemas()
-> Result<Vec<(&'static str, serde_json::Value)>, ProtocolExportError> {
    let mut schemas = build_core_runtime_json_schemas()?;
    schemas.extend(agent_runtime_runtime_schemas()?);
    Ok(schemas)
}

pub(super) struct JsonSchemaPublicType {
    pub(super) write: fn(&Path) -> Result<(), ProtocolExportError>,
    pub(super) runtime: fn() -> Result<(&'static str, serde_json::Value), ProtocolExportError>,
}

pub(super) fn write_schema<T: JsonSchema>(schema_dir: &Path) -> Result<(), ProtocolExportError> {
    let schema_path = schema_dir.join(format!("{}.json", simple_type_name::<T>()));
    let schema_json = serde_json::to_string_pretty(&schema_value::<T>()?)?;
    fs::write(schema_path, format!("{schema_json}\n"))?;
    Ok(())
}

pub(super) fn write_selected_schemas(
    schemas: &[JsonSchemaPublicType],
    schema_dir: &Path,
) -> Result<(), ProtocolExportError> {
    for schema in schemas {
        (schema.write)(schema_dir)?;
    }
    Ok(())
}

pub(super) fn selected_runtime_schemas(
    schemas: &[JsonSchemaPublicType],
) -> Result<Vec<(&'static str, serde_json::Value)>, ProtocolExportError> {
    schemas.iter().map(|schema| (schema.runtime)()).collect()
}

fn write_agent_runtime_schemas(schema_dir: &Path) -> Result<(), ProtocolExportError> {
    write_schema::<AgentRuntimeStrategyId>(schema_dir)?;
    write_schema::<AgentRuntimeModelId>(schema_dir)?;
    write_schema::<AgentRuntimeModelRef>(schema_dir)?;
    write_schema::<AgentRuntimeMediaCapability>(schema_dir)?;
    write_schema::<AgentRuntimeMediaCapabilities>(schema_dir)?;
    write_schema::<AgentRuntimeStrategyHealthStatus>(schema_dir)?;
    write_schema::<AgentRuntimeStrategyHealth>(schema_dir)?;
    write_schema::<AgentRuntimeStrategyInfo>(schema_dir)?;
    write_schema::<AuthMethodId>(schema_dir)?;
    write_schema::<AuthMethodRef>(schema_dir)?;
    write_schema::<AuthProfileId>(schema_dir)?;
    write_schema::<AuthProfileConnectionState>(schema_dir)?;
    write_schema::<AuthProfileLoginMethod>(schema_dir)?;
    write_schema::<AuthProfileRef>(schema_dir)?;
    write_schema::<AuthProfilePreferences>(schema_dir)?;
    write_schema::<AuthProfileUsage>(schema_dir)?;
    write_schema::<AuthProfileUsageWindow>(schema_dir)?;
    write_schema::<AuthProfileState>(schema_dir)?;
    write_schema::<AuthProfileLoginChallenge>(schema_dir)?;
    write_schema::<AuthProfileLoginResult>(schema_dir)?;
    write_schema::<AuthProfileLogoutResult>(schema_dir)?;
    write_schema::<RuntimeExtensionId>(schema_dir)?;
    write_schema::<RuntimeExtensionDescriptor>(schema_dir)?;
    write_schema::<RuntimeExtensionAvailability>(schema_dir)?;
    write_schema::<RuntimeExtensionMcpServer>(schema_dir)?;
    write_schema::<RuntimeExtensionMcpStdioServer>(schema_dir)?;
    write_schema::<RuntimeExtensionMcpHttpServer>(schema_dir)?;
    write_schema::<RuntimeExtensionEnvVar>(schema_dir)?;
    write_schema::<RuntimeExtensionHttpHeader>(schema_dir)?;
    write_schema::<RuntimeExtensionState>(schema_dir)?;
    write_schema::<RuntimeProfileId>(schema_dir)?;
    write_schema::<RuntimePolicyMode>(schema_dir)?;
    write_schema::<RuntimeProfileExecutionKind>(schema_dir)?;
    write_schema::<RuntimeProfileSummary>(schema_dir)?;
    write_schema::<RuntimeProfilePatch>(schema_dir)?;
    write_schema::<AgentRuntimeSelection>(schema_dir)?;
    write_schema::<AgentRuntimeSnapshot>(schema_dir)?;
    write_schema::<GetAgentRuntimeQuery>(schema_dir)?;
    write_schema::<DaemonAgentRuntimePatchProfileParams>(schema_dir)?;
    write_schema::<DaemonAgentRuntimeAuthLoginParams>(schema_dir)?;
    write_schema::<DaemonAgentRuntimeAuthLoginCompleteParams>(schema_dir)?;
    write_schema::<DaemonAgentRuntimeAuthLogoutParams>(schema_dir)?;
    write_schema::<DaemonAgentRuntimeSetExtensionEnabledParams>(schema_dir)?;
    Ok(())
}

fn agent_runtime_runtime_schemas()
-> Result<Vec<(&'static str, serde_json::Value)>, ProtocolExportError> {
    Ok(vec![
        schema_pair::<AgentRuntimeStrategyId>()?,
        schema_pair::<AgentRuntimeModelId>()?,
        schema_pair::<AgentRuntimeModelRef>()?,
        schema_pair::<AgentRuntimeMediaCapability>()?,
        schema_pair::<AgentRuntimeMediaCapabilities>()?,
        schema_pair::<AgentRuntimeStrategyHealthStatus>()?,
        schema_pair::<AgentRuntimeStrategyHealth>()?,
        schema_pair::<AgentRuntimeStrategyInfo>()?,
        schema_pair::<AuthMethodId>()?,
        schema_pair::<AuthMethodRef>()?,
        schema_pair::<AuthProfileId>()?,
        schema_pair::<AuthProfileConnectionState>()?,
        schema_pair::<AuthProfileLoginMethod>()?,
        schema_pair::<AuthProfileRef>()?,
        schema_pair::<AuthProfilePreferences>()?,
        schema_pair::<AuthProfileUsage>()?,
        schema_pair::<AuthProfileUsageWindow>()?,
        schema_pair::<AuthProfileState>()?,
        schema_pair::<AuthProfileLoginChallenge>()?,
        schema_pair::<AuthProfileLoginResult>()?,
        schema_pair::<AuthProfileLogoutResult>()?,
        schema_pair::<RuntimeExtensionId>()?,
        schema_pair::<RuntimeExtensionDescriptor>()?,
        schema_pair::<RuntimeExtensionAvailability>()?,
        schema_pair::<RuntimeExtensionMcpServer>()?,
        schema_pair::<RuntimeExtensionMcpStdioServer>()?,
        schema_pair::<RuntimeExtensionMcpHttpServer>()?,
        schema_pair::<RuntimeExtensionEnvVar>()?,
        schema_pair::<RuntimeExtensionHttpHeader>()?,
        schema_pair::<RuntimeExtensionState>()?,
        schema_pair::<RuntimeProfileId>()?,
        schema_pair::<RuntimePolicyMode>()?,
        schema_pair::<RuntimeProfileExecutionKind>()?,
        schema_pair::<RuntimeProfileSummary>()?,
        schema_pair::<RuntimeProfilePatch>()?,
        schema_pair::<AgentRuntimeSelection>()?,
        schema_pair::<AgentRuntimeSnapshot>()?,
        schema_pair::<GetAgentRuntimeQuery>()?,
        schema_pair::<DaemonAgentRuntimePatchProfileParams>()?,
        schema_pair::<DaemonAgentRuntimeAuthLoginParams>()?,
        schema_pair::<DaemonAgentRuntimeAuthLoginCompleteParams>()?,
        schema_pair::<DaemonAgentRuntimeAuthLogoutParams>()?,
        schema_pair::<DaemonAgentRuntimeSetExtensionEnabledParams>()?,
    ])
}

pub(super) fn schema_pair<T: JsonSchema>()
-> Result<(&'static str, serde_json::Value), ProtocolExportError> {
    Ok((simple_type_name::<T>(), schema_value::<T>()?))
}

fn write_protocol_version_schema(schema_dir: &Path) -> Result<(), ProtocolExportError> {
    let schema = protocol_version_schema_value();
    let schema_json = serde_json::to_string_pretty(&schema)?;
    fs::write(
        schema_dir.join("ProtocolVersion.json"),
        format!("{schema_json}\n"),
    )?;
    Ok(())
}

fn schema_value<T: JsonSchema>() -> Result<serde_json::Value, ProtocolExportError> {
    Ok(serde_json::to_value(schema_for!(T))?)
}

pub(super) fn protocol_version_schema_value() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ProtocolVersion",
        "type": "string",
        "const": PROTOCOL_VERSION,
    })
}

fn simple_type_name<T>() -> &'static str {
    std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .expect("type name should be present")
}
