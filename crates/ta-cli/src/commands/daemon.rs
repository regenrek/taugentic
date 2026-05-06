use std::time::Duration;

use ta_daemon_client::DaemonClient;

use crate::{
    args::{DaemonBackgroundCommands, DaemonCommands, DaemonWaitOptions},
    daemon_ops::{
        disable_background_mode, enable_background_mode, read_background_status,
        read_daemon_status, read_logs, reconcile_runtime_control, restart_daemon, start_daemon,
        stop_configured_daemon, wait_for_daemon, watch_daemon_status,
    },
    defaults::{default_poll_interval, default_wait_timeout},
    error::CliError,
    output::{self, CommandOutput, OutputFormat},
};

pub fn run(
    daemon_client: &DaemonClient,
    command: DaemonCommands,
    format: OutputFormat,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        DaemonCommands::Status { options } => {
            if options.watch {
                watch_daemon_status(
                    daemon_client,
                    Duration::from_millis(options.interval_ms.max(1)),
                    options.count,
                    |poll| output::print(&CommandOutput::DaemonStatusPoll(poll), format),
                )?;
                return Ok(None);
            }
            let status = read_daemon_status(daemon_client)?;
            Ok(Some(CommandOutput::DaemonStatus(status)))
        }
        DaemonCommands::Start { options } => {
            let result = start_daemon(daemon_client, timeout(&options), interval(&options))?;
            Ok(Some(CommandOutput::DaemonStart(result)))
        }
        DaemonCommands::Wait { options } => {
            let result = wait_for_daemon(daemon_client, timeout(&options), interval(&options))?;
            Ok(Some(CommandOutput::DaemonWait(result)))
        }
        DaemonCommands::Restart { options } => {
            let result = restart_daemon(daemon_client, timeout(&options), interval(&options))?;
            Ok(Some(CommandOutput::DaemonRestart(result)))
        }
        DaemonCommands::Logs { options } => {
            let result = read_logs(daemon_client, options.tail)?;
            Ok(Some(CommandOutput::DaemonLogs(result)))
        }
        DaemonCommands::Stop => {
            let (timeout, interval) = default_control_timing();
            let result = stop_configured_daemon(daemon_client, timeout, interval)?;
            Ok(Some(CommandOutput::DaemonStop(result)))
        }
        DaemonCommands::Background { command } => match command {
            DaemonBackgroundCommands::Status => {
                let result = read_background_status(daemon_client)?;
                Ok(Some(CommandOutput::DaemonBackgroundStatus(result)))
            }
            DaemonBackgroundCommands::Enable => {
                let (timeout, interval) = default_control_timing();
                let result = enable_background_mode(daemon_client, timeout, interval)?;
                Ok(Some(CommandOutput::DaemonBackgroundStatus(result)))
            }
            DaemonBackgroundCommands::Disable => {
                let (timeout, interval) = default_control_timing();
                let result = disable_background_mode(daemon_client, timeout, interval)?;
                Ok(Some(CommandOutput::DaemonBackgroundStatus(result)))
            }
            DaemonBackgroundCommands::Reconcile => {
                let (timeout, interval) = default_control_timing();
                let result = reconcile_runtime_control(daemon_client, timeout, interval)?;
                Ok(Some(CommandOutput::DaemonBackgroundStatus(result)))
            }
        },
    }
}

fn timeout(options: &DaemonWaitOptions) -> Duration {
    Duration::from_millis(options.timeout_ms.max(1))
}

fn interval(options: &DaemonWaitOptions) -> Duration {
    Duration::from_millis(options.interval_ms.max(1))
}

fn default_control_timing() -> (Duration, Duration) {
    (default_wait_timeout(), default_poll_interval())
}
