use std::{
    thread,
    time::{Duration, Instant},
};

use super::BackgroundServiceControlError;

const PROCESS_TERMINATE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PROCESS_KILL_GRACE_PERIOD: Duration = Duration::from_secs(2);

pub(crate) fn terminate_process(
    process_id: u32,
    terminate_timeout: Duration,
) -> Result<(), BackgroundServiceControlError> {
    if matches!(process_is_running(process_id), Some(false)) {
        return Ok(());
    }
    terminate_process_inner(process_id, terminate_timeout)
}

#[cfg(unix)]
pub(crate) fn process_is_running(process_id: u32) -> Option<bool> {
    use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

    match kill(Pid::from_raw(process_id as i32), None) {
        Ok(()) | Err(Errno::EPERM) => Some(!process_is_zombie(process_id).unwrap_or(false)),
        Err(Errno::ESRCH) => Some(false),
        Err(_) => None,
    }
}

#[cfg(not(unix))]
pub(crate) fn process_is_running(_process_id: u32) -> Option<bool> {
    None
}

#[cfg(unix)]
fn terminate_process_inner(
    process_id: u32,
    terminate_timeout: Duration,
) -> Result<(), BackgroundServiceControlError> {
    use nix::{
        errno::Errno,
        sys::signal::{Signal, kill},
        unistd::Pid,
    };

    let pid = Pid::from_raw(process_id as i32);
    match kill(pid, Some(Signal::SIGTERM)) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => {
            return Err(BackgroundServiceControlError::CommandFailed {
                command: "runtime-control terminate-owned-process",
                detail: format!("failed to send SIGTERM to pid {process_id}: {error}"),
            });
        }
    }

    wait_for_process_exit(process_id, terminate_timeout, "SIGTERM")?;
    if matches!(process_is_running(process_id), Some(false)) {
        return Ok(());
    }

    match kill(pid, Some(Signal::SIGKILL)) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => {
            return Err(BackgroundServiceControlError::CommandFailed {
                command: "runtime-control terminate-owned-process",
                detail: format!("failed to send SIGKILL to pid {process_id}: {error}"),
            });
        }
    }

    wait_for_process_exit(process_id, PROCESS_KILL_GRACE_PERIOD, "SIGKILL")
}

#[cfg(not(unix))]
fn terminate_process_inner(
    _process_id: u32,
    _terminate_timeout: Duration,
) -> Result<(), BackgroundServiceControlError> {
    Err(BackgroundServiceControlError::UnsupportedPlatform)
}

#[cfg(unix)]
fn process_is_zombie(process_id: u32) -> Option<bool> {
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &process_id.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let state = String::from_utf8(output.stdout).ok()?;
    let state = state.trim();
    if state.is_empty() {
        return Some(false);
    }
    Some(state.starts_with('Z'))
}

fn wait_for_process_exit(
    process_id: u32,
    timeout: Duration,
    signal_name: &str,
) -> Result<(), BackgroundServiceControlError> {
    let deadline = Instant::now() + timeout;
    loop {
        match process_is_running(process_id) {
            Some(false) => return Ok(()),
            Some(true) if Instant::now() < deadline => {
                thread::sleep(PROCESS_TERMINATE_POLL_INTERVAL);
            }
            Some(true) => {
                return Err(BackgroundServiceControlError::CommandFailed {
                    command: "runtime-control terminate-owned-process",
                    detail: format!(
                        "pid {process_id} did not exit after {signal_name} within {}ms",
                        timeout.as_millis()
                    ),
                });
            }
            None => {
                return Err(BackgroundServiceControlError::CommandFailed {
                    command: "runtime-control terminate-owned-process",
                    detail: format!(
                        "failed to determine whether pid {process_id} is still running"
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{process_is_running, terminate_process};
    use std::{process::Command, thread, time::Duration};

    #[test]
    #[cfg(unix)]
    fn terminate_process_stops_spawned_child() {
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep should spawn");
        let process_id = child.id();

        assert_eq!(process_is_running(process_id), Some(true));
        terminate_process(process_id, Duration::from_secs(2)).expect("terminate should stop child");
        let _ = child.wait();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if matches!(process_is_running(process_id), Some(false)) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert_eq!(process_is_running(process_id), Some(false));
    }
}
