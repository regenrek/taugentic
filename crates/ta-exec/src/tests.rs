use super::*;
use crate::local_engine::{
    effective_sandbox_profile, prepare_sandboxed_command, prepare_sandboxed_command_for_request,
};
use std::ffi::OsString;
use std::time::Duration;

#[cfg(unix)]
use ta_sandbox::{
    PreparedSandboxCommand, SandboxBackend, SandboxCommand, SandboxError, SandboxKind,
};

#[test]
fn shell_request_sets_shell_tool_defaults() {
    let request = SpawnRequest::shell("echo ok", "/tmp");

    assert_eq!(request.stdin, StdioPolicy::Null);
    assert_eq!(request.stdout, StdioPolicy::Piped);
    assert_eq!(request.stderr, StdioPolicy::Piped);
    #[cfg(unix)]
    assert_eq!(request.process_group, ProcessGroupPolicy::New);
    #[cfg(not(unix))]
    assert_eq!(request.process_group, ProcessGroupPolicy::Inherit);
    #[cfg(unix)]
    assert_eq!(request.program, OsString::from("/bin/sh"));
    #[cfg(not(unix))]
    assert_eq!(request.program, OsString::from("cmd"));
    assert!(request.sandbox_profile_ref().is_none());
}

#[test]
fn spawn_request_accepts_sandbox_profile() {
    let profile = SandboxProfile::new()
        .read_path("/repo")
        .network(NetworkPolicy::Off);
    let request = SpawnRequest::new("echo").sandbox_profile(profile.clone());

    assert_eq!(request.sandbox_profile_ref(), Some(&profile));
}

#[test]
fn sandbox_backend_error_is_fail_closed() {
    let backend = ta_sandbox::backend::UnsupportedSandboxBackend::new(
        ta_sandbox::SandboxKind::Unsupported,
        "test backend missing",
    );
    let profile = SandboxProfile::default();

    let error = prepare_sandboxed_command("echo".into(), vec!["ok".into()], &profile, &backend)
        .expect_err("sandbox backend should fail");

    assert!(matches!(
        error,
        ExecError::Sandbox(ta_sandbox::SandboxError::Unsupported {
            reason: "test backend missing",
            ..
        })
    ));
}

#[test]
fn shell_request_with_sandbox_profile_fails_closed_on_unsupported_backend() {
    let backend = ta_sandbox::backend::UnsupportedSandboxBackend::new(
        ta_sandbox::SandboxKind::Unsupported,
        "test backend missing",
    );
    let request = SpawnRequest::shell("echo should-not-run", "/tmp")
        .sandbox_profile(SandboxProfile::default());

    let error = LocalExecEngine
        .spawn_with_backend(request, &backend)
        .expect_err("unsupported sandbox should prevent spawn");

    assert!(matches!(
        error,
        ExecError::Sandbox(ta_sandbox::SandboxError::Unsupported {
            reason: "test backend missing",
            ..
        })
    ));
}

#[test]
fn sandbox_effective_profile_allows_explicit_caller_env_names() {
    let profile = effective_sandbox_profile(
        SandboxProfile::new().env("PATH"),
        &[
            (OsString::from("TA_CALLER_TOKEN"), OsString::from("secret")),
            (
                OsString::from("TA_REMOVED_TOKEN"),
                OsString::from("removed"),
            ),
        ],
        &[OsString::from("TA_REMOVED_TOKEN")],
    );

    assert!(profile.allows_env("PATH"));
    assert!(profile.allows_env("TA_CALLER_TOKEN"));
    assert!(!profile.allows_env("TA_REMOVED_TOKEN"));
}

#[cfg(unix)]
#[test]
fn linux_bwrap_args_mark_effective_caller_env() {
    let prepared = prepare_sandboxed_command_for_request(
        OsString::from("/bin/true"),
        vec![OsString::from("--ok")],
        SandboxProfile::new().network(NetworkPolicy::Off),
        &[(OsString::from("TA_CALLER_TOKEN"), OsString::from("secret"))],
        &[],
        &LinuxLandlockBwrapTestBackend,
    )
    .expect("prepare linux bwrap command");

    assert!(
        prepared.args.contains(&OsString::from(
            ta_sandbox::LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG
        )),
        "Linux bwrap helper args should mark effective caller env: {:?}",
        prepared.args
    );
    assert!(prepared.profile.allows_env("TA_CALLER_TOKEN"));
}

#[cfg(unix)]
#[test]
fn linux_bwrap_args_omit_caller_env_flag_without_effective_env() {
    for (env, env_remove) in [
        (Vec::new(), Vec::new()),
        (
            vec![(OsString::from("TA_CALLER_TOKEN"), OsString::from("secret"))],
            vec![OsString::from("TA_CALLER_TOKEN")],
        ),
    ] {
        let prepared = prepare_sandboxed_command_for_request(
            OsString::from("/bin/true"),
            vec![OsString::from("--ok")],
            SandboxProfile::new().network(NetworkPolicy::Off),
            &env,
            &env_remove,
            &LinuxLandlockBwrapTestBackend,
        )
        .expect("prepare linux bwrap command");

        assert!(
            !prepared.args.contains(&OsString::from(
                ta_sandbox::LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG
            )),
            "Linux bwrap helper args should not mark absent caller env: {:?}",
            prepared.args
        );
        assert!(!prepared.profile.allows_env("TA_CALLER_TOKEN"));
    }
}

#[cfg(unix)]
#[test]
fn sandbox_profile_empty_env_allowlist_hides_parent_env() {
    run_ignored_helper(
        "tests::sandbox_env_helper_empty_env_allowlist",
        &[("TA_TEST_SECRET", "parent-secret")],
    );
}

#[cfg(unix)]
#[test]
fn sandbox_profile_env_allowlist_rehydrates_only_allowed_parent_env() {
    run_ignored_helper(
        "tests::sandbox_env_helper_allowlist",
        &[
            ("TA_TEST_SECRET", "parent-secret"),
            ("TA_TEST_ALLOWED", "parent-allowed"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn sandbox_profile_caller_env_overrides_allowed_parent_env() {
    run_ignored_helper(
        "tests::sandbox_env_helper_caller_override",
        &[("TA_TEST_OVERRIDE", "parent-override")],
    );
}

#[cfg(unix)]
#[test]
fn sandbox_profile_applies_explicit_caller_env_without_parent_allowlist() {
    run_ignored_helper("tests::sandbox_env_helper_applies_explicit_caller_env", &[]);
}

#[cfg(unix)]
#[test]
fn sandbox_profile_env_remove_prunes_allowlist_and_caller_env() {
    run_ignored_helper(
        "tests::sandbox_env_helper_env_remove",
        &[("TA_TEST_REMOVE", "parent-remove")],
    );
}

#[cfg(unix)]
#[test]
fn sandbox_profile_base_allowlist_does_not_rehydrate_provider_api_keys() {
    run_ignored_helper(
        "tests::sandbox_env_helper_provider_api_keys",
        &[
            ("OPENAI_API_KEY", "parent-openai"),
            ("ANTHROPIC_API_KEY", "parent-anthropic"),
            ("GEMINI_API_KEY", "parent-gemini"),
        ],
    );
}

#[cfg(unix)]
fn run_ignored_helper(helper_name: &str, envs: &[(&str, &str)]) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary path"))
        .arg("--ignored")
        .arg("--exact")
        .arg(helper_name)
        .envs(envs.iter().copied())
        .output()
        .expect("run sandbox env helper");

    assert!(
        output.status.success(),
        "sandbox env helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_empty_env_allowlist() {
    let lines = run_sandbox_env_probe(SandboxProfile::new(), &[]).await;

    assert_eq!(lines, ["<unset>", "<unset>", "<unset>", "<unset>"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_allowlist() {
    let lines = run_sandbox_env_probe(SandboxProfile::new().env("TA_TEST_ALLOWED"), &[]).await;

    assert_eq!(lines, ["<unset>", "parent-allowed", "<unset>", "<unset>"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_caller_override() {
    let lines = run_sandbox_env_probe(
        SandboxProfile::new().env("TA_TEST_OVERRIDE"),
        &[("TA_TEST_OVERRIDE", "caller-override")],
    )
    .await;

    assert_eq!(lines, ["<unset>", "<unset>", "caller-override", "<unset>"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_applies_explicit_caller_env() {
    let lines = run_sandbox_env_probe(
        SandboxProfile::new(),
        &[("TA_TEST_CALLER_SECRET", "caller-secret")],
    )
    .await;

    assert_eq!(lines, ["<unset>", "<unset>", "<unset>", "caller-secret"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_env_remove() {
    let lines = run_sandbox_env_remove_probe(
        SandboxProfile::new().env("TA_TEST_REMOVE"),
        &[("TA_TEST_CALLER_REMOVE", "caller-remove")],
    )
    .await;

    assert_eq!(lines, ["<unset>", "<unset>"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "spawned with controlled parent env by sandbox env isolation tests"]
async fn sandbox_env_helper_provider_api_keys() {
    let lines = run_provider_api_key_probe(
        SandboxProfile::new().env("PATH").env("HOME"),
        &[("ANTHROPIC_API_KEY", "explicit-anthropic")],
    )
    .await;

    assert_eq!(lines, ["<unset>", "explicit-anthropic", "<unset>"]);
}

#[cfg(unix)]
async fn run_sandbox_env_probe(
    profile: SandboxProfile,
    caller_env: &[(&str, &str)],
) -> Vec<String> {
    let mut request = SpawnRequest::new("/bin/sh")
        .args([
            "-c",
            r#"printf '%s\n%s\n%s\n%s\n' "${TA_TEST_SECRET-<unset>}" "${TA_TEST_ALLOWED-<unset>}" "${TA_TEST_OVERRIDE-<unset>}" "${TA_TEST_CALLER_SECRET-<unset>}""#,
        ])
        .stdin(StdioPolicy::Null)
        .stdout(StdioPolicy::Piped)
        .stderr(StdioPolicy::Piped)
        .sandbox_profile(profile);
    for (name, value) in caller_env {
        request = request.env(*name, *value);
    }

    let child = LocalExecEngine
        .spawn_with_backend(request, &PassthroughSandboxBackend)
        .expect("spawn env probe");
    let output = child.wait_with_output().await.expect("wait for env probe");
    assert!(
        output.status.success(),
        "env probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("env probe stdout is utf8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(unix)]
async fn run_sandbox_env_remove_probe(
    profile: SandboxProfile,
    caller_env: &[(&str, &str)],
) -> Vec<String> {
    let mut request = SpawnRequest::new("/bin/sh")
        .args([
            "-c",
            r#"printf '%s\n%s\n' "${TA_TEST_REMOVE-<unset>}" "${TA_TEST_CALLER_REMOVE-<unset>}""#,
        ])
        .stdin(StdioPolicy::Null)
        .stdout(StdioPolicy::Piped)
        .stderr(StdioPolicy::Piped)
        .env_remove(["TA_TEST_REMOVE", "TA_TEST_CALLER_REMOVE"])
        .sandbox_profile(profile);
    for (name, value) in caller_env {
        request = request.env(*name, *value);
    }

    let child = LocalExecEngine
        .spawn_with_backend(request, &PassthroughSandboxBackend)
        .expect("spawn env remove probe");
    let output = child
        .wait_with_output()
        .await
        .expect("wait for env remove probe");
    assert!(
        output.status.success(),
        "env remove probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("env remove probe stdout is utf8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(unix)]
async fn run_provider_api_key_probe(
    profile: SandboxProfile,
    caller_env: &[(&str, &str)],
) -> Vec<String> {
    let mut request = SpawnRequest::new("/bin/sh")
        .args([
            "-c",
            r#"printf '%s\n%s\n%s\n' "${OPENAI_API_KEY-<unset>}" "${ANTHROPIC_API_KEY-<unset>}" "${GEMINI_API_KEY-<unset>}""#,
        ])
        .stdin(StdioPolicy::Null)
        .stdout(StdioPolicy::Piped)
        .stderr(StdioPolicy::Piped)
        .sandbox_profile(profile);
    for (name, value) in caller_env {
        request = request.env(*name, *value);
    }

    let child = LocalExecEngine
        .spawn_with_backend(request, &PassthroughSandboxBackend)
        .expect("spawn provider env probe");
    let output = child.wait_with_output().await.expect("wait for env probe");
    assert!(
        output.status.success(),
        "provider env probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("env probe stdout is utf8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct PassthroughSandboxBackend;

#[cfg(unix)]
impl SandboxBackend for PassthroughSandboxBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Unsupported
    }

    fn prepare(
        &self,
        _profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        Ok(PreparedSandboxCommand::new(
            self.kind(),
            command.program().clone(),
            command.args().to_vec(),
        ))
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct LinuxLandlockBwrapTestBackend;

#[cfg(unix)]
impl SandboxBackend for LinuxLandlockBwrapTestBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::LinuxLandlockBwrap
    }

    fn prepare(
        &self,
        _profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        let mut args = vec![command.program().clone()];
        args.extend(command.args().iter().cloned());
        Ok(PreparedSandboxCommand::new(
            self.kind(),
            OsString::from("/opt/taugentic/ta-linux-sandbox"),
            args,
        ))
    }
}

#[cfg(unix)]
#[tokio::test]
#[cfg(target_os = "macos")]
async fn sandbox_wrapper_ignores_caller_path_override() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ta-exec-sandbox-path-test-{}-{unique}",
        std::process::id()
    ));
    let fake_sandbox_exec = root.join("sandbox-exec");
    let sentinel = root.join("fake-sandbox-exec-ran");
    fs::create_dir_all(&root).expect("create test root");
    let sentinel_literal = sentinel.to_string_lossy().replace('\'', "'\"'\"'");
    fs::write(
        &fake_sandbox_exec,
        format!("#!/bin/sh\nprintf fake > '{sentinel_literal}'\nexit 99\n"),
    )
    .expect("write fake sandbox-exec");
    let mut permissions = fs::metadata(&fake_sandbox_exec)
        .expect("fake sandbox-exec metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_sandbox_exec, permissions).expect("chmod fake sandbox-exec");

    let request = SpawnRequest::shell("printf ok", &root)
        .sandbox_profile(
            SandboxProfile::new()
                .read_path(&root)
                .write_path(&root)
                .network(NetworkPolicy::Off)
                .env("PATH"),
        )
        .env("PATH", root.as_os_str());
    let child = LocalExecEngine
        .spawn(request)
        .expect("spawn sandboxed shell");
    let output = child
        .wait_with_output()
        .await
        .expect("wait sandboxed shell");

    assert!(
        output.status.success(),
        "sandboxed shell failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    assert!(
        !sentinel.exists(),
        "caller PATH fake sandbox-exec should not run"
    );

    fs::remove_dir_all(&root).expect("remove test root");
}

#[cfg(unix)]
#[tokio::test]
async fn terminate_child_handles_default_inherited_process_group() {
    let mut child = LocalExecEngine
        .spawn(
            SpawnRequest::new("sleep")
                .arg("30")
                .stdout(StdioPolicy::Null)
                .stderr(StdioPolicy::Null),
        )
        .expect("spawn child");

    let status = terminate_child(&mut child, Duration::from_millis(50))
        .await
        .expect("terminate child");

    assert!(!status.success());
}

#[cfg(unix)]
#[tokio::test]
async fn terminate_child_tree_falls_back_for_inherited_process_group() {
    let mut child = LocalExecEngine
        .spawn(
            SpawnRequest::new("sleep")
                .arg("30")
                .stdout(StdioPolicy::Null)
                .stderr(StdioPolicy::Null),
        )
        .expect("spawn child");

    let status = terminate_child_tree(&mut child, Duration::from_millis(50))
        .await
        .expect("terminate child tree");

    assert!(!status.success());
}

#[cfg(unix)]
#[tokio::test]
async fn terminate_child_tree_kills_new_process_group_descendants() {
    let root = unique_temp_dir("ta-exec-process-tree-test");
    std::fs::create_dir_all(&root).expect("create test root");
    let pid_file = root.join("grandchild.pid");
    let mut child = LocalExecEngine
        .spawn(
            SpawnRequest::new("/bin/sh")
                .args([
                    "-c",
                    r#"sleep 30 & printf '%s' "$!" > "$TA_TEST_GRANDCHILD_PID"; wait"#,
                ])
                .env(
                    "TA_TEST_GRANDCHILD_PID",
                    pid_file.as_os_str().to_os_string(),
                )
                .stdin(StdioPolicy::Null)
                .stdout(StdioPolicy::Null)
                .stderr(StdioPolicy::Null)
                .process_group(ProcessGroupPolicy::New),
        )
        .expect("spawn process tree");
    let grandchild_pid = wait_for_pid_file(&pid_file).await;

    let status = terminate_child_tree(&mut child, Duration::from_millis(50))
        .await
        .expect("terminate child tree");

    assert!(!status.success());
    wait_for_process_exit(grandchild_pid).await;
    std::fs::remove_dir_all(root).expect("remove test root");
}

#[cfg(unix)]
fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{unique}", std::process::id()))
}

#[cfg(unix)]
async fn wait_for_pid_file(path: &std::path::Path) -> nix::unistd::Pid {
    wait_for("pid file", || {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .map(nix::unistd::Pid::from_raw)
    })
    .await
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: nix::unistd::Pid) {
    wait_for("process exit", || (!process_exists(pid)).then_some(())).await;
}

#[cfg(unix)]
async fn wait_for<T>(label: &str, mut poll: impl FnMut() -> Option<T>) -> T {
    let started = std::time::Instant::now();
    loop {
        if let Some(value) = poll() {
            return value;
        }
        if started.elapsed() > Duration::from_secs(3) {
            panic!("timed out waiting for {label}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(unix)]
fn process_exists(pid: nix::unistd::Pid) -> bool {
    match nix::sys::signal::kill(pid, None) {
        Ok(()) => true,
        Err(nix::errno::Errno::ESRCH) => false,
        Err(_) => true,
    }
}
