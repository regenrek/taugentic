use clap::{Args, Parser, Subcommand};

use crate::{
    defaults::{
        DEFAULT_LOG_TAIL_LINES, DEFAULT_POLL_INTERVAL_MS, DEFAULT_WAIT_TIMEOUT_MS,
        DEFAULT_WATCH_INTERVAL_MS,
    },
    output::OutputFormat,
};

#[derive(Debug, Parser)]
#[command(
    name = "ta",
    bin_name = "ta",
    version,
    about = "Taugentic command-line interface",
    long_about = None,
    propagate_version = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    /// Emit machine-readable JSON to stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Override the daemon socket name. Falls back to TAUGENTIC_DAEMON_SOCKET_NAME, then the default.
    #[arg(long, global = true, value_name = "SOCKET_NAME")]
    pub socket: Option<String>,
}

impl GlobalArgs {
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Inspect and manage the local daemon.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
    /// Read or create daemon-owned sessions.
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    /// Read or mutate daemon-owned approvals.
    Approval {
        #[command(subcommand)]
        command: ApprovalCommands,
    },
    /// Read or mutate daemon-owned runs.
    Run {
        #[command(subcommand)]
        command: RunCommands,
    },
    /// Inspect and configure agent runtimes.
    AgentRuntime {
        #[command(subcommand)]
        command: AgentRuntimeCommands,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum DaemonCommands {
    /// Show daemon readiness, socket path, and version.
    Status {
        #[command(flatten)]
        options: DaemonStatusOptions,
    },
    /// Start the local daemon and wait for readiness.
    Start {
        #[command(flatten)]
        options: DaemonWaitOptions,
    },
    /// Wait until the local daemon responds to status checks.
    Wait {
        #[command(flatten)]
        options: DaemonWaitOptions,
    },
    /// Stop and then start the local daemon.
    Restart {
        #[command(flatten)]
        options: DaemonWaitOptions,
    },
    /// Show the CLI-managed daemon log file.
    Logs {
        #[command(flatten)]
        options: DaemonLogsOptions,
    },
    /// Request a graceful daemon shutdown.
    Stop,
    /// Inspect and control explicit background mode.
    Background {
        #[command(subcommand)]
        command: DaemonBackgroundCommands,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum DaemonBackgroundCommands {
    /// Show configured background mode and service state.
    Status,
    /// Persist background mode and enable the OS-managed service.
    Enable,
    /// Disable the OS-managed background service and return to local default mode.
    Disable,
    /// Resume or repair a degraded runtime-control transition.
    Reconcile,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommands {
    /// List daemon-owned sessions.
    List,
    /// Open a new daemon-owned session.
    Open {
        #[arg(value_name = "TITLE")]
        title: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ApprovalCommands {
    /// List open daemon-owned approvals for a session.
    List {
        #[arg(long, value_name = "SESSION_ID")]
        session: String,
    },
    /// Decide an existing approval in an attached session.
    Decide {
        #[arg(long, value_name = "SESSION_ID")]
        session: String,
        #[arg(long, value_name = "APPROVAL_ID")]
        approval: String,
        #[arg(long, value_name = "DECISION", value_parser = ["approved", "rejected"])]
        decision: String,
        #[arg(long, value_name = "TEXT")]
        commentary: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum RunCommands {
    /// List daemon-owned runs for a session.
    List {
        #[arg(long, value_name = "SESSION_ID")]
        session: String,
    },
    /// Start a run in an existing session.
    Start {
        #[arg(long, value_name = "SESSION_ID")]
        session: String,
        #[arg(value_name = "OBJECTIVE")]
        objective: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentRuntimeCommands {
    /// List runtime profiles and providers.
    List,
    /// Manage local model endpoint profiles.
    Local {
        #[command(subcommand)]
        command: AgentRuntimeLocalCommands,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentRuntimeLocalCommands {
    /// Configure the custom local OpenAI-compatible runtime profile.
    Add {
        #[command(flatten)]
        options: AgentRuntimeLocalEndpointOptions,
    },
    /// Test a local model endpoint without changing runtime configuration.
    Test {
        #[command(flatten)]
        options: AgentRuntimeLocalEndpointOptions,
        /// Request a tool-call compatibility probe.
        #[arg(long)]
        tool_call: bool,
    },
    /// Clear local endpoint config from a runtime profile.
    Remove {
        #[arg(
            long,
            value_name = "RUNTIME_PROFILE_ID",
            default_value = "runtime-local-custom"
        )]
        profile: String,
    },
    /// Set the selected model for a local runtime profile.
    SetModel {
        #[arg(
            long,
            value_name = "RUNTIME_PROFILE_ID",
            default_value = "runtime-local-custom"
        )]
        profile: String,
        #[arg(value_name = "MODEL_ID")]
        model: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct AgentRuntimeLocalEndpointOptions {
    #[arg(long, value_name = "URL")]
    pub base_url: String,
    #[arg(long, value_name = "MODEL_ID")]
    pub model: Option<String>,
    #[arg(long, value_name = "STANDARD", default_value = "openai-chat-completions", value_parser = [
        "openai-chat-completions",
        "ollama-openai",
        "lm-studio-openai",
        "llama-cpp-openai",
        "vllm-openai",
        "tgi-messages",
    ])]
    pub standard: String,
    #[arg(long, value_name = "MODE", default_value = "none", value_parser = ["none", "bearer-env"])]
    pub auth_mode: String,
    #[arg(long, value_name = "ENV")]
    pub api_key_env: Option<String>,
    #[arg(long, default_value_t = true)]
    pub model_discovery: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DaemonStatusOptions {
    /// Poll daemon status continuously instead of exiting after one response.
    #[arg(long)]
    pub watch: bool,

    /// Delay between watch polls in milliseconds.
    #[arg(long, default_value_t = DEFAULT_WATCH_INTERVAL_MS)]
    pub interval_ms: u64,

    /// Exit after this many watch updates. Useful for scripts and tests.
    #[arg(long, requires = "watch", value_name = "COUNT")]
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Args)]
pub struct DaemonWaitOptions {
    /// Maximum time to wait for the daemon to become ready.
    #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
    pub timeout_ms: u64,

    /// Delay between readiness polls in milliseconds.
    #[arg(long, default_value_t = DEFAULT_POLL_INTERVAL_MS)]
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Args)]
pub struct DaemonLogsOptions {
    /// Show only the last N log lines.
    #[arg(long, default_value_t = DEFAULT_LOG_TAIL_LINES)]
    pub tail: usize,
}
