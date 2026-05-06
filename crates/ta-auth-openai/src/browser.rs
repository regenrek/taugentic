use std::process::Command;

use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserLaunch {
    Opened,
    Manual { authorize_url: Url, reason: String },
}

pub fn open_authorize_url(authorize_url: &Url) -> BrowserLaunch {
    match launch_url(authorize_url.as_str()) {
        Ok(()) => BrowserLaunch::Opened,
        Err(reason) => BrowserLaunch::Manual {
            authorize_url: authorize_url.clone(),
            reason,
        },
    }
}

#[cfg(target_os = "macos")]
fn launch_url(url: &str) -> Result<(), String> {
    run_launcher(LauncherCommand::new("open", [url]))
}

#[cfg(target_os = "windows")]
fn launch_url(url: &str) -> Result<(), String> {
    run_launcher(windows_launcher_command(url))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_url(url: &str) -> Result<(), String> {
    run_launcher(LauncherCommand::new("xdg-open", [url]))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn launch_url(_url: &str) -> Result<(), String> {
    Err("no browser launcher is available for this platform".to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LauncherCommand {
    program: &'static str,
    args: Vec<String>,
}

impl LauncherCommand {
    fn new<const N: usize>(program: &'static str, args: [&str; N]) -> Self {
        Self {
            program,
            args: args.into_iter().map(str::to_string).collect(),
        }
    }
}

#[cfg(any(test, target_os = "windows"))]
fn windows_launcher_command(url: &str) -> LauncherCommand {
    LauncherCommand::new("cmd", ["/C", "start", "", url])
}

fn run_launcher(command: LauncherCommand) -> Result<(), String> {
    let status = Command::new(command.program)
        .args(command.args)
        .status()
        .map_err(|error| format!("{} could not be started: {error}", command.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with status {status}", command.program))
    }
}

#[cfg(test)]
mod tests {
    use super::{LauncherCommand, windows_launcher_command};

    #[test]
    fn windows_launcher_uses_empty_title_and_unmodified_url_arg() {
        for url in [
            "https://auth.openai.com/oauth/authorize?client_id=ta&scope=openid%20offline_access",
            "https://auth.openai.com/oauth/authorize?state=a&code_challenge=b",
            "https://auth.openai.com/oauth/authorize?state=a^b%25c",
        ] {
            assert_eq!(
                windows_launcher_command(url),
                LauncherCommand::new("cmd", ["/C", "start", "", url])
            );
        }
    }
}
