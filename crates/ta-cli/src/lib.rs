//! Command-line interface for Taugentic.

mod args;
mod commands;
mod daemon_ops;
mod defaults;
mod error;
mod output;

use std::ffi::OsString;

use clap::Parser;
use ta_daemon_client::DaemonClient;

pub use error::CliError;

use crate::args::Cli;

pub fn run_env() -> Result<(), CliError> {
    run(std::env::args_os())
}

pub fn run<I, T>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    let daemon_client = DaemonClient::new(cli.global.socket.as_deref());
    let output = commands::run(&daemon_client, cli.command, cli.global.output_format())?;
    if let Some(output) = output {
        output::print(&output, cli.global.output_format())?;
    }
    Ok(())
}
