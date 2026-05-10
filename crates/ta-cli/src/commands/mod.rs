mod agent_runtime;
mod approval;
mod daemon;
mod run;
mod session;

use crate::{
    args::Commands,
    error::CliError,
    output::{CommandOutput, OutputFormat},
};
use ta_daemon_client::DaemonClient;

pub fn run(
    daemon_client: &DaemonClient,
    command: Commands,
    format: OutputFormat,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        Commands::Daemon { command } => daemon::run(daemon_client, command, format),
        Commands::Session { command } => session::run(daemon_client, command),
        Commands::Approval { command } => approval::run(daemon_client, command),
        Commands::Run { command } => run::run(daemon_client, command),
        Commands::AgentRuntime { command } => agent_runtime::run(daemon_client, command),
    }
}
