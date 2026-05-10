use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{
    AgentRuntimeModelId, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeTestLocalEndpointParams, LocalModelApiStandard, LocalModelAuthMode,
    LocalModelEndpointConfig, RuntimeProfileId, RuntimeProfileLocalEndpointPatch,
    RuntimeProfileModelIdPatch, RuntimeProfilePatch,
};

use crate::{
    args::{AgentRuntimeCommands, AgentRuntimeLocalCommands, AgentRuntimeLocalEndpointOptions},
    error::CliError,
    output::CommandOutput,
};

const CLI_CLIENT_NAME: &str = "ta-cli";
const CLI_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CUSTOM_LOCAL_PROFILE_ID: &str = "runtime-local-custom";

pub fn run(
    daemon_client: &DaemonClient,
    command: AgentRuntimeCommands,
) -> Result<Option<CommandOutput>, CliError> {
    let mut client = daemon_client.connect_persistent(CLI_CLIENT_NAME, CLI_CLIENT_VERSION)?;
    match command {
        AgentRuntimeCommands::List => {
            let snapshot = client.get_agent_runtime()?;
            Ok(Some(CommandOutput::AgentRuntimeSnapshot(snapshot)))
        }
        AgentRuntimeCommands::Local { command } => match command {
            AgentRuntimeLocalCommands::Add { options } => {
                let endpoint = endpoint_config(options)?;
                let model_id = endpoint.default_model.clone();
                let snapshot =
                    client.patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
                        runtime_profile_id: runtime_profile_id(CUSTOM_LOCAL_PROFILE_ID)?,
                        patch: RuntimeProfilePatch {
                            model_id: model_id
                                .map(|value| RuntimeProfileModelIdPatch::Set { value }),
                            local_endpoint: Some(RuntimeProfileLocalEndpointPatch::Set {
                                value: endpoint,
                            }),
                            ..Default::default()
                        },
                    })?;
                Ok(Some(CommandOutput::AgentRuntimeSnapshot(snapshot)))
            }
            AgentRuntimeLocalCommands::Test { options, tool_call } => {
                let endpoint = endpoint_config(options)?;
                let result = client.test_local_model_endpoint(
                    DaemonAgentRuntimeTestLocalEndpointParams {
                        model_id: endpoint.default_model.clone(),
                        endpoint,
                        test_tool_call: tool_call,
                    },
                )?;
                Ok(Some(CommandOutput::LocalModelEndpointTest(result)))
            }
            AgentRuntimeLocalCommands::Remove { profile } => {
                let snapshot =
                    client.patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
                        runtime_profile_id: runtime_profile_id(&profile)?,
                        patch: RuntimeProfilePatch {
                            model_id: Some(RuntimeProfileModelIdPatch::Clear),
                            local_endpoint: Some(RuntimeProfileLocalEndpointPatch::Clear),
                            ..Default::default()
                        },
                    })?;
                Ok(Some(CommandOutput::AgentRuntimeSnapshot(snapshot)))
            }
            AgentRuntimeLocalCommands::SetModel { profile, model } => {
                let snapshot =
                    client.patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
                        runtime_profile_id: runtime_profile_id(&profile)?,
                        patch: RuntimeProfilePatch {
                            model_id: Some(RuntimeProfileModelIdPatch::Set {
                                value: model_id(&model)?,
                            }),
                            ..Default::default()
                        },
                    })?;
                Ok(Some(CommandOutput::AgentRuntimeSnapshot(snapshot)))
            }
        },
    }
}

fn endpoint_config(
    options: AgentRuntimeLocalEndpointOptions,
) -> Result<LocalModelEndpointConfig, CliError> {
    Ok(LocalModelEndpointConfig {
        base_url: options.base_url,
        api_standard: api_standard(&options.standard)?,
        auth_mode: auth_mode(&options.auth_mode)?,
        api_key_env: options.api_key_env,
        default_model: options.model.as_deref().map(model_id).transpose()?,
        model_discovery: options.model_discovery,
        capabilities: None,
    })
}

fn api_standard(value: &str) -> Result<LocalModelApiStandard, CliError> {
    match value {
        "openai-chat-completions" => Ok(LocalModelApiStandard::OpenAiChatCompletions),
        "ollama-openai" => Ok(LocalModelApiStandard::OllamaOpenAi),
        "lm-studio-openai" => Ok(LocalModelApiStandard::LmStudioOpenAi),
        "llama-cpp-openai" => Ok(LocalModelApiStandard::LlamaCppOpenAi),
        "vllm-openai" => Ok(LocalModelApiStandard::VllmOpenAi),
        "tgi-messages" => Ok(LocalModelApiStandard::TgiMessages),
        _ => Err(CliError::InvalidInput(format!(
            "unsupported local endpoint standard: {value}"
        ))),
    }
}

fn auth_mode(value: &str) -> Result<LocalModelAuthMode, CliError> {
    match value {
        "none" => Ok(LocalModelAuthMode::None),
        "bearer-env" => Ok(LocalModelAuthMode::BearerEnv),
        _ => Err(CliError::InvalidInput(format!(
            "unsupported local endpoint auth mode: {value}"
        ))),
    }
}

fn runtime_profile_id(value: &str) -> Result<RuntimeProfileId, CliError> {
    RuntimeProfileId::new(value).map_err(|error| CliError::InvalidInput(error.to_string()))
}

fn model_id(value: &str) -> Result<AgentRuntimeModelId, CliError> {
    AgentRuntimeModelId::new(value).map_err(|error| CliError::InvalidInput(error.to_string()))
}
