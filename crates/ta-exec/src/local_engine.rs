use std::ffi::OsString;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::ExecEngine;
use crate::error::ExecError;
use crate::spawn_request::{ProcessGroupPolicy, SpawnRequest, StdioPolicy};

const LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG: &str = ta_sandbox::LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG;

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalExecEngine;

impl ExecEngine for LocalExecEngine {
    fn spawn(&self, request: SpawnRequest) -> Result<Child, ExecError> {
        let backend = ta_sandbox::current_backend();
        self.spawn_with_backend(request, backend.as_ref())
    }
}

impl LocalExecEngine {
    pub(crate) fn spawn_with_backend(
        &self,
        request: SpawnRequest,
        sandbox_backend: &dyn ta_sandbox::SandboxBackend,
    ) -> Result<Child, ExecError> {
        let SpawnRequest {
            mut program,
            args,
            cwd,
            env,
            env_remove,
            env_clear,
            stdin,
            stdout,
            stderr,
            kill_on_drop,
            process_group,
            sandbox_profile,
        } = request;

        ensure_process_group_supported(process_group)?;
        let mut args = args;
        let sandbox_profile = if let Some(profile) = sandbox_profile {
            let prepared = prepare_sandboxed_command_for_request(
                program,
                args,
                profile,
                &env,
                &env_remove,
                sandbox_backend,
            )?;
            program = prepared.program;
            args = prepared.args;
            Some(prepared.profile)
        } else {
            None
        };
        let mut command = Command::new(program);
        command.args(args);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        apply_child_environment(
            &mut command,
            env_clear,
            sandbox_profile.as_ref(),
            env,
            env_remove,
        );
        command
            .stdin(stdio(stdin))
            .stdout(stdio(stdout))
            .stderr(stdio(stderr))
            .kill_on_drop(kill_on_drop);
        apply_process_group(&mut command, process_group);
        command.spawn().map_err(ExecError::Spawn)
    }
}

#[derive(Debug)]
pub(crate) struct PreparedLocalSandboxCommand {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) profile: ta_sandbox::SandboxProfile,
}

pub(crate) fn prepare_sandboxed_command_for_request(
    program: OsString,
    args: Vec<OsString>,
    profile: ta_sandbox::SandboxProfile,
    env: &[(OsString, OsString)],
    env_remove: &[OsString],
    backend: &dyn ta_sandbox::SandboxBackend,
) -> Result<PreparedLocalSandboxCommand, ExecError> {
    let caller_env_present = caller_env_present(env, env_remove);
    let profile = effective_sandbox_profile(profile, env, env_remove);
    let (program, mut args) = prepare_sandboxed_command(program, args, &profile, backend)?;
    if caller_env_present && backend.kind() == ta_sandbox::SandboxKind::LinuxLandlockBwrap {
        args.insert(0, OsString::from(LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG));
    }

    Ok(PreparedLocalSandboxCommand {
        program,
        args,
        profile,
    })
}

fn caller_env_present(env: &[(OsString, OsString)], env_remove: &[OsString]) -> bool {
    env.iter()
        .any(|(name, _)| !env_remove.iter().any(|removed| removed == name))
}

pub(crate) fn effective_sandbox_profile(
    mut profile: ta_sandbox::SandboxProfile,
    env: &[(OsString, OsString)],
    env_remove: &[OsString],
) -> ta_sandbox::SandboxProfile {
    for (name, _) in env {
        if env_remove.contains(name) {
            continue;
        }
        if let Some(name) = name.to_str() {
            profile = profile.env(name);
        }
    }
    profile
}

fn apply_child_environment(
    command: &mut Command,
    env_clear: bool,
    sandbox_profile: Option<&ta_sandbox::SandboxProfile>,
    env: Vec<(OsString, OsString)>,
    env_remove: Vec<OsString>,
) {
    if let Some(profile) = sandbox_profile {
        if !profile.inherits_all_env() {
            command.env_clear();
            // Rehydrate only the profile's parent-env allowlist before applying explicit env below.
            for name in profile.env_allowlist() {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
    } else if env_clear {
        command.env_clear();
    }

    for (name, value) in env {
        command.env(name, value);
    }

    for name in env_remove {
        command.env_remove(name);
    }
}

pub(crate) fn prepare_sandboxed_command(
    program: OsString,
    args: Vec<OsString>,
    profile: &ta_sandbox::SandboxProfile,
    backend: &dyn ta_sandbox::SandboxBackend,
) -> Result<(OsString, Vec<OsString>), ExecError> {
    let command = ta_sandbox::SandboxCommand::new(program, args);
    let prepared = backend.prepare(profile, command)?;
    Ok(prepared.into_parts())
}

pub async fn terminate_child(
    child: &mut Child,
    grace_period: Duration,
) -> Result<ExitStatus, ExecError> {
    terminate_with(child, TerminationTarget::Child, grace_period).await
}

pub async fn terminate_child_tree(
    child: &mut Child,
    grace_period: Duration,
) -> Result<ExitStatus, ExecError> {
    terminate_with(child, TerminationTarget::ChildTree, grace_period).await
}

async fn terminate_with(
    child: &mut Child,
    target: TerminationTarget,
    grace_period: Duration,
) -> Result<ExitStatus, ExecError> {
    signal_child(child, target, TerminationSignal::Terminate)?;
    tokio::select! {
        status = child.wait() => status.map_err(ExecError::Wait),
        () = tokio::time::sleep(grace_period) => {
            signal_child(child, target, TerminationSignal::Kill)?;
            child.wait().await.map_err(ExecError::Wait)
        }
    }
}

fn stdio(policy: StdioPolicy) -> Stdio {
    match policy {
        StdioPolicy::Null => Stdio::null(),
        StdioPolicy::Inherit => Stdio::inherit(),
        StdioPolicy::Piped => Stdio::piped(),
    }
}

#[cfg(unix)]
fn apply_process_group(command: &mut Command, policy: ProcessGroupPolicy) {
    if policy == ProcessGroupPolicy::New {
        command.process_group(0);
    }
}

#[cfg(not(unix))]
fn apply_process_group(_command: &mut Command, _policy: ProcessGroupPolicy) {}

#[cfg(not(unix))]
fn ensure_process_group_supported(policy: ProcessGroupPolicy) -> Result<(), ExecError> {
    if policy == ProcessGroupPolicy::New {
        return Err(ExecError::UnsupportedProcessGroup(policy));
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_process_group_supported(_policy: ProcessGroupPolicy) -> Result<(), ExecError> {
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TerminationTarget {
    Child,
    ChildTree,
}

#[derive(Debug, Clone, Copy)]
enum TerminationSignal {
    Terminate,
    Kill,
}

#[cfg(unix)]
fn signal_child(
    child: &mut Child,
    target: TerminationTarget,
    signal: TerminationSignal,
) -> Result<(), ExecError> {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, kill, killpg};
    use nix::unistd::Pid;

    let Some(id) = child.id() else {
        return Ok(());
    };
    let signal = match signal {
        TerminationSignal::Terminate => Signal::SIGTERM,
        TerminationSignal::Kill => Signal::SIGKILL,
    };
    let pid = Pid::from_raw(id as i32);
    let result = match target {
        TerminationTarget::Child => kill(pid, signal),
        TerminationTarget::ChildTree => match killpg(pid, signal) {
            Ok(()) => Ok(()),
            Err(Errno::ESRCH) => kill(pid, signal),
            Err(error) => Err(error),
        },
    };
    match result {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(ExecError::Signal(error.to_string())),
    }
}

#[cfg(not(unix))]
fn signal_child(
    child: &mut Child,
    _target: TerminationTarget,
    _signal: TerminationSignal,
) -> Result<(), ExecError> {
    child.start_kill().map_err(ExecError::Wait)
}
