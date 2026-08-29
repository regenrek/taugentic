use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use portable_pty::{CommandBuilder, MasterPty, PtySize as PortablePtySize, native_pty_system};

use crate::{ExecError, SandboxProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl PtySize {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn portable(self) -> PortablePtySize {
        PortablePtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PtyRequest {
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    env_clear: bool,
    size: PtySize,
    sandbox_profile: Option<SandboxProfile>,
}

impl PtyRequest {
    pub fn new(program: impl Into<OsString>, cwd: impl Into<PathBuf>, size: PtySize) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            env: Vec::new(),
            env_clear: false,
            size,
            sandbox_profile: None,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    pub fn env_clear(mut self, env_clear: bool) -> Self {
        self.env_clear = env_clear;
        self
    }

    pub fn sandbox_profile(mut self, profile: SandboxProfile) -> Self {
        self.sandbox_profile = Some(profile);
        self
    }

    pub fn default_shell(cwd: impl Into<PathBuf>, size: PtySize) -> Self {
        Self::new(default_shell_program(), cwd, size).args(default_shell_args())
    }
}

#[cfg(unix)]
fn default_shell_program() -> OsString {
    std::env::var_os("SHELL")
        .filter(|value| {
            let path = std::path::Path::new(value);
            path.is_absolute() && path.is_file()
        })
        .unwrap_or_else(|| OsString::from("/bin/sh"))
}

#[cfg(unix)]
fn default_shell_args() -> Vec<OsString> {
    vec![OsString::from("-l")]
}

#[cfg(windows)]
fn default_shell_program() -> OsString {
    std::env::var_os("COMSPEC").unwrap_or_else(|| OsString::from("cmd.exe"))
}

#[cfg(windows)]
fn default_shell_args() -> Vec<OsString> {
    Vec::new()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalPtyEngine;

pub struct PtySession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl LocalPtyEngine {
    pub fn spawn(
        &self,
        request: PtyRequest,
    ) -> Result<(Arc<PtySession>, Box<dyn Read + Send>), ExecError> {
        let PtyRequest {
            mut program,
            mut args,
            cwd,
            env,
            env_clear,
            size,
            sandbox_profile,
        } = request;
        if let Some(profile) = sandbox_profile.as_ref() {
            let prepared = ta_sandbox::prepare_current(
                profile,
                ta_sandbox::SandboxCommand::new(program, args),
            )?;
            (program, args) = prepared.into_parts();
        }

        let program = program
            .into_string()
            .map_err(|_| ExecError::Pty("program path is not valid UTF-8".to_string()))?;
        let mut command = CommandBuilder::new(program);
        command.args(args);
        command.cwd(cwd);
        if env_clear
            || sandbox_profile
                .as_ref()
                .is_some_and(|profile| !profile.inherits_all_env())
        {
            command.env_clear();
        }
        if let Some(profile) = sandbox_profile.as_ref() {
            for name in profile.env_allowlist() {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
        for (name, value) in env {
            command.env(name, value);
        }

        let pair = native_pty_system()
            .openpty(size.portable())
            .map_err(|error| ExecError::Pty(error.to_string()))?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| ExecError::Pty(error.to_string()))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| ExecError::Pty(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| ExecError::Pty(error.to_string()))?;
        Ok((
            Arc::new(PtySession {
                master: Mutex::new(pair.master),
                writer: Mutex::new(writer),
                child: Mutex::new(child),
            }),
            reader,
        ))
    }
}

impl PtySession {
    pub fn write_input(&self, bytes: &[u8]) -> Result<(), ExecError> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| ExecError::Pty("PTY writer lock poisoned".to_string()))?;
        writer
            .write_all(bytes)
            .and_then(|()| writer.flush())
            .map_err(|error| ExecError::Pty(error.to_string()))
    }

    pub fn resize(&self, size: PtySize) -> Result<(), ExecError> {
        self.master
            .lock()
            .map_err(|_| ExecError::Pty("PTY master lock poisoned".to_string()))?
            .resize(size.portable())
            .map_err(|error| ExecError::Pty(error.to_string()))
    }

    pub fn close(&self) -> Result<(), ExecError> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| ExecError::Pty("PTY child lock poisoned".to_string()))?;
        if child
            .try_wait()
            .map_err(|error| ExecError::Pty(error.to_string()))?
            .is_some()
        {
            return Ok(());
        }
        crate::local_engine::terminate_pty_child_tree(child.as_mut())?;
        child
            .wait()
            .map(|_| ())
            .map_err(|error| ExecError::Pty(error.to_string()))
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Read as _, time::Duration};

    use super::*;

    #[test]
    fn pty_accepts_input_and_resizes_without_replacing_the_shell() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let request = PtyRequest::new("/bin/sh", temporary.path(), PtySize::new(24, 80))
            .args(["-i"])
            .env("TERM", "xterm-256color");
        let (session, mut reader) = LocalPtyEngine.spawn(request).expect("spawn PTY");
        session.resize(PtySize::new(40, 120)).expect("resize PTY");
        session
            .write_input(b"printf 'taugentic-pty-ready\\n'\nexit\n")
            .expect("write PTY input");

        let mut output = Vec::new();
        let started = std::time::Instant::now();
        let mut buffer = [0_u8; 1024];
        while started.elapsed() < Duration::from_secs(5) {
            let read = reader.read(&mut buffer).expect("read PTY output");
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
            if output
                .windows(20)
                .any(|window| window == b"taugentic-pty-ready")
            {
                break;
            }
        }
        assert!(
            String::from_utf8_lossy(&output).contains("taugentic-pty-ready"),
            "interactive PTY should stream command output"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sandboxed_interactive_shell_can_own_its_terminal_process_group() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let sandbox = SandboxProfile::new()
            .read_path(temporary.path())
            .write_path(temporary.path())
            .child_inherits_tty(true);
        let request = PtyRequest::new("/bin/zsh", temporary.path(), PtySize::new(24, 80))
            .args(["-f", "-i"])
            .env("TERM", "xterm-256color")
            .sandbox_profile(sandbox);
        let (session, mut reader) = LocalPtyEngine.spawn(request).expect("spawn sandboxed PTY");
        session
            .write_input(b"printf 'taugentic-sandboxed-pty-ready\\n'\nexit\n")
            .expect("write PTY input");

        let mut output = Vec::new();
        let started = std::time::Instant::now();
        let mut buffer = [0_u8; 1024];
        while started.elapsed() < Duration::from_secs(5) {
            let read = reader.read(&mut buffer).expect("read PTY output");
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
            if output
                .windows(b"taugentic-sandboxed-pty-ready".len())
                .any(|window| window == b"taugentic-sandboxed-pty-ready")
            {
                break;
            }
        }
        let output = String::from_utf8_lossy(&output);
        assert!(
            output.contains("taugentic-sandboxed-pty-ready"),
            "sandboxed interactive PTY should stream command output: {output}"
        );
        assert!(
            !output.contains("can't set tty pgrp"),
            "sandboxed interactive shell must own its terminal process group: {output}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn closing_a_pty_terminates_its_process_group() {
        let temporary = tempfile::tempdir().expect("tempdir");
        let pid_file = temporary.path().join("child.pid");
        let request = PtyRequest::new("/bin/sh", temporary.path(), PtySize::new(24, 80))
            .args([
                OsString::from("-c"),
                OsString::from("sleep 30 & printf '%s' \"$!\" > \"$TA_TEST_CHILD_PID\"; wait"),
            ])
            .env("TA_TEST_CHILD_PID", pid_file.as_os_str());
        let (session, _reader) = LocalPtyEngine
            .spawn(request)
            .expect("spawn PTY process group");
        let started = std::time::Instant::now();
        let child_pid = loop {
            if let Ok(value) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = value.parse::<i32>()
            {
                break nix::unistd::Pid::from_raw(pid);
            }
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "timed out waiting for PTY child"
            );
            std::thread::sleep(Duration::from_millis(20));
        };

        session.close().expect("close PTY process group");

        let started = std::time::Instant::now();
        loop {
            match nix::sys::signal::kill(child_pid, None) {
                Err(nix::errno::Errno::ESRCH) => break,
                Ok(()) | Err(_) => {
                    assert!(
                        started.elapsed() < Duration::from_secs(3),
                        "PTY descendant survived terminal close"
                    );
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}
