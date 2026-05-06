use serde::Serialize;
use ta_protocol::wire::{
    ApprovalRequest, DaemonActualRuntimeMode, DaemonControlAction, DaemonControlStatusResult,
    DaemonRuntimeMode, DaemonStatusResult, DaemonStopResult, DaemonTransitionStatus, RunSummary,
    SessionSummary,
};

use crate::daemon_ops::{
    DaemonLogsResult, DaemonRestartResult, DaemonStartResult, DaemonWaitResult,
};
use crate::error::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum DaemonStatusPoll {
    Reachable {
        status: DaemonStatusResult,
    },
    Unavailable {
        socket_path: String,
        log_path: Option<String>,
        error: String,
    },
    Error {
        socket_path: String,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutput {
    DaemonStatus(DaemonStatusResult),
    DaemonStop(DaemonStopResult),
    DaemonStart(DaemonStartResult),
    DaemonWait(DaemonWaitResult),
    DaemonRestart(DaemonRestartResult),
    DaemonLogs(DaemonLogsResult),
    DaemonStatusPoll(DaemonStatusPoll),
    DaemonBackgroundStatus(DaemonControlStatusResult),
    SessionList(Vec<SessionSummary>),
    SessionOpen(SessionSummary),
    ApprovalList(Vec<ApprovalRequest>),
    ApprovalDecide(RunSummary),
    RunList(Vec<RunSummary>),
    RunStart(RunSummary),
}

pub fn print(output: &CommandOutput, format: OutputFormat) -> Result<(), CliError> {
    let rendered = match format {
        OutputFormat::Human => render_text(output),
        OutputFormat::Json => render_json(output)?,
    };
    println!("{rendered}");
    Ok(())
}

fn render_text(output: &CommandOutput) -> String {
    match output {
        CommandOutput::DaemonStatus(status) => format_daemon_status_text(status),
        CommandOutput::DaemonStop(result) => format_daemon_stop_text(result),
        CommandOutput::DaemonStart(result) => format_daemon_start_text(result),
        CommandOutput::DaemonWait(result) => format_daemon_wait_text(result),
        CommandOutput::DaemonRestart(result) => format_daemon_restart_text(result),
        CommandOutput::DaemonLogs(result) => format_daemon_logs_text(result),
        CommandOutput::DaemonStatusPoll(result) => format_daemon_status_poll_text(result),
        CommandOutput::DaemonBackgroundStatus(result) => {
            format_daemon_background_status_text(result)
        }
        CommandOutput::SessionList(sessions) => format_session_list_text(sessions),
        CommandOutput::SessionOpen(session) => format_session_text(session),
        CommandOutput::ApprovalList(approvals) => format_approval_list_text(approvals),
        CommandOutput::ApprovalDecide(run) => format_run_text(run),
        CommandOutput::RunList(runs) => format_run_list_text(runs),
        CommandOutput::RunStart(run) => format_run_text(run),
    }
}

fn render_json(output: &CommandOutput) -> Result<String, CliError> {
    match output {
        CommandOutput::DaemonStatus(status) => to_json(status),
        CommandOutput::DaemonStop(result) => to_json(result),
        CommandOutput::DaemonStart(result) => to_json(result),
        CommandOutput::DaemonWait(result) => to_json(result),
        CommandOutput::DaemonRestart(result) => to_json(result),
        CommandOutput::DaemonLogs(result) => to_json(result),
        CommandOutput::DaemonStatusPoll(result) => to_json(result),
        CommandOutput::DaemonBackgroundStatus(result) => to_json(result),
        CommandOutput::SessionList(sessions) => to_json(sessions),
        CommandOutput::SessionOpen(session) => to_json(session),
        CommandOutput::ApprovalList(approvals) => to_json(approvals),
        CommandOutput::ApprovalDecide(run) => to_json(run),
        CommandOutput::RunList(runs) => to_json(runs),
        CommandOutput::RunStart(run) => to_json(run),
    }
}

fn format_daemon_status_text(status: &DaemonStatusResult) -> String {
    let readiness = if status.ready { "ready" } else { "not ready" };
    let runtime_mode = match status.runtime_mode {
        DaemonRuntimeMode::Local => "local",
        DaemonRuntimeMode::Background => "background",
    };
    format_with_connection_details(
        &format!("daemon {readiness}\nmode: {runtime_mode}"),
        &status.socket_path,
        &status.log_path,
        &status.version,
    )
}

fn format_daemon_stop_text(result: &DaemonStopResult) -> String {
    if result.stopping {
        "daemon stopping".to_string()
    } else {
        "daemon stop request rejected".to_string()
    }
}

fn format_daemon_start_text(result: &DaemonStartResult) -> String {
    let state = if result.already_running {
        "daemon already running"
    } else {
        "daemon started"
    };
    format_with_connection_details(
        state,
        &result.socket_path,
        &result.log_path,
        &result.version,
    )
}

fn format_daemon_wait_text(result: &DaemonWaitResult) -> String {
    format_with_connection_details(
        &format!("daemon ready after {}ms", result.waited_ms),
        &result.socket_path,
        &result.log_path,
        &result.version,
    )
}

fn format_daemon_restart_text(result: &DaemonRestartResult) -> String {
    let state = if result.was_running {
        "daemon restarted"
    } else {
        "daemon started"
    };
    format_with_connection_details(
        state,
        &result.socket_path,
        &result.log_path,
        &result.version,
    )
}

fn format_daemon_logs_text(result: &DaemonLogsResult) -> String {
    if result.contents.trim().is_empty() {
        return format!("daemon log {} is empty", result.path);
    }

    if result.truncated {
        return format!(
            "daemon log ({}, last {} lines):\n{}",
            result.path, result.lines, result.contents
        );
    }

    format!("daemon log ({}):\n{}", result.path, result.contents)
}

fn format_daemon_status_poll_text(result: &DaemonStatusPoll) -> String {
    match result {
        DaemonStatusPoll::Reachable { status } => format_daemon_status_text(status),
        DaemonStatusPoll::Unavailable {
            socket_path,
            log_path,
            error,
        } => format_with_socket_log_error(socket_path, log_path.as_deref(), error),
        DaemonStatusPoll::Error { socket_path, error } => {
            format!("daemon status error\nsocket: {socket_path}\nerror: {error}")
        }
    }
}

fn format_daemon_background_status_text(result: &DaemonControlStatusResult) -> String {
    let actions = if result.allowed_actions.is_empty() {
        "none".to_string()
    } else {
        result
            .allowed_actions
            .iter()
            .map(|action| format_control_action(action))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut lines = vec![
        format!("desired: {}", format_runtime_mode(result.desired_mode)),
        format!("actual: {}", format_actual_mode(&result.actual_mode)),
        format!(
            "transition: {}",
            format_transition_status(&result.transition_status)
        ),
        format!("reconcile required: {}", yes_no(result.reconcile_required)),
        format!("background opt-in: {}", yes_no(result.background_opt_in)),
        format!("allowed actions: {actions}"),
        format!("message: {}", result.message),
        format!("socket: {}", result.socket_path),
        format!("log: {}", result.log_path),
    ];
    if let Some(daemon_version) = result.daemon_version.as_deref() {
        lines.push(format!("version: {daemon_version}"));
    }
    lines.join("\n")
}

fn format_approval_list_text(approvals: &[ApprovalRequest]) -> String {
    if approvals.is_empty() {
        return "no open approvals".to_string();
    }

    approvals
        .iter()
        .map(|approval| {
            format!(
                "{}\nscope: {:?}\nrun: {}\nreason: {}",
                approval.id.as_str(),
                approval.scope,
                approval.run_id.as_str(),
                approval.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn to_json<T: Serialize>(value: &T) -> Result<String, CliError> {
    serde_json::to_string(value).map_err(CliError::SerializeOutput)
}

fn format_with_connection_details(
    header: &str,
    socket_path: &str,
    log_path: &str,
    version: &str,
) -> String {
    format!("{header}\nsocket: {socket_path}\nlog: {log_path}\nversion: {version}")
}

fn format_with_socket_log_error(socket_path: &str, log_path: Option<&str>, error: &str) -> String {
    format!(
        "daemon unavailable\nsocket: {socket_path}\nlog: {}\nerror: {error}",
        log_path.unwrap_or("unknown"),
    )
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_runtime_mode(mode: DaemonRuntimeMode) -> &'static str {
    match mode {
        DaemonRuntimeMode::Local => "local",
        DaemonRuntimeMode::Background => "background",
    }
}

fn format_actual_mode(mode: &DaemonActualRuntimeMode) -> &'static str {
    match mode {
        DaemonActualRuntimeMode::Stopped => "stopped",
        DaemonActualRuntimeMode::Local => "local",
        DaemonActualRuntimeMode::Background => "background",
        DaemonActualRuntimeMode::Foreign => "foreign",
    }
}

fn format_transition_status(status: &DaemonTransitionStatus) -> &'static str {
    match status {
        DaemonTransitionStatus::Idle => "idle",
        DaemonTransitionStatus::Applying => "applying",
        DaemonTransitionStatus::DegradedReconcileRequired => "degraded-reconcile-required",
        DaemonTransitionStatus::FailedNoStateChange => "failed-no-state-change",
    }
}

fn format_control_action(action: &DaemonControlAction) -> &'static str {
    match action {
        DaemonControlAction::Start => "start",
        DaemonControlAction::Stop => "stop",
        DaemonControlAction::EnableBackground => "enable-background",
        DaemonControlAction::DisableBackground => "disable-background",
        DaemonControlAction::Reconcile => "reconcile",
    }
}

fn format_session_list_text(sessions: &[SessionSummary]) -> String {
    if sessions.is_empty() {
        return "no sessions".to_string();
    }

    sessions
        .iter()
        .map(format_session_text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_session_text(session: &SessionSummary) -> String {
    format!(
        "session {}\ntitle: {}\nstatus: {}",
        session.id.as_str(),
        session.title,
        format_session_status(session.status),
    )
}

fn format_session_status(status: ta_protocol::wire::SessionStatus) -> &'static str {
    match status {
        ta_protocol::wire::SessionStatus::Idle => "idle",
        ta_protocol::wire::SessionStatus::Running => "running",
        ta_protocol::wire::SessionStatus::Paused => "paused",
        ta_protocol::wire::SessionStatus::Failed => "failed",
        ta_protocol::wire::SessionStatus::Completed => "completed",
    }
}

fn format_run_list_text(runs: &[RunSummary]) -> String {
    if runs.is_empty() {
        return "no runs".to_string();
    }

    runs.iter()
        .map(format_run_text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_run_text(run: &RunSummary) -> String {
    format!(
        "run {}\nobjective: {}\nstatus: {}",
        run.id.as_str(),
        run.objective,
        format_run_status(run.status),
    )
}

fn format_run_status(status: ta_protocol::wire::RunStatus) -> &'static str {
    match status {
        ta_protocol::wire::RunStatus::Queued => "queued",
        ta_protocol::wire::RunStatus::Running => "running",
        ta_protocol::wire::RunStatus::WaitingForApproval => "waiting-for-approval",
        ta_protocol::wire::RunStatus::Completed => "completed",
        ta_protocol::wire::RunStatus::Failed => "failed",
        ta_protocol::wire::RunStatus::BudgetExceeded => "budget-exceeded",
        ta_protocol::wire::RunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use ta_protocol::wire::{
        DaemonActualRuntimeMode, DaemonControlAction, DaemonControlStatusResult, DaemonRuntimeMode,
        DaemonStatusResult, DaemonStopResult, DaemonTransitionStatus,
    };

    use super::{
        DaemonStatusPoll, format_daemon_logs_text, format_daemon_restart_text,
        format_daemon_start_text, format_daemon_status_poll_text, format_daemon_status_text,
        format_daemon_stop_text, format_daemon_wait_text, yes_no,
    };
    use crate::daemon_ops::{
        DaemonLogsResult, DaemonRestartResult, DaemonStartResult, DaemonWaitResult,
    };

    #[test]
    fn formats_text_daemon_status_output() {
        let status = DaemonStatusResult {
            ready: true,
            daemon_instance_id: "daemon-1".to_string(),
            runtime_mode: DaemonRuntimeMode::Local,
            socket_path: "/tmp/ta.sock".to_string(),
            log_path: "/tmp/taugentic-daemon/ta/ta-daemon.log.jsonl".to_string(),
            version: "0.0.1".to_string(),
        };

        assert_eq!(
            format_daemon_status_text(&status),
            "daemon ready\nmode: local\nsocket: /tmp/ta.sock\nlog: /tmp/taugentic-daemon/ta/ta-daemon.log.jsonl\nversion: 0.0.1"
        );
    }

    #[test]
    fn formats_text_daemon_stop_output_branches() {
        assert_eq!(
            format_daemon_stop_text(&DaemonStopResult { stopping: true }),
            "daemon stopping"
        );
        assert_eq!(
            format_daemon_stop_text(&DaemonStopResult { stopping: false }),
            "daemon stop request rejected"
        );
    }

    #[test]
    fn formats_text_connection_detail_outputs() {
        let socket_path = "/tmp/ta.sock";
        let log_path = "/tmp/log/ta-daemon.log.jsonl";
        let version = "0.0.1";

        let start_result = DaemonStartResult {
            started: true,
            already_running: false,
            pid: Some(42),
            socket_path: socket_path.to_string(),
            log_path: log_path.to_string(),
            version: version.to_string(),
        };
        let already_running_result = DaemonStartResult {
            already_running: true,
            ..start_result.clone()
        };
        let wait_result = DaemonWaitResult {
            ready: true,
            socket_path: socket_path.to_string(),
            log_path: "/tmp/taugentic-daemon/ta/ta-daemon.log.jsonl".to_string(),
            version: version.to_string(),
            waited_ms: 250,
        };
        let restart_result = DaemonRestartResult {
            restarted: true,
            was_running: true,
            pid: Some(42),
            socket_path: socket_path.to_string(),
            log_path: log_path.to_string(),
            version: version.to_string(),
        };
        let started_from_restart_result = DaemonRestartResult {
            was_running: false,
            ..restart_result.clone()
        };

        assert_connection_details(
            &format_daemon_start_text(&start_result),
            "daemon started",
            socket_path,
            log_path,
            version,
        );
        assert_connection_details(
            &format_daemon_start_text(&already_running_result),
            "daemon already running",
            socket_path,
            log_path,
            version,
        );
        assert_connection_details(
            &format_daemon_wait_text(&wait_result),
            "daemon ready after 250ms",
            socket_path,
            "/tmp/taugentic-daemon/ta/ta-daemon.log.jsonl",
            version,
        );
        assert_connection_details(
            &format_daemon_restart_text(&restart_result),
            "daemon restarted",
            socket_path,
            log_path,
            version,
        );
        assert_connection_details(
            &format_daemon_restart_text(&started_from_restart_result),
            "daemon started",
            socket_path,
            log_path,
            version,
        );
    }

    #[test]
    fn formats_text_daemon_logs_output_branches() {
        assert_eq!(
            format_daemon_logs_text(&DaemonLogsResult {
                path: "/tmp/log/ta-daemon.log.jsonl".to_string(),
                contents: "   ".to_string(),
                lines: 0,
                truncated: false,
            }),
            "daemon log /tmp/log/ta-daemon.log.jsonl is empty"
        );
        assert_eq!(
            format_daemon_logs_text(&DaemonLogsResult {
                path: "/tmp/log/ta-daemon.log.jsonl".to_string(),
                contents: "line one\nline two".to_string(),
                lines: 2,
                truncated: true,
            }),
            "daemon log (/tmp/log/ta-daemon.log.jsonl, last 2 lines):\nline one\nline two"
        );
        assert_eq!(
            format_daemon_logs_text(&DaemonLogsResult {
                path: "/tmp/log/ta-daemon.log.jsonl".to_string(),
                contents: "line one\nline two".to_string(),
                lines: 2,
                truncated: false,
            }),
            "daemon log (/tmp/log/ta-daemon.log.jsonl):\nline one\nline two"
        );
    }

    #[test]
    fn formats_text_daemon_status_poll_output_when_unavailable() {
        let result = DaemonStatusPoll::Unavailable {
            socket_path: "/tmp/ta.sock".to_string(),
            log_path: Some("/tmp/taugentic-daemon/ta/ta-daemon.log.jsonl".to_string()),
            error: "failed to connect".to_string(),
        };

        assert_eq!(
            format_daemon_status_poll_text(&result),
            "daemon unavailable\nsocket: /tmp/ta.sock\nlog: /tmp/taugentic-daemon/ta/ta-daemon.log.jsonl\nerror: failed to connect"
        );
    }

    #[test]
    fn formats_text_daemon_status_poll_output_when_error() {
        let result = DaemonStatusPoll::Error {
            socket_path: "/tmp/ta.sock".to_string(),
            error: "remote JSON-RPC error -32601: method not found".to_string(),
        };

        assert_eq!(
            format_daemon_status_poll_text(&result),
            "daemon status error\nsocket: /tmp/ta.sock\nerror: remote JSON-RPC error -32601: method not found"
        );
    }

    #[test]
    fn formats_text_daemon_background_status_output() {
        let result = DaemonControlStatusResult {
            background_opt_in: true,
            desired_mode: DaemonRuntimeMode::Background,
            actual_mode: DaemonActualRuntimeMode::Background,
            transition_status: DaemonTransitionStatus::Idle,
            reconcile_required: false,
            allowed_actions: vec![
                DaemonControlAction::Stop,
                DaemonControlAction::DisableBackground,
            ],
            error_code: None,
            message: "Background mode is the desired runtime.".to_string(),
            pending_transition: None,
            socket_path: "/tmp/ta.sock".to_string(),
            log_path: "/tmp/taugentic-daemon/ta/ta-daemon.log.jsonl".to_string(),
            daemon_version: Some("0.0.1".to_string()),
            protocol_version: "2026-04-stage2".to_string(),
        };

        assert_eq!(
            super::format_daemon_background_status_text(&result),
            "desired: background\nactual: background\ntransition: idle\nreconcile required: no\nbackground opt-in: yes\nallowed actions: stop, disable-background\nmessage: Background mode is the desired runtime.\nsocket: /tmp/ta.sock\nlog: /tmp/taugentic-daemon/ta/ta-daemon.log.jsonl\nversion: 0.0.1"
        );
        assert_eq!(yes_no(true), "yes");
        assert_eq!(yes_no(false), "no");
    }

    fn assert_connection_details(
        rendered: &str,
        header: &str,
        socket_path: &str,
        log_path: &str,
        version: &str,
    ) {
        assert!(
            rendered.starts_with(header),
            "unexpected header: {rendered}"
        );
        assert!(rendered.contains(&format!("\nsocket: {socket_path}")));
        assert!(rendered.contains(&format!("\nlog: {log_path}")));
        assert!(rendered.ends_with(&format!("version: {version}")));
    }
}
