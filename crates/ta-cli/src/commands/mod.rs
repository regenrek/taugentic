mod approval;
mod daemon;
mod run;
mod session;
mod workspace;

use crate::{
    args::{Commands, GlobalArgs},
    error::CliError,
    output::{CommandOutput, OutputFormat},
};
use ta_daemon_client::DaemonClient;

pub fn run(
    daemon_client: &DaemonClient,
    command: Commands,
    global: &GlobalArgs,
    format: OutputFormat,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        Commands::Daemon { command } => daemon::run(daemon_client, command, format),
        Commands::Session { command } => session::run(daemon_client, command, global),
        Commands::Approval { command } => approval::run(daemon_client, command),
        Commands::Run { command } => run::run(daemon_client, command),
        Commands::Workspace { command } => workspace::run(daemon_client, command),
    }
}
