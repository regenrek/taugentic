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
        "print-daemon-binary" => print_daemon_binary()?,
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

fn print_daemon_binary() -> Result<(), Box<dyn Error>> {
    let cargo_target_directory = cargo_metadata_target_directory()?;
    println!(
        "{}",
        cargo_binary_path(&cargo_target_directory, "ta-daemon").display()
    );
    Ok(())
}

fn print_help() {
    println!("xtask commands:");
    println!("  print-layout");
    println!("  print-daemon-binary");
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
    let expected_changes = export_protocol_expected_changes()?;
    let candidate_root = create_candidate_root()?;
    let result = (|| {
        ta_protocol::export_protocol_artifacts(&candidate_root)?;
        publish_generated_candidate(
            &shared_dir.join("generated"),
            &candidate_root.join("generated"),
            &expected_changes,
        )
    })();
    let cleanup = fs::remove_dir_all(&candidate_root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(cleanup)) => Err(cleanup.into()),
        (Err(primary), Err(cleanup)) => {
            Err(format!("{primary}; candidate cleanup also failed: {cleanup}").into())
        }
    }
}

/// Publication is local to one filesystem and assumes no concurrent mutator.
fn create_candidate_root() -> Result<PathBuf, Box<dyn Error>> {
    let root = env::temp_dir().join(format!(
        "taugentic-protocol-export-{}-{}",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&root)?;
    Ok(root)
}

#[derive(Debug, Default)]
struct ExpectedGeneratedChanges {
    writes: BTreeSet<PathBuf>,
    deletes: BTreeSet<PathBuf>,
}

impl ExpectedGeneratedChanges {
    fn all(&self) -> BTreeSet<PathBuf> {
        self.writes.union(&self.deletes).cloned().collect()
    }

    fn validate(&self) -> Result<(), Box<dyn Error>> {
        let paths = self.all();
        if paths.is_empty() {
            return Err(
                "export-protocol requires at least one --expect-write or --expect-delete declaration"
                    .into(),
            );
        }
        if !self.writes.is_disjoint(&self.deletes) {
            return Err("--expect-write and --expect-delete declarations must be disjoint".into());
        }
        for path in paths {
            validate_generated_relative_path(&path)?;
        }
        Ok(())
    }
}

fn export_protocol_expected_changes() -> Result<ExpectedGeneratedChanges, Box<dyn Error>> {
    let mut expected = ExpectedGeneratedChanges::default();
    let mut args = env::args().skip(2);
    while let Some(argument) = args.next() {
        let path = args
            .next()
            .ok_or("each export-protocol declaration requires a generated relative path")?;
        let path = normalize_public_generated_path(&path)?;
        match argument.as_str() {
            "--expect-write" => {
                expected.writes.insert(path);
            }
            "--expect-delete" => {
                expected.deletes.insert(path);
            }
            _ => return Err(format!("unknown export-protocol argument: {argument}").into()),
        }
    }
    expected.validate()?;
    Ok(expected)
}

fn normalize_public_generated_path(value: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = Path::new(value);
    let mut components = path.components();
    if !matches!(components.next(), Some(std::path::Component::Normal(name)) if name == "generated")
    {
        return Err("--expect-change paths must start with generated/".into());
    }
    let relative = components.collect::<PathBuf>();
    validate_generated_relative_path(&relative)?;
    Ok(relative)
}

/// Publishes an already-generated candidate without ever replacing the live
/// directory. Validation and every non-closure snapshot happen before the
/// first live write; each closure file is installed with an atomic rename.
fn publish_generated_candidate(
    live: &Path,
    candidate: &Path,
    expected_changes: &ExpectedGeneratedChanges,
) -> Result<(), Box<dyn Error>> {
    publish_generated_candidate_with_failure(live, candidate, expected_changes, None)
}

fn publish_generated_candidate_with_failure(
    live: &Path,
    candidate: &Path,
    expected_changes: &ExpectedGeneratedChanges,
    fail_after: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    publish_generated_candidate_with_failures(live, candidate, expected_changes, fail_after, None)
}

fn publish_generated_candidate_with_failures(
    live: &Path,
    candidate: &Path,
    expected_changes: &ExpectedGeneratedChanges,
    fail_after: Option<usize>,
    fail_preservation_read_after: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    validate_generated_root(live)?;
    validate_generated_root(candidate)?;
    expected_changes.validate()?;
    let expected_paths = expected_changes.all();
    let live_files = file_map(live)?;
    let candidate_files = file_map(candidate)?;
    let paths = live_files
        .keys()
        .chain(candidate_files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let changed = paths
        .iter()
        .filter(|path| live_files.get(*path) != candidate_files.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if changed != expected_paths {
        return Err(format!(
            "candidate changed paths do not match declared closure; expected {:?}, got {:?}",
            expected_paths, changed
        )
        .into());
    }
    if expected_changes
        .writes
        .iter()
        .any(|path| !candidate_files.contains_key(path))
    {
        return Err("candidate is missing a declared generated write path".into());
    }
    if expected_changes
        .deletes
        .iter()
        .any(|path| !live_files.contains_key(path) || candidate_files.contains_key(path))
    {
        return Err("declared generated delete must remove a live regular file".into());
    }
    let snapshots = live_files
        .iter()
        .filter(|(path, _)| !expected_paths.contains(*path))
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let before = expected_paths
        .iter()
        .map(|path| (path.clone(), live_files.get(path).cloned()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut published = Vec::new();
    for path in &expected_changes.writes {
        let mut publish = || -> Result<(), Box<dyn Error>> {
            if fail_after == Some(published.len()) {
                return Err("injected later publication failure".into());
            }
            atomic_write_generated_file(live, path, candidate_files.get(path).expect("validated"))?;
            published.push(path.clone());
            Ok(())
        };
        if let Err(error) = publish() {
            return rollback_or_error(live, &before, &published, error);
        }
    }
    for path in &expected_changes.deletes {
        if fail_after == Some(published.len()) {
            return rollback_or_error(
                live,
                &before,
                &published,
                "injected later publication failure".into(),
            );
        }
        if let Err(error) = delete_generated_file(live, path) {
            return rollback_or_error(live, &before, &published, error);
        }
        published.push(path.clone());
    }
    for (index, (path, bytes)) in snapshots.into_iter().enumerate() {
        let actual = if fail_preservation_read_after == Some(index) {
            Err(std::io::Error::other("injected preservation read failure"))
        } else {
            fs::read(live.join(&path))
        };
        let actual = match actual {
            Ok(actual) => actual,
            Err(error) => return rollback_or_error(live, &before, &published, Box::new(error)),
        };
        if actual != bytes {
            return rollback_or_error(
                live,
                &before,
                &published,
                "non-closure generated file changed during publication".into(),
            );
        }
    }
    Ok(())
}

fn delete_generated_file(live: &Path, relative: &Path) -> Result<(), Box<dyn Error>> {
    validate_generated_relative_path(relative)?;
    let destination = live.join(relative);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_file() => fs::remove_file(destination)?,
        Ok(_) => return Err("generated deletion target must be a normal file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("generated deletion target is missing".into());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn rollback_or_error(
    live: &Path,
    before: &std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    published: &[PathBuf],
    primary: Box<dyn Error>,
) -> Result<(), Box<dyn Error>> {
    match rollback_generated_closure(live, before, published) {
        Ok(()) => Err(primary),
        Err(rollback) => Err(format!("{primary}; rollback also failed: {rollback}").into()),
    }
}

fn validate_generated_root(root: &Path) -> Result<(), Box<dyn Error>> {
    if root.exists() && fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err("generated root must not be a symlink".into());
    }
    Ok(())
}

fn validate_generated_relative_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("generated path must be a non-empty relative normal path".into());
    }
    for component in path.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("generated path contains a non-normal component".into());
        }
    }
    Ok(())
}

fn atomic_write_generated_file(
    live: &Path,
    relative: &Path,
    bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    validate_generated_relative_path(relative)?;
    let destination = live.join(relative);
    let parent = destination.parent().ok_or("generated path has no parent")?;
    ensure_normal_parent(live, parent)?;
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err("generated destination must be a normal file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let unique = format!(
        ".publish-{}-{}",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let staged = parent.join(unique);
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        use std::io::Write;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&staged, destination)?;
        Ok(())
    })();
    if result.is_ok() {
        return Ok(());
    }
    let primary = result.expect_err("checked error");
    match fs::remove_file(&staged) {
        Ok(()) => Err(primary),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(primary),
        Err(cleanup) => Err(format!("{primary}; stage cleanup also failed: {cleanup}").into()),
    }
}

fn ensure_normal_parent(root: &Path, parent: &Path) -> Result<(), Box<dyn Error>> {
    let relative = parent.strip_prefix(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("generated parent escapes its root".into());
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("generated parent must not be a symlink".into());
            }
            Ok(_) => return Err("generated parent is not a directory".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn rollback_generated_closure(
    live: &Path,
    before: &std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    published: &[PathBuf],
) -> Result<(), Box<dyn Error>> {
    for path in published.iter().rev() {
        let bytes = before
            .get(path)
            .expect("published path must have a snapshot");
        match bytes {
            Some(bytes) => atomic_write_generated_file(live, path, bytes)?,
            None => delete_generated_file(live, path)?,
        }
    }
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
    let cargo_target_directory = cargo_metadata_target_directory()?;
    let daemon_binary = cargo_binary_path(&cargo_target_directory, "ta-daemon");
    let cli_binary = cargo_binary_path(&cargo_target_directory, "ta-cli");

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

fn cargo_metadata_target_directory() -> Result<PathBuf, Box<dyn Error>> {
    let output = cargo_command(["metadata", "--no-deps", "--format-version=1"])?.output()?;

    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            format_cli_failure(output.status, &output.stdout, &output.stderr)
        )
        .into());
    }

    cargo_metadata_target_directory_from_output(&output.stdout).map_err(Into::into)
}

fn cargo_metadata_target_directory_from_output(metadata: &[u8]) -> Result<PathBuf, String> {
    let payload: Value = serde_json::from_slice(metadata)
        .map_err(|error| format!("cargo metadata returned invalid JSON: {error}"))?;
    let target_directory = payload
        .get("target_directory")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "cargo metadata omitted a non-empty target_directory".to_string())?;

    Ok(PathBuf::from(target_directory))
}

fn cargo_binary_path(cargo_target_directory: &Path, binary_name: &str) -> PathBuf {
    let binary_file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    cargo_target_directory.join("debug").join(binary_file_name)
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
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            return Err("generated traversal rejects symlinks".into());
        }
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }
        if !file_type.is_file() {
            return Err("generated traversal rejects special files".into());
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
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };

    use super::{
        ExpectedGeneratedChanges, cargo_metadata_target_directory_from_output,
        create_candidate_root, daemon_status_output_is_ready, publish_generated_candidate,
        publish_generated_candidate_with_failure, publish_generated_candidate_with_failures,
    };

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test root");
        root
    }

    fn write(root: &Path, path: &str, value: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("parent");
        fs::write(path, value).expect("write");
    }

    fn expected(writes: &[&str], deletes: &[&str]) -> ExpectedGeneratedChanges {
        ExpectedGeneratedChanges {
            writes: writes.iter().map(PathBuf::from).collect(),
            deletes: deletes.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn protocol_export_rejects_mismatch_before_live_write() {
        let root = test_root("mismatch");
        let live = root.join("live");
        let candidate = root.join("candidate");
        write(&live, "generated/keep.ts", "keep");
        write(&live, "generated/change.ts", "old");
        write(&candidate, "generated/keep.ts", "keep");
        write(&candidate, "generated/change.ts", "new");
        write(&candidate, "generated/unexpected.ts", "new");
        let closure = expected(&["change.ts"], &["obsolete.ts"]);
        assert!(
            publish_generated_candidate(
                &live.join("generated"),
                &candidate.join("generated"),
                &closure
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(live.join("generated/change.ts")).expect("live"),
            "old"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_creates_a_unique_candidate_root() {
        let candidate = create_candidate_root().expect("candidate root");
        assert!(candidate.is_dir());
        assert!(
            fs::symlink_metadata(&candidate)
                .expect("metadata")
                .file_type()
                .is_dir()
        );
        fs::remove_dir_all(candidate).expect("cleanup");
    }

    #[test]
    fn protocol_export_publishes_only_exact_closure_and_preserves_bytes() {
        let root = test_root("closure");
        let live = root.join("live");
        let candidate = root.join("candidate");
        write(&live, "generated/keep.ts", "keep");
        write(&live, "generated/schema/keep.json", "{\"keep\":true}");
        write(&live, "generated/change.ts", "old");
        write(&candidate, "generated/keep.ts", "keep");
        write(&candidate, "generated/schema/keep.json", "{\"keep\":true}");
        write(&candidate, "generated/change.ts", "new");
        write(&live, "generated/obsolete.ts", "obsolete");
        let closure = expected(&["change.ts"], &["obsolete.ts"]);
        publish_generated_candidate(
            &live.join("generated"),
            &candidate.join("generated"),
            &closure,
        )
        .expect("publish");
        assert_eq!(
            fs::read_to_string(live.join("generated/change.ts")).expect("changed"),
            "new"
        );
        assert_eq!(
            fs::read_to_string(live.join("generated/keep.ts")).expect("keep"),
            "keep"
        );
        assert_eq!(
            fs::read_to_string(live.join("generated/schema/keep.json")).expect("schema"),
            "{\"keep\":true}"
        );
        assert!(!live.join("generated/obsolete.ts").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_accepts_write_only_delete_only_and_mixed_declarations() {
        let root = test_root("declaration-sides");

        let write_live = root.join("write/live");
        let write_candidate = root.join("write/candidate");
        write(&write_live, "keep.ts", "keep");
        write(&write_candidate, "keep.ts", "keep");
        write(&write_candidate, "created.ts", "created");
        publish_generated_candidate(
            &write_live,
            &write_candidate,
            &expected(&["created.ts"], &[]),
        )
        .expect("write-only declaration publishes");
        assert_eq!(
            fs::read_to_string(write_live.join("created.ts")).expect("created"),
            "created"
        );

        let delete_live = root.join("delete/live");
        let delete_candidate = root.join("delete/candidate");
        write(&delete_live, "obsolete.ts", "obsolete");
        publish_generated_candidate(
            &delete_live,
            &delete_candidate,
            &expected(&[], &["obsolete.ts"]),
        )
        .expect("delete-only declaration publishes");
        assert!(!delete_live.join("obsolete.ts").exists());

        let mixed_live = root.join("mixed/live");
        let mixed_candidate = root.join("mixed/candidate");
        write(&mixed_live, "obsolete.ts", "obsolete");
        write(&mixed_candidate, "created.ts", "created");
        publish_generated_candidate(
            &mixed_live,
            &mixed_candidate,
            &expected(&["created.ts"], &["obsolete.ts"]),
        )
        .expect("mixed declaration publishes");
        assert_eq!(
            fs::read_to_string(mixed_live.join("created.ts")).expect("created"),
            "created"
        );
        assert!(!mixed_live.join("obsolete.ts").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_rejects_empty_undeclared_and_false_declarations_before_mutation() {
        let root = test_root("declaration-rejections");

        let empty_live = root.join("empty/live");
        let empty_candidate = root.join("empty/candidate");
        write(&empty_live, "keep.ts", "keep");
        write(&empty_candidate, "keep.ts", "changed");
        assert!(
            publish_generated_candidate(&empty_live, &empty_candidate, &expected(&[], &[]))
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(empty_live.join("keep.ts")).expect("keep"),
            "keep"
        );

        let undeclared_write_live = root.join("undeclared-write/live");
        let undeclared_write_candidate = root.join("undeclared-write/candidate");
        write(&undeclared_write_candidate, "actual.ts", "actual");
        assert!(
            publish_generated_candidate(
                &undeclared_write_live,
                &undeclared_write_candidate,
                &expected(&["declared.ts"], &[]),
            )
            .is_err()
        );
        assert!(!undeclared_write_live.join("actual.ts").exists());

        let undeclared_delete_live = root.join("undeclared-delete/live");
        let undeclared_delete_candidate = root.join("undeclared-delete/candidate");
        write(&undeclared_delete_live, "actual.ts", "actual");
        assert!(
            publish_generated_candidate(
                &undeclared_delete_live,
                &undeclared_delete_candidate,
                &expected(&[], &["declared.ts"]),
            )
            .is_err()
        );
        assert!(undeclared_delete_live.join("actual.ts").exists());

        let false_live = root.join("false/live");
        let false_candidate = root.join("false/candidate");
        write(&false_live, "unchanged.ts", "same");
        write(&false_candidate, "unchanged.ts", "same");
        assert!(
            publish_generated_candidate(
                &false_live,
                &false_candidate,
                &expected(&["unchanged.ts"], &[]),
            )
            .is_err()
        );
        assert!(
            publish_generated_candidate(
                &false_live,
                &false_candidate,
                &expected(&[], &["unchanged.ts"]),
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(false_live.join("unchanged.ts")).expect("unchanged"),
            "same"
        );

        assert!(
            ExpectedGeneratedChanges {
                writes: [PathBuf::from("../escape.ts")].into_iter().collect(),
                deletes: BTreeSet::new(),
            }
            .validate()
            .is_err()
        );
        assert!(
            ExpectedGeneratedChanges {
                writes: [PathBuf::from("same.ts")].into_iter().collect(),
                deletes: [PathBuf::from("same.ts")].into_iter().collect(),
            }
            .validate()
            .is_err()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_rolls_back_a_later_publish_failure() {
        let root = test_root("rollback");
        let live = root.join("live/generated");
        let candidate = root.join("candidate/generated");
        write(&live, "first.ts", "old-first");
        write(&live, "obsolete.ts", "old-obsolete");
        write(&candidate, "first.ts", "new-first");
        let closure = expected(&["first.ts"], &["obsolete.ts"]);
        assert!(
            publish_generated_candidate_with_failure(&live, &candidate, &closure, Some(1)).is_err()
        );
        assert_eq!(
            fs::read_to_string(live.join("first.ts")).expect("first"),
            "old-first"
        );
        assert_eq!(
            fs::read_to_string(live.join("obsolete.ts")).expect("obsolete"),
            "old-obsolete"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_rolls_back_a_preservation_read_failure() {
        let root = test_root("preservation-read");
        let live = root.join("live/generated");
        let candidate = root.join("candidate/generated");
        write(&live, "change.ts", "old");
        write(&live, "obsolete.ts", "obsolete");
        write(&live, "keep.ts", "keep");
        write(&candidate, "change.ts", "new");
        write(&candidate, "keep.ts", "keep");
        let closure = expected(&["change.ts"], &["obsolete.ts"]);
        assert!(
            publish_generated_candidate_with_failures(&live, &candidate, &closure, None, Some(0))
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(live.join("change.ts")).expect("change"),
            "old"
        );
        assert_eq!(
            fs::read_to_string(live.join("obsolete.ts")).expect("obsolete"),
            "obsolete"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn protocol_export_rejects_symlinked_generated_paths() {
        use std::os::unix::fs::symlink;
        let root = test_root("symlink");
        let live = root.join("live/generated");
        let candidate = root.join("candidate/generated");
        write(&live, "change.ts", "old");
        write(&live, "obsolete.ts", "obsolete");
        write(&candidate, "change.ts", "new");
        symlink(root.join("elsewhere"), candidate.join("linked.ts")).expect("symlink");
        let closure = expected(&["change.ts"], &["obsolete.ts"]);
        assert!(publish_generated_candidate(&live, &candidate, &closure).is_err());
        assert_eq!(
            fs::read_to_string(live.join("change.ts")).expect("live"),
            "old"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protocol_export_preserves_every_regular_file_extension() {
        let root = test_root("all-extensions");
        let live = root.join("live/generated");
        let candidate = root.join("candidate/generated");
        write(&live, "change.bin", "old");
        write(&live, "obsolete.custom", "obsolete");
        write(&live, "keep.d.ts", "declaration");
        write(&live, "keep.no-extension", "keep");
        write(&candidate, "change.bin", "new");
        write(&candidate, "keep.d.ts", "declaration");
        write(&candidate, "keep.no-extension", "keep");
        let closure = expected(&["change.bin"], &["obsolete.custom"]);
        publish_generated_candidate(&live, &candidate, &closure).expect("publish");
        assert_eq!(
            fs::read_to_string(live.join("change.bin")).expect("change"),
            "new"
        );
        assert_eq!(
            fs::read_to_string(live.join("keep.d.ts")).expect("declaration"),
            "declaration"
        );
        assert_eq!(
            fs::read_to_string(live.join("keep.no-extension")).expect("keep"),
            "keep"
        );
        assert!(!live.join("obsolete.custom").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_target_directory_from_cargo_metadata_output() {
        let metadata = br#"{"target_directory":"/opt/build/taugentic-target"}"#;

        let target_directory = cargo_metadata_target_directory_from_output(metadata)
            .expect("metadata target_directory should parse");

        assert_eq!(
            target_directory,
            PathBuf::from("/opt/build/taugentic-target")
        );
    }

    #[test]
    fn rejects_cargo_metadata_without_a_non_empty_target_directory() {
        let error = cargo_metadata_target_directory_from_output(br#"{"target_directory":"  "}"#)
            .expect_err("missing target_directory must fail without a fallback");

        assert!(error.contains("non-empty target_directory"));
    }

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
