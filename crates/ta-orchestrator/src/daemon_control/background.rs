use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::paths::normalize_env_path;
use crate::BackgroundServiceControlError;
use crate::host::config::DaemonControlLaunchConfig;
use ta_host_platform::current_capabilities;

const TAUGENTIC_LAUNCH_AGENT_LABEL: &str = "com.taugentic.daemon";
const TAUGENTIC_SYSTEMD_USER_UNIT_NAME: &str = "taugentic-daemon.service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackgroundServiceKind {
    Launchd,
    Systemd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundServiceState {
    pub available: bool,
    pub enabled: bool,
    pub loaded: bool,
    pub running: bool,
    pub process_id: Option<u32>,
    pub service_name: Option<String>,
}

pub fn read_background_service_state()
-> Result<BackgroundServiceState, BackgroundServiceControlError> {
    let Some(kind) = detect_background_service_kind() else {
        return Ok(BackgroundServiceState {
            available: false,
            enabled: false,
            loaded: false,
            running: false,
            process_id: None,
            service_name: None,
        });
    };

    match kind {
        BackgroundServiceKind::Launchd => read_launch_agent_state(),
        BackgroundServiceKind::Systemd => read_systemd_user_service_state(),
    }
}

pub fn enable_background_service(
    program: &Path,
    launch_config: &DaemonControlLaunchConfig,
) -> Result<(), BackgroundServiceControlError> {
    ensure_background_service_running(program, launch_config)
}

pub fn ensure_background_service_running(
    program: &Path,
    launch_config: &DaemonControlLaunchConfig,
) -> Result<(), BackgroundServiceControlError> {
    let kind = detect_background_service_kind()
        .ok_or(BackgroundServiceControlError::UnsupportedPlatform)?;

    let working_directory = program.parent().map(Path::to_path_buf);
    let service_env = launch_config.environment();

    match kind {
        BackgroundServiceKind::Launchd => {
            let plist_path = resolve_launch_agent_plist_path()?;
            let stdout_path = resolve_launch_agent_log_path("ta-daemon.launchd.log")?;
            let stderr_path = resolve_launch_agent_log_path("ta-daemon.launchd.err.log")?;
            ensure_parent_dir(&plist_path)?;
            ensure_parent_dir(&stdout_path)?;
            ensure_parent_dir(&stderr_path)?;
            let contents = build_launch_agent_plist(
                program,
                working_directory.as_deref(),
                &stdout_path,
                &stderr_path,
                &service_env,
            );
            let contents_changed = write_if_changed(&plist_path, &contents)?;
            let state = read_launch_agent_state()?;
            if state.running && !contents_changed {
                return Ok(());
            }
            if state.loaded {
                bootout_launch_agent_if_loaded(&plist_path)?;
            }
            clear_launch_agent_disabled_override()?;
            bootstrap_launch_agent(&plist_path)?;
            kickstart_launch_agent()?;
            Ok(())
        }
        BackgroundServiceKind::Systemd => {
            let unit_path = resolve_systemd_user_unit_path()?;
            ensure_parent_dir(&unit_path)?;
            let contents =
                build_systemd_user_unit(program, working_directory.as_deref(), &service_env);
            let contents_changed = write_if_changed(&unit_path, &contents)?;
            let state = read_systemd_user_service_state()?;
            if contents_changed {
                daemon_reload_systemd_user()?;
            }
            if state.running && !contents_changed {
                return Ok(());
            }
            enable_systemd_user_unit()?;
            restart_systemd_user_unit()?;
            Ok(())
        }
    }
}

pub fn stop_background_service() -> Result<(), BackgroundServiceControlError> {
    let kind = detect_background_service_kind()
        .ok_or(BackgroundServiceControlError::UnsupportedPlatform)?;

    match kind {
        BackgroundServiceKind::Launchd => {
            let plist_path = resolve_launch_agent_plist_path()?;
            if read_launch_agent_state()?.loaded {
                bootout_launch_agent_if_loaded(&plist_path)?;
            }
        }
        BackgroundServiceKind::Systemd => {
            stop_systemd_user_unit()?;
        }
    }

    Ok(())
}

pub fn disable_background_service() -> Result<(), BackgroundServiceControlError> {
    disable_background_service_runtime_only()
}

pub fn disable_background_service_runtime_only() -> Result<(), BackgroundServiceControlError> {
    let kind = detect_background_service_kind()
        .ok_or(BackgroundServiceControlError::UnsupportedPlatform)?;

    match kind {
        BackgroundServiceKind::Launchd => {
            let plist_path = resolve_launch_agent_plist_path()?;
            stop_background_service()?;
            remove_if_exists(&plist_path)?;
        }
        BackgroundServiceKind::Systemd => {
            let unit_path = resolve_systemd_user_unit_path()?;
            stop_background_service()?;
            disable_systemd_user_unit()?;
            remove_if_exists(&unit_path)?;
            daemon_reload_systemd_user()?;
        }
    }

    Ok(())
}

fn detect_background_service_kind() -> Option<BackgroundServiceKind> {
    let capabilities = current_capabilities();
    if capabilities.supports_launchd_user_services {
        return Some(BackgroundServiceKind::Launchd);
    }
    if capabilities.supports_systemd_user_services {
        return Some(BackgroundServiceKind::Systemd);
    }
    None
}

fn resolve_launch_agent_plist_path() -> Result<PathBuf, BackgroundServiceControlError> {
    Ok(resolve_home_dir()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{TAUGENTIC_LAUNCH_AGENT_LABEL}.plist")))
}

fn resolve_launch_agent_log_path(
    file_name: &str,
) -> Result<PathBuf, BackgroundServiceControlError> {
    Ok(resolve_home_dir()?
        .join("Library")
        .join("Logs")
        .join("Taugentic")
        .join(file_name))
}

fn resolve_systemd_user_unit_path() -> Result<PathBuf, BackgroundServiceControlError> {
    Ok(resolve_home_dir()?
        .join(".config")
        .join("systemd")
        .join("user")
        .join(TAUGENTIC_SYSTEMD_USER_UNIT_NAME))
}

fn resolve_home_dir() -> Result<PathBuf, BackgroundServiceControlError> {
    normalize_env_path(env::var_os("HOME"))
        .or_else(|| normalize_env_path(env::var_os("USERPROFILE")))
        .map(PathBuf::from)
        .ok_or(BackgroundServiceControlError::MissingHomeDirectory)
}

fn ensure_parent_dir(path: &Path) -> Result<(), BackgroundServiceControlError> {
    let parent = path
        .parent()
        .expect("service control paths should always have a parent")
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|source| BackgroundServiceControlError::CreateDirectory {
        path: parent,
        source,
    })
}

fn write_if_changed(path: &Path, contents: &str) -> Result<bool, BackgroundServiceControlError> {
    let existing = match fs::read_to_string(path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(BackgroundServiceControlError::ReadFile {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if existing.as_deref() == Some(contents) {
        return Ok(false);
    }
    fs::write(path, contents).map_err(|source| BackgroundServiceControlError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

fn remove_if_exists(path: &Path) -> Result<(), BackgroundServiceControlError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(BackgroundServiceControlError::RemoveFile {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn build_launch_agent_plist(
    program: &Path,
    working_directory: Option<&Path>,
    stdout_path: &Path,
    stderr_path: &Path,
    environment: &[(String, String)],
) -> String {
    let args_xml = format!(
        "\n      <string>{}</string>",
        plist_escape(&program.display().to_string())
    );
    let working_directory_xml = working_directory
        .map(|directory| {
            format!(
                "\n    <key>WorkingDirectory</key>\n    <string>{}</string>",
                plist_escape(&directory.display().to_string())
            )
        })
        .unwrap_or_default();
    let environment_xml = if environment.is_empty() {
        String::new()
    } else {
        format!(
            "\n    <key>EnvironmentVariables</key>\n    <dict>{}\n    </dict>",
            environment
                .iter()
                .map(|(key, value)| {
                    format!(
                        "\n      <key>{}</key>\n      <string>{}</string>",
                        plist_escape(key),
                        plist_escape(value)
                    )
                })
                .collect::<String>()
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n  <dict>\n    <key>Label</key>\n    <string>{}</string>\n    <key>RunAtLoad</key>\n    <true/>\n    <key>KeepAlive</key>\n    <dict>\n      <key>SuccessfulExit</key>\n      <false/>\n    </dict>\n    <key>ProgramArguments</key>\n    <array>{}\n    </array>{}{}\n    <key>StandardOutPath</key>\n    <string>{}</string>\n    <key>StandardErrorPath</key>\n    <string>{}</string>\n  </dict>\n</plist>\n",
        plist_escape(TAUGENTIC_LAUNCH_AGENT_LABEL),
        args_xml,
        working_directory_xml,
        environment_xml,
        plist_escape(&stdout_path.display().to_string()),
        plist_escape(&stderr_path.display().to_string())
    )
}

fn build_systemd_user_unit(
    program: &Path,
    working_directory: Option<&Path>,
    environment: &[(String, String)],
) -> String {
    let mut lines = vec![
        "[Unit]".to_string(),
        "Description=Taugentic Daemon".to_string(),
        "After=network-online.target".to_string(),
        "Wants=network-online.target".to_string(),
        String::new(),
        "[Service]".to_string(),
    ];
    lines.extend(
        environment.iter().map(|(key, value)| {
            format!("Environment={}", systemd_escape(&format!("{key}={value}")))
        }),
    );
    lines.push(format!(
        "ExecStart={}",
        systemd_escape(&program.display().to_string())
    ));
    lines.push("Restart=on-failure".to_string());
    lines.push("RestartSec=2".to_string());
    if let Some(directory) = working_directory {
        lines.push(format!(
            "WorkingDirectory={}",
            systemd_escape(&directory.display().to_string())
        ));
    }
    lines.extend([
        String::new(),
        "[Install]".to_string(),
        "WantedBy=default.target".to_string(),
        String::new(),
    ]);
    lines.join("\n")
}

fn plist_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn systemd_escape(value: &str) -> String {
    if !value
        .chars()
        .any(|character| character.is_whitespace() || character == '"' || character == '\\')
    {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn read_launch_agent_state() -> Result<BackgroundServiceState, BackgroundServiceControlError> {
    let plist_path = resolve_launch_agent_plist_path()?;
    let enabled = plist_path.is_file();
    let result = exec_command("launchctl", &["print", &launch_agent_domain_service_name()])?;
    if result.code != 0 {
        if is_launchctl_missing_service(&result) {
            return Ok(BackgroundServiceState {
                available: true,
                enabled,
                loaded: false,
                running: false,
                process_id: None,
                service_name: Some(TAUGENTIC_LAUNCH_AGENT_LABEL.to_string()),
            });
        }
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "launchctl print",
            detail: command_detail(&result),
        });
    }

    if let Some(active_plist_path) = match_assignment_value(&result.stdout, "path")
        && !launch_agent_paths_match(&plist_path, &active_plist_path)
    {
        return Ok(BackgroundServiceState {
            available: true,
            enabled,
            loaded: false,
            running: false,
            process_id: None,
            service_name: Some(TAUGENTIC_LAUNCH_AGENT_LABEL.to_string()),
        });
    }

    let state = match_assignment_value(&result.stdout, "state").unwrap_or_default();
    let pid =
        match_assignment_value(&result.stdout, "pid").and_then(|value| value.parse::<u32>().ok());
    Ok(BackgroundServiceState {
        available: true,
        enabled,
        loaded: true,
        running: state.eq_ignore_ascii_case("running") || pid.is_some(),
        process_id: pid,
        service_name: Some(TAUGENTIC_LAUNCH_AGENT_LABEL.to_string()),
    })
}

fn read_systemd_user_service_state() -> Result<BackgroundServiceState, BackgroundServiceControlError>
{
    let unit_path = resolve_systemd_user_unit_path()?;
    let enabled = unit_path.is_file();
    let result = exec_command(
        "systemctl",
        &[
            "--user",
            "show",
            TAUGENTIC_SYSTEMD_USER_UNIT_NAME,
            "--no-page",
            "--property",
            "LoadState,ActiveState,SubState,MainPID",
        ],
    )?;
    if result.code != 0 {
        if is_systemd_user_bus_unavailable(&result) {
            return Ok(BackgroundServiceState {
                available: false,
                enabled: false,
                loaded: false,
                running: false,
                process_id: None,
                service_name: Some(TAUGENTIC_SYSTEMD_USER_UNIT_NAME.to_string()),
            });
        }
        if is_systemd_missing_unit(&result) {
            return Ok(BackgroundServiceState {
                available: true,
                enabled,
                loaded: false,
                running: false,
                process_id: None,
                service_name: Some(TAUGENTIC_SYSTEMD_USER_UNIT_NAME.to_string()),
            });
        }
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "systemctl --user show",
            detail: command_detail(&result),
        });
    }

    let load_state = parse_key_value(&result.stdout, "LoadState");
    let active_state = parse_key_value(&result.stdout, "ActiveState");
    let main_pid =
        parse_key_value(&result.stdout, "MainPID").and_then(|value| value.parse::<u32>().ok());
    Ok(BackgroundServiceState {
        available: true,
        enabled: enabled || load_state.as_deref() == Some("loaded"),
        loaded: load_state.as_deref() == Some("loaded"),
        running: active_state.as_deref() == Some("active"),
        process_id: main_pid.filter(|pid| *pid > 0),
        service_name: Some(TAUGENTIC_SYSTEMD_USER_UNIT_NAME.to_string()),
    })
}

fn bootout_launch_agent_if_loaded(plist_path: &Path) -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "launchctl",
        &[
            "bootout",
            &resolve_launch_agent_domain(),
            &plist_path.display().to_string(),
        ],
    )?;
    if result.code != 0 && !is_launchctl_missing_service(&result) {
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "launchctl bootout",
            detail: command_detail(&result),
        });
    }
    Ok(())
}

fn bootstrap_launch_agent(plist_path: &Path) -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "launchctl",
        &[
            "bootstrap",
            &resolve_launch_agent_domain(),
            &plist_path.display().to_string(),
        ],
    )?;
    ensure_command_success("launchctl bootstrap", &result)
}

fn clear_launch_agent_disabled_override() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "launchctl",
        &["enable", &launch_agent_domain_service_name()],
    )?;
    if result.code != 0 && !is_launchctl_missing_service(&result) {
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "launchctl enable",
            detail: command_detail(&result),
        });
    }
    Ok(())
}

fn kickstart_launch_agent() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "launchctl",
        &["kickstart", "-k", &launch_agent_domain_service_name()],
    )?;
    ensure_command_success("launchctl kickstart", &result)
}

fn daemon_reload_systemd_user() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command("systemctl", &["--user", "daemon-reload"])?;
    ensure_command_success("systemctl daemon-reload", &result)
}

fn enable_systemd_user_unit() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "systemctl",
        &["--user", "enable", TAUGENTIC_SYSTEMD_USER_UNIT_NAME],
    )?;
    ensure_command_success("systemctl enable", &result)
}

fn restart_systemd_user_unit() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "systemctl",
        &["--user", "restart", TAUGENTIC_SYSTEMD_USER_UNIT_NAME],
    )?;
    ensure_command_success("systemctl restart", &result)
}

fn stop_systemd_user_unit() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "systemctl",
        &["--user", "stop", TAUGENTIC_SYSTEMD_USER_UNIT_NAME],
    )?;
    if result.code != 0 && !is_systemd_missing_unit(&result) {
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "systemctl stop",
            detail: command_detail(&result),
        });
    }
    Ok(())
}

fn disable_systemd_user_unit() -> Result<(), BackgroundServiceControlError> {
    let result = exec_command(
        "systemctl",
        &["--user", "disable", TAUGENTIC_SYSTEMD_USER_UNIT_NAME],
    )?;
    if result.code != 0 && !is_systemd_missing_unit(&result) {
        return Err(BackgroundServiceControlError::CommandFailed {
            command: "systemctl disable",
            detail: command_detail(&result),
        });
    }
    Ok(())
}

struct CommandResult {
    code: i32,
    stdout: String,
    stderr: String,
}

fn exec_command(
    command: &'static str,
    args: &[&str],
) -> Result<CommandResult, BackgroundServiceControlError> {
    let output = Command::new(command).args(args).output().map_err(|error| {
        BackgroundServiceControlError::CommandFailed {
            command,
            detail: error.to_string(),
        }
    })?;
    Ok(CommandResult {
        code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn ensure_command_success(
    command: &'static str,
    result: &CommandResult,
) -> Result<(), BackgroundServiceControlError> {
    if result.code == 0 {
        return Ok(());
    }
    Err(BackgroundServiceControlError::CommandFailed {
        command,
        detail: command_detail(result),
    })
}

fn command_detail(result: &CommandResult) -> String {
    let detail = format!("{} {}", result.stderr.trim(), result.stdout.trim())
        .trim()
        .to_string();
    if detail.is_empty() {
        format!("exit code {}", result.code)
    } else {
        detail
    }
}

fn resolve_launch_agent_domain() -> String {
    if let Some(uid) = current_uid() {
        return format!("gui/{uid}");
    }
    "gui/501".to_string()
}

fn current_uid() -> Option<u32> {
    env::var("UID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .or_else(resolve_uid_from_id_command)
}

fn resolve_uid_from_id_command() -> Option<u32> {
    #[cfg(unix)]
    {
        let output = Command::new("id").arg("-u").output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .ok()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn launch_agent_domain_service_name() -> String {
    format!(
        "{}/{}",
        resolve_launch_agent_domain(),
        TAUGENTIC_LAUNCH_AGENT_LABEL
    )
}

fn is_launchctl_missing_service(result: &CommandResult) -> bool {
    let detail = format!("{} {}", result.stderr, result.stdout).to_lowercase();
    detail.contains("could not find service")
        || detail.contains("no such process")
        || detail.contains("not found")
}

fn is_systemd_missing_unit(result: &CommandResult) -> bool {
    let detail = format!("{} {}", result.stderr, result.stdout).to_lowercase();
    detail.contains("not-found")
        || detail.contains("not found")
        || detail.contains("loaded: not-found")
}

fn is_systemd_user_bus_unavailable(result: &CommandResult) -> bool {
    let detail = format!("{} {}", result.stderr, result.stdout).to_lowercase();
    detail.contains("failed to connect to bus")
        || detail.contains("no medium found")
        || detail.contains("connection refused")
}

fn parse_key_value(output: &str, key: &str) -> Option<String> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix(&format!("{key}=")).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn match_assignment_value(output: &str, key: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(&format!("{key} = "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn launch_agent_paths_match(expected: &Path, active: &str) -> bool {
    Path::new(active) == expected
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::launch_agent_paths_match;

    #[test]
    fn launch_agent_paths_match_for_same_plist() {
        assert!(launch_agent_paths_match(
            Path::new("/Users/kregenrek/Library/LaunchAgents/com.taugentic.daemon.plist"),
            "/Users/kregenrek/Library/LaunchAgents/com.taugentic.daemon.plist",
        ));
    }

    #[test]
    fn launch_agent_paths_do_not_match_for_temp_agent() {
        assert!(!launch_agent_paths_match(
            Path::new("/Users/kregenrek/Library/LaunchAgents/com.taugentic.daemon.plist"),
            "/Users/kregenrek/taugentic-dev/tmp/ta-cli-control-serve-mutating-1775928468144206000/Library/LaunchAgents/com.taugentic.daemon.plist",
        ));
    }
}
