use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeSelection, AuthProfileId, ListRunsQuery, RuntimeProfileId,
    SessionId, StartRunCommand,
};

use crate::{args::RunCommands, error::CliError, output::CommandOutput};

const CLI_CLIENT_NAME: &str = "ta-cli";
const CLI_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(
    daemon_client: &DaemonClient,
    command: RunCommands,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        RunCommands::List { session } => {
            let session_id = parse_session_id(session)?;
            let mut client =
                daemon_client.connect_persistent(CLI_CLIENT_NAME, CLI_CLIENT_VERSION)?;
            let _ = client.attach_session(session_id)?;
            let runs = client.list_runs(ListRunsQuery {})?;
            Ok(Some(CommandOutput::RunList(runs)))
        }
        RunCommands::Start {
            session,
            runtime_profile,
            model,
            auth_profile,
            objective,
        } => {
            let session_id = parse_session_id(session)?;
            let selection = AgentRuntimeSelection {
                runtime_profile_id: RuntimeProfileId::new(runtime_profile)
                    .map_err(|error| CliError::InvalidInput(error.to_string()))?,
                auth_profile_id: auth_profile
                    .map(AuthProfileId::new)
                    .transpose()
                    .map_err(|error| CliError::InvalidInput(error.to_string()))?,
                model_id: Some(
                    AgentRuntimeModelId::new(model)
                        .map_err(|error| CliError::InvalidInput(error.to_string()))?,
                ),
            };
            let mut client =
                daemon_client.connect_persistent(CLI_CLIENT_NAME, CLI_CLIENT_VERSION)?;
            let _ = client.attach_session(session_id.clone())?;
            let run = client.start_run(StartRunCommand::new(objective, selection))?;
            Ok(Some(CommandOutput::RunStart(run)))
        }
    }
}

fn parse_session_id(value: String) -> Result<SessionId, CliError> {
    SessionId::new(value).map_err(|error| CliError::InvalidInput(error.to_string()))
}
