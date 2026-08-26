use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{DaemonSessionOpenParams, WorkspacePath, WorkspaceSelector};

use crate::{
    args::{GlobalArgs, SessionCommands},
    error::CliError,
    output::CommandOutput,
};

use super::workspace::resolve_workspace_path_input;

pub fn run(
    daemon_client: &DaemonClient,
    command: SessionCommands,
    global: &GlobalArgs,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        SessionCommands::List => {
            let mut client =
                daemon_client.connect_persistent("ta-cli", env!("CARGO_PKG_VERSION"))?;
            let sessions = client.list_sessions()?;
            Ok(Some(CommandOutput::SessionList(sessions)))
        }
        SessionCommands::Open { title, workspace } => {
            let workspace_path = resolve_workspace_path(workspace)?;
            let trust_acknowledged = should_trust_workspace(global, workspace_path.as_str());
            let mut client =
                daemon_client.connect_persistent("ta-cli", env!("CARGO_PKG_VERSION"))?;
            let session = client
                .open_session(DaemonSessionOpenParams {
                    title,
                    workspace: WorkspaceSelector::ByPath {
                        path: workspace_path,
                        trust_acknowledged,
                    },
                })?
                .session;
            Ok(Some(CommandOutput::SessionOpen(session)))
        }
    }
}

fn resolve_workspace_path(path: Option<String>) -> Result<WorkspacePath, CliError> {
    resolve_workspace_path_input(path.unwrap_or_else(|| ".".to_string()))
}

fn should_trust_workspace(global: &GlobalArgs, workspace_path: &str) -> bool {
    global.trust_all_workspaces
        || global.trust_workspaces.iter().any(|trusted| {
            resolve_workspace_path_input(trusted)
                .map(|path| path.as_str() == workspace_path)
                .unwrap_or(false)
        })
}
