mod foundation;

use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs,
    fs::File,
    path::{Path, PathBuf},
    process::{self, Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use ta_protocol::wire::DAEMON_SOCKET_NAME_ENV_VAR;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "print-layout" => print_layout(),
        "export-ts" => export_ts()?,
        "export-schema" => export_schema()?,
        "export-protocol" => export_protocol()?,
        "check-daemon-foundation" => foundation::check_daemon_foundation(&repo_root())?,
        "check-protocol" => check_protocol_artifacts()?,
        "check-platforms" => check_platform_targets()?,
        "smoke-local-daemon" => smoke_local_daemon()?,
        _ => print_help(),
    }

    Ok(())
}

fn print_layout() {
    println!("crates/ta-orchestrator");
    println!("crates/ta-cli");
    println!("apps/desktop");
    println!("crates/ta-host-platform");
    println!("crates/ta-protocol");
    println!("crates/ta-policy");
    println!("crates/ta-store");
}

fn print_help() {
    println!("xtask commands:");
    println!("  print-layout");
    println!("  export-ts");
    println!("  export-schema");
    println!("  export-protocol");
    println!("  check-daemon-foundation");
    println!("  check-protocol");
    println!("  check-platforms");
    println!("  smoke-local-daemon");
}

fn check_protocol_artifacts() -> Result<(), Box<dyn Error>> {
    let shared_dir = shared_package_dir();
    let expected_dir = shared_dir.join("generated");
    let temp_root = temp_export_root();

    ta_protocol::export_protocol_artifacts(&temp_root)?;

    let actual_dir = temp_root.join("generated");
    let differences = diff_dirs(&expected_dir, &actual_dir)?;

    if differences.is_empty() {
        println!("protocol artifacts are up to date");
        fs::remove_dir_all(temp_root)?;
        return Ok(());
    }

    for difference in differences {
        eprintln!("{difference}");
    }

    fs::remove_dir_all(temp_root)?;
    Err("generated protocol artifacts are stale; run `cargo xtask export-protocol`".into())
}

fn export_ts() -> Result<(), Box<dyn Error>> {
    let shared_dir = shared_package_dir();
    reset_typescript_output(&shared_dir)?;
    ta_protocol::export_typescript_bindings(&shared_dir)?;
    Ok(())
}

fn export_schema() -> Result<(), Box<dyn Error>> {
    let shared_dir = shared_package_dir();
    reset_schema_output(&shared_dir)?;
    ta_protocol::export_json_schemas(&shared_dir)?;
    Ok(())
}

fn export_protocol() -> Result<(), Box<dyn Error>> {
    let shared_dir = shared_package_dir();
    reset_generated_output(&shared_dir)?;
    ta_protocol::export_protocol_artifacts(&shared_dir)?;
    Ok(())
}

fn check_platform_targets() -> Result<(), Box<dyn Error>> {
    for target in ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"] {
        run_cargo_check(target)?;
    }

    Ok(())
}

fn smoke_local_daemon() -> Result<(), Box<dyn Error>> {
    run_cargo_build(&["-p", "ta-orchestrator", "-p", "ta-cli"])?;

    let socket_name = unique_smoke_socket_name();
    let isolated_home = unique_smoke_home_dir(&socket_name);
    let daemon_binary = cargo_binary_path("ta-daemon");
    let cli_binary = cargo_binary_path("ta-cli");

    {
        let mut daemon = ManagedDaemon::spawn(&daemon_binary, &socket_name, &isolated_home)?;
        wait_for_daemon_status(&mut daemon, &cli_binary, &socket_name, &isolated_home)?;
        daemon.stop()?;
    }

    {
        let mut daemon = ManagedDaemon::spawn(&daemon_binary, &socket_name, &isolated_home)?;
        wait_for_daemon_status(&mut daemon, &cli_binary, &socket_name, &isolated_home)?;
        daemon.stop()?;
    }

    if isolated_home.exists() {
        fs::remove_dir_all(&isolated_home)?;
    }

    println!("local daemon smoke passed for socket name `{socket_name}`");
    Ok(())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}

fn shared_package_dir() -> PathBuf {
    repo_root().join("apps/desktop/packages/shared")
}

fn reset_generated_output(shared_dir: &Path) -> Result<(), Box<dyn Error>> {
    let generated_dir = shared_dir.join("generated");

    if generated_dir.exists() {
        fs::remove_dir_all(&generated_dir)?;
    }

    fs::create_dir_all(generated_dir.join("schema"))?;
    Ok(())
}

fn reset_typescript_output(shared_dir: &Path) -> Result<(), Box<dyn Error>> {
    let generated_dir = shared_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;
    remove_files_with_extension(&generated_dir, "ts")?;
    Ok(())
}

fn reset_schema_output(shared_dir: &Path) -> Result<(), Box<dyn Error>> {
    let schema_dir = shared_dir.join("generated/schema");

    if schema_dir.exists() {
        fs::remove_dir_all(&schema_dir)?;
    }

    fs::create_dir_all(&schema_dir)?;
    Ok(())
}

fn temp_export_root() -> PathBuf {
    env::temp_dir().join(format!("taugentic-protocol-export-{}", process::id()))
}

fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("target"))
}

fn cargo_binary_path(binary_name: &str) -> PathBuf {
    let binary_file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    target_dir().join("debug").join(binary_file_name)
}

fn unique_smoke_socket_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    format!("tsm-{:x}-{:x}", process::id(), nanos)
}

fn unique_smoke_home_dir(socket_name: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "taugentic-smoke-home-{}-{socket_name}",
        process::id()
    ))
}

fn apply_isolated_home_env(command: &mut Command, isolated_home: &Path) {
    command
        .env("HOME", isolated_home)
        .env("USERPROFILE", isolated_home)
        .env("XDG_CONFIG_HOME", isolated_home.join(".config"))
        .env("APPDATA", isolated_home.join("AppData").join("Roaming"));
}

fn diff_dirs(expected_root: &Path, actual_root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let expected_files = file_map(expected_root)?;
    let actual_files = file_map(actual_root)?;
    let all_paths = expected_files
        .keys()
        .chain(actual_files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut differences = Vec::new();

    for relative_path in all_paths {
        match (
            expected_files.get(&relative_path),
            actual_files.get(&relative_path),
        ) {
            (Some(expected), Some(actual)) if expected == actual => {}
            (Some(_), Some(_)) => {
                differences.push(format!("modified: {}", relative_path.display()))
            }
            (Some(_), None) => differences.push(format!("missing: {}", relative_path.display())),
            (None, Some(_)) => differences.push(format!("unexpected: {}", relative_path.display())),
            (None, None) => {}
        }
    }

    Ok(differences)
}

fn file_map(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let mut files = std::collections::BTreeMap::new();

    if !root.exists() {
        return Ok(files);
    }

    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }

        let extension = path.extension().and_then(|ext| ext.to_str());
        let is_declaration = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".d.ts"));

        if is_declaration || !matches!(extension, Some("ts" | "js" | "json")) {
            continue;
        }

        let relative_path = path.strip_prefix(root)?.to_path_buf();
        files.insert(relative_path, fs::read(&path)?);
    }

    Ok(())
}

fn remove_files_with_extension(dir: &Path, extension: &str) -> Result<(), Box<dyn Error>> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            remove_files_with_extension(&path, extension)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn run_cargo_check(target: &str) -> Result<(), Box<dyn Error>> {
    let status = cargo_command([
        "check",
        "-p",
        "ta-host-platform",
        "-p",
        "ta-orchestrator",
        "-p",
        "ta-cli",
        "--target",
        target,
    ])?
    .status()?;

    if status.success() {
        return Ok(());
    }

    Err(format!("platform check failed for target `{target}`").into())
}

fn run_cargo_build(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let status = cargo_command(std::iter::once("build").chain(args.iter().copied()))?.status()?;

    if status.success() {
        return Ok(());
    }

    Err(format!("cargo build failed for args `{}`", args.join(" ")).into())
}

fn cargo_command<I, S>(args: I) -> Result<Command, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = Command::new("cargo");
    command.args(args).current_dir(repo_root());
    Ok(command)
}

fn wait_for_daemon_status(
    daemon: &mut ManagedDaemon,
    cli_binary: &Path,
    socket_name: &str,
    isolated_home: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut last_cli_error = String::new();

    for _attempt in 0..40 {
        if let Some(status) = daemon.child.try_wait()? {
            return Err(format!(
                "ta-daemon exited before reporting ready: {status}\n{}",
                daemon.log_excerpt()?
            )
            .into());
        }

        let mut command = Command::new(cli_binary);
        command
            .args(["daemon", "status", "--json", "--socket", socket_name])
            .current_dir(repo_root());
        apply_isolated_home_env(&mut command, isolated_home);
        let output = command.output()?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            if daemon_status_output_is_ready(&stdout)? {
                return Ok(());
            }

            return Err(format!("unexpected ta-cli daemon status output:\n{stdout}").into());
        }

        last_cli_error = format_cli_failure(output.status, &output.stdout, &output.stderr);
        thread::sleep(Duration::from_millis(250));
    }

    Err(format!(
        "timed out waiting for ta-daemon readiness via ta-cli\nlast cli failure: {last_cli_error}\n{}",
        daemon.log_excerpt()?
    )
    .into())
}

fn format_cli_failure(status: ExitStatus, stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    format!("status={status}; stdout={stdout:?}; stderr={stderr:?}")
}

fn daemon_status_output_is_ready(stdout: &str) -> Result<bool, serde_json::Error> {
    let payload: Value = serde_json::from_str(stdout)?;
    let ready = payload
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let socket_path = payload
        .get("socketPath")
        .and_then(Value::as_str)
        .unwrap_or_default();

    Ok(ready && !socket_path.trim().is_empty())
}

struct ManagedDaemon {
    child: Child,
    log_path: PathBuf,
}

impl ManagedDaemon {
    fn spawn(
        binary: &Path,
        socket_name: &str,
        isolated_home: &Path,
    ) -> Result<Self, Box<dyn Error>> {
        let log_path = env::temp_dir().join(format!(
            "taugentic-daemon-smoke-{}-{}.log",
            process::id(),
            socket_name
        ));
        fs::create_dir_all(isolated_home)?;
        let log_file = File::create(&log_path)?;
        let log_file_for_stderr = log_file.try_clone()?;
        let mut command = Command::new(binary);
        command
            .env(DAEMON_SOCKET_NAME_ENV_VAR, socket_name)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_for_stderr))
            .current_dir(repo_root());
        apply_isolated_home_env(&mut command, isolated_home);
        let child = command.spawn()?;

        Ok(Self { child, log_path })
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
            let _ = self.child.wait()?;
        }
        Ok(())
    }

    fn log_excerpt(&self) -> Result<String, Box<dyn Error>> {
        let contents = fs::read_to_string(&self.log_path)?;
        if contents.trim().is_empty() {
            return Ok(format!("daemon log: {} is empty", self.log_path.display()));
        }

        Ok(format!(
            "daemon log ({}):\n{}",
            self.log_path.display(),
            contents.trim()
        ))
    }
}

impl Drop for ManagedDaemon {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::daemon_status_output_is_ready;

    #[test]
    fn accepts_ready_json_for_requested_socket_name() {
        let stdout = r#"{"ready":true,"socketPath":"/tmp/ta-daemon-smoke.sock","version":"0.0.1"}"#;
        let ready = daemon_status_output_is_ready(stdout).expect("status JSON should parse");

        assert!(ready);
    }

    #[test]
    fn rejects_ready_json_when_not_ready() {
        let stdout =
            r#"{"ready":false,"socketPath":"/tmp/ta-daemon-smoke.sock","version":"0.0.1"}"#;
        let ready = daemon_status_output_is_ready(stdout).expect("status JSON should parse");

        assert!(!ready);
    }

    #[test]
    fn rejects_ready_json_for_missing_socket_path() {
        let stdout = r#"{"ready":true,"version":"0.0.1"}"#;
        let ready = daemon_status_output_is_ready(stdout).expect("status JSON should parse");

        assert!(!ready);
    }
}
