use serde::{Deserialize, Serialize};
use serde_json::Value;
use ta_protocol::wire::{
    RuntimeExtensionAvailability, RuntimeExtensionMcpServer, RuntimeExtensionState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AcpMcpServerSpec {
    Stdio(AcpMcpStdioServer),
    Http(AcpMcpHttpServer),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpMcpStdioServer {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<AcpEnvVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpMcpHttpServer {
    #[serde(rename = "type")]
    pub transport_type: AcpMcpHttpTransportType,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<AcpHttpHeader>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcpMcpHttpTransportType {
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpEnvVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpHttpHeader {
    pub name: String,
    pub value: String,
}

pub fn extension_to_mcp_server(extension: &RuntimeExtensionState) -> Option<AcpMcpServerSpec> {
    if !extension.enabled || extension.availability != RuntimeExtensionAvailability::Available {
        return None;
    }
    match extension.mcp_server.as_ref()? {
        RuntimeExtensionMcpServer::Stdio(server) => {
            Some(AcpMcpServerSpec::Stdio(AcpMcpStdioServer {
                name: server.name.clone(),
                command: server.command.clone(),
                args: server.args.clone(),
                env: server
                    .env
                    .iter()
                    .map(|item| AcpEnvVariable {
                        name: item.name.clone(),
                        value: item.value.clone(),
                    })
                    .collect(),
            }))
        }
        RuntimeExtensionMcpServer::Http(server) => Some(AcpMcpServerSpec::Http(AcpMcpHttpServer {
            transport_type: AcpMcpHttpTransportType::Http,
            name: server.name.clone(),
            url: server.url.clone(),
            headers: server
                .headers
                .iter()
                .map(|item| AcpHttpHeader {
                    name: item.name.clone(),
                    value: item.value.clone(),
                })
                .collect(),
        })),
    }
}

pub fn extensions_to_mcp_servers(extensions: &[RuntimeExtensionState]) -> Vec<AcpMcpServerSpec> {
    extensions
        .iter()
        .filter_map(extension_to_mcp_server)
        .collect()
}

pub fn extensions_include_http_mcp(extensions: &[RuntimeExtensionState]) -> bool {
    extensions
        .iter()
        .filter_map(extension_to_mcp_server)
        .any(|server| matches!(server, AcpMcpServerSpec::Http(_)))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AcpMcpCapabilities {
    http: bool,
}

impl AcpMcpCapabilities {
    pub fn from_initialize_result(result: &Value) -> Self {
        Self {
            http: result
                .pointer("/agentCapabilities/mcpCapabilities/http")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

pub fn filter_supported_mcp_servers(
    servers: &[AcpMcpServerSpec],
    capabilities: &AcpMcpCapabilities,
) -> Vec<AcpMcpServerSpec> {
    servers
        .iter()
        .filter(|server| match server {
            AcpMcpServerSpec::Http(http) if !capabilities.http => {
                tracing::debug!(
                    name = http.name,
                    "skipping HTTP MCP server, agent lacks capability"
                );
                false
            }
            _ => true,
        })
        .cloned()
        .collect()
}
