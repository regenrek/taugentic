use ta_daemon_client::DaemonClient;
use ta_protocol::wire::WorkspaceId;

use crate::{args::SessionCommands, error::CliError, output::CommandOutput};

pub fn run(
    daemon_client: &DaemonClient,
    command: SessionCommands,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        SessionCommands::List => {
            let mut client =
                daemon_client.connect_persistent("ta-cli", env!("CARGO_PKG_VERSION"))?;
            let sessions = client.list_sessions()?;
            Ok(Some(CommandOutput::SessionList(sessions)))
        }
        SessionCommands::Open {
            title,
            workspace_id,
        } => {
            let workspace_id = WorkspaceId::new(workspace_id)
                .map_err(|error| CliError::InvalidArgument(error.to_string()))?;
            let mut client =
                daemon_client.connect_persistent("ta-cli", env!("CARGO_PKG_VERSION"))?;
            let session = client.open_session(&title, workspace_id)?.session;
            Ok(Some(CommandOutput::SessionOpen(session)))
        }
    }
}
