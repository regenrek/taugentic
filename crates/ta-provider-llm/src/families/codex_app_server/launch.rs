use std::{
    fs,
    path::{Path, PathBuf},
};

use ta_exec::{NetworkPolicy, SandboxProfile};
use ta_protocol::{provider_id, wire::EnvPolicy};

use super::{CODEX_PROVIDER_ID, CodexLlmClientError};

pub const CODEX_CACHE_DIR_NAME: &str = ".codex";

pub fn build_codex_perimeter_profile_for_context(
    workspace_cwd: &Path,
    command: &Path,
    env_policy: &EnvPolicy,
) -> Result<SandboxProfile, CodexLlmClientError> {
    validate_codex_provider_id(CODEX_PROVIDER_ID)?;
    require_absolute_path("workspace cwd", workspace_cwd)?;
    require_absolute_path("Codex app-server command", command)?;

    let workspace_root = workspace_cwd.canonicalize().map_err(|error| {
        CodexLlmClientError::InvalidConfig(format!(
            "Codex perimeter sandbox workspace cwd must exist: {}: {error}",
            workspace_cwd.display()
        ))
    })?;
    let home = home_dir()?;
    let codex_cache = home.join(CODEX_CACHE_DIR_NAME);
    validate_codex_cache_symlink_target(&home, &codex_cache)?;
    let temp_dir = std::env::temp_dir();
    require_absolute_path("temp dir", &temp_dir)?;

    // Codex may open network from its own child runtime; Linux Landlock rejects
    // this profile until the network allowlist slice lands, so spawn fails closed.
    let mut profile = SandboxProfile::new()
        .network(NetworkPolicy::Open)
        .child_inherits_tty(false)
        .read_path(&workspace_root)
        .write_path(&workspace_root)
        .read_path(&codex_cache)
        .write_path(&codex_cache)
        .read_path(&temp_dir)
        .write_path(temp_dir);

    for path in system_read_paths() {
        profile = profile.read_path(path);
    }
    for path in command_read_paths(command, &home)? {
        profile = profile.read_path(path);
    }
    for name in env_allowlist(env_policy)? {
        profile = profile.env(name);
    }

    Ok(profile)
}

fn require_absolute_path(label: &str, path: &Path) -> Result<(), CodexLlmClientError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(CodexLlmClientError::InvalidConfig(format!(
            "Codex perimeter sandbox requires absolute {label}: {}",
            path.display()
        )))
    }
}

fn home_dir() -> Result<PathBuf, CodexLlmClientError> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        CodexLlmClientError::InvalidConfig(
            "Codex perimeter sandbox requires HOME to scope the Codex cache".to_string(),
        )
    })?;
    let home = PathBuf::from(home);
    require_absolute_path("HOME", &home)?;
    Ok(home)
}

fn system_read_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        "/bin",
        "/sbin",
        "/usr",
        "/System",
        "/Library/Apple",
        "/private/var/db",
        "/opt/homebrew",
        "/usr/local",
    ];
    #[cfg(target_os = "macos")]
    paths.push("/private/etc/ssl");
    paths.into_iter().map(PathBuf::from).collect()
}

fn command_read_paths(command: &Path, home: &Path) -> Result<Vec<PathBuf>, CodexLlmClientError> {
    let mut paths = Vec::new();
    // Current sandbox backends model read grants as subpaths, so command
    // execution still grants the smallest safe directory containing the binary.
    if let Some(parent) = command.parent() {
        push_command_read_path(&mut paths, command, parent, home, "command parent")?;
    }
    if let Ok(canonical) = command.canonicalize() {
        if let Some(parent) = canonical.parent() {
            push_command_read_path(
                &mut paths,
                command,
                parent,
                home,
                "canonical command parent",
            )?;
        }
    } else if let Some(parent) = command.parent()
        && let Ok(canonical_parent) = parent.canonicalize()
    {
        push_command_read_path(
            &mut paths,
            command,
            &canonical_parent,
            home,
            "canonical command parent",
        )?;
    }
    Ok(paths)
}

fn push_command_read_path(
    paths: &mut Vec<PathBuf>,
    command: &Path,
    path: &Path,
    home: &Path,
    label: &str,
) -> Result<(), CodexLlmClientError> {
    validate_command_read_path(command, path, home, label)?;
    let path = path.to_path_buf();
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
    Ok(())
}

fn validate_command_read_path(
    command: &Path,
    path: &Path,
    home: &Path,
    label: &str,
) -> Result<(), CodexLlmClientError> {
    let allowed_roots = allowed_codex_roots(home);
    if is_disallowed_home_command_path(path, home) {
        return Err(CodexLlmClientError::InvalidConfig(format!(
            "Codex perimeter sandbox refuses to grant {label} under HOME outside {allowed}: command={}, path={}",
            command.display(),
            path.display(),
            allowed = display_paths(&allowed_roots),
        )));
    }
    Ok(())
}

fn validate_codex_cache_symlink_target(
    home: &Path,
    codex_cache: &Path,
) -> Result<(), CodexLlmClientError> {
    let metadata = match fs::symlink_metadata(codex_cache) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(CodexLlmClientError::InvalidConfig(format!(
                "Codex perimeter sandbox could not inspect HOME .codex path: {}: {error}",
                codex_cache.display()
            )));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(());
    }

    let target = codex_cache.canonicalize().map_err(|error| {
        CodexLlmClientError::InvalidConfig(format!(
            "Codex perimeter sandbox refuses unresolved HOME .codex symlink: {}: {error}",
            codex_cache.display()
        ))
    })?;
    let home_roots = candidate_paths(home);
    let allowed_roots = allowed_codex_roots(home);
    if home_roots
        .iter()
        .any(|home_root| target.starts_with(home_root))
        && !allowed_roots
            .iter()
            .any(|codex_root| target.starts_with(codex_root))
    {
        return Err(CodexLlmClientError::InvalidConfig(format!(
            "Codex perimeter sandbox refuses HOME .codex symlink target under HOME outside allowed Codex roots: cache={}, target={}, allowed={}",
            codex_cache.display(),
            target.display(),
            display_paths(&allowed_roots),
        )));
    }

    Ok(())
}

fn is_disallowed_home_command_path(path: &Path, home: &Path) -> bool {
    let home_roots = candidate_paths(home);
    let codex_roots = allowed_codex_roots(home);

    home_roots
        .iter()
        .any(|home_root| path.starts_with(home_root))
        && !codex_roots
            .iter()
            .any(|codex_root| path.starts_with(codex_root))
}

fn allowed_codex_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![home.join(CODEX_CACHE_DIR_NAME)];
    if let Ok(canonical_home) = home.canonicalize() {
        push_unique_path(&mut roots, canonical_home.join(CODEX_CACHE_DIR_NAME));
    }
    roots
}

fn candidate_paths(path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![path.to_path_buf()];
    if let Ok(canonical) = path.canonicalize() {
        push_unique_path(&mut paths, canonical);
    }
    paths
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn validate_codex_provider_id(provider_id: &str) -> Result<(), CodexLlmClientError> {
    provider_id::validate_provider_id(provider_id).map_err(|error| {
        CodexLlmClientError::InvalidConfig(format!(
            "invalid Codex provider id {:?}: {}",
            error.provider_id(),
            error.reason()
        ))
    })
}

fn env_allowlist(env_policy: &EnvPolicy) -> Result<&[String], CodexLlmClientError> {
    let EnvPolicy::Allowlist { vars } = env_policy else {
        return Err(CodexLlmClientError::InvalidConfig(
            "Codex perimeter requires an explicit environment allowlist".to_string(),
        ));
    };
    for required in ["PATH", "HOME", "TMPDIR"] {
        if !vars.iter().any(|name| name == required) {
            return Err(CodexLlmClientError::InvalidConfig(format!(
                "Codex perimeter environment allowlist is missing {required}"
            )));
        }
    }
    Ok(vars)
}

#[cfg(test)]
fn build_codex_perimeter_profile(
    workspace_cwd: &Path,
    command: &Path,
) -> Result<SandboxProfile, CodexLlmClientError> {
    build_codex_perimeter_profile_for_context(
        workspace_cwd,
        command,
        &EnvPolicy::workspace_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs as unix_fs;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn codex_perimeter_profile_allows_expected_boundary() {
        let workspace = std::env::temp_dir();
        let command = Path::new("/opt/homebrew/bin/codex");

        let profile =
            build_codex_perimeter_profile(&workspace, command).expect("codex perimeter profile");

        assert_eq!(profile.network_policy(), &NetworkPolicy::Open);
        assert!(!profile.child_inherits_tty_enabled());
        assert!(profile.reads_path(&workspace));
        assert!(profile.writes_path(&workspace));
        assert!(profile.reads_path(Path::new("/usr/bin/env")));
        assert!(profile.reads_path(command));
        assert!(profile.writes_path(&std::env::temp_dir()));
        if let Some(home) = std::env::var_os("HOME") {
            let cache = PathBuf::from(home).join(CODEX_CACHE_DIR_NAME);
            assert!(profile.reads_path(&cache));
            assert!(profile.writes_path(&cache));
            assert!(!profile.writes_path(&cache.with_file_name(".ssh")));
        }
    }

    #[test]
    fn codex_perimeter_profile_rejects_relative_program() {
        let error = build_codex_perimeter_profile(&std::env::temp_dir(), Path::new("codex"))
            .expect_err("relative command should fail closed");

        assert!(matches!(
            error,
            CodexLlmClientError::InvalidConfig(message)
                if message.contains("absolute Codex app-server command")
        ));
    }

    #[test]
    fn codex_perimeter_profile_rejects_relative_home() {
        const CHILD_MARKER: &str = "TA_PROVIDER_LLM_RELATIVE_HOME_CHILD";

        if std::env::var_os(CHILD_MARKER).is_some() {
            let error =
                build_codex_perimeter_profile(&std::env::temp_dir(), Path::new("/usr/bin/codex"))
                    .expect_err("relative HOME should fail closed");

            assert!(matches!(
                error,
                CodexLlmClientError::InvalidConfig(message) if message.contains("absolute HOME")
            ));
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg("families::codex_app_server::launch::tests::codex_perimeter_profile_rejects_relative_home")
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env("HOME", "relative-home")
            .output()
            .expect("run isolated relative HOME test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "relative HOME child test failed\nstdout:\n{}\nstderr:\n{}",
            stdout,
            stderr
        );
    }

    #[test]
    fn codex_provider_id_uses_shared_segment_contract() {
        for valid in provider_id::VALID_PROVIDER_ID_TEST_CASES {
            validate_codex_provider_id(valid).expect("valid provider id");
        }
        for invalid in provider_id::INVALID_PROVIDER_ID_TEST_CASES {
            assert!(matches!(
                validate_codex_provider_id(invalid),
                Err(CodexLlmClientError::InvalidConfig(_))
            ));
        }
    }

    #[test]
    fn codex_perimeter_profile_rejects_sensitive_home_command_dirs() {
        if !inside_isolated_home_child() {
            run_isolated_home_child("codex_perimeter_profile_rejects_sensitive_home_command_dirs");
            return;
        }

        let home = isolated_home();
        for sensitive_dir in [".ssh", ".aws"] {
            let command = home.join(sensitive_dir).join("codex");
            let error = build_codex_perimeter_profile(&std::env::temp_dir(), &command)
                .expect_err("sensitive HOME command should fail closed");

            assert!(matches!(
                error,
                CodexLlmClientError::InvalidConfig(message)
                    if message.contains("under HOME outside")
                        && message.contains(CODEX_CACHE_DIR_NAME)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn codex_perimeter_profile_rejects_canonical_sensitive_home_command_parent() {
        if !inside_isolated_home_child() {
            run_isolated_home_child(
                "codex_perimeter_profile_rejects_canonical_sensitive_home_command_parent",
            );
            return;
        }

        let home = isolated_home();
        let root = isolated_root();
        let sensitive_dir = home.join(".ssh");
        fs::create_dir_all(&sensitive_dir).expect("create sensitive dir");
        fs::write(sensitive_dir.join("codex"), "#!/bin/sh\n").expect("write codex target");
        let symlink_dir = root.join("linked-codex-bin");
        unix_fs::symlink(&sensitive_dir, &symlink_dir).expect("symlink command parent");

        let command = symlink_dir.join("codex");
        let error = build_codex_perimeter_profile(&std::env::temp_dir(), &command)
            .expect_err("canonical sensitive HOME command should fail closed");

        assert!(matches!(
            error,
            CodexLlmClientError::InvalidConfig(message)
                if message.contains("canonical command parent")
                    && message.contains("under HOME outside")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn codex_perimeter_profile_rejects_codex_cache_symlink_to_sensitive_home_sibling() {
        if !inside_isolated_home_child() {
            run_isolated_home_child(
                "codex_perimeter_profile_rejects_codex_cache_symlink_to_sensitive_home_sibling",
            );
            return;
        }

        let home = isolated_home();
        for sensitive_dir in [".ssh", ".aws"] {
            let target = home.join(sensitive_dir);
            let command_target = target.join("bin").join("codex");
            fs::create_dir_all(command_target.parent().expect("command parent"))
                .expect("create sensitive command parent");
            fs::write(&command_target, "#!/bin/sh\n").expect("write codex target");

            let codex_cache = home.join(CODEX_CACHE_DIR_NAME);
            unix_fs::symlink(&target, &codex_cache).expect("symlink .codex to sensitive dir");
            let command = codex_cache.join("bin").join("codex");
            let error = build_codex_perimeter_profile(&std::env::temp_dir(), &command)
                .expect_err("sensitive HOME .codex symlink should fail closed");

            assert!(matches!(
                error,
                CodexLlmClientError::InvalidConfig(message)
                    if message.contains("HOME .codex symlink target under HOME outside allowed Codex roots")
                        && message.contains(sensitive_dir)
            ));
            fs::remove_file(&codex_cache).expect("remove .codex symlink");
        }
    }

    #[cfg(unix)]
    #[test]
    fn codex_perimeter_profile_allows_codex_cache_symlink_outside_home() {
        if !inside_isolated_home_child() {
            run_isolated_home_child(
                "codex_perimeter_profile_allows_codex_cache_symlink_outside_home",
            );
            return;
        }

        let home = isolated_home();
        let external_cache = isolated_root().join("opt-codex").join("cache");
        let command_target = external_cache.join("bin").join("codex");
        fs::create_dir_all(command_target.parent().expect("command parent"))
            .expect("create external command parent");
        fs::write(&command_target, "#!/bin/sh\n").expect("write codex target");

        let codex_cache = home.join(CODEX_CACHE_DIR_NAME);
        unix_fs::symlink(&external_cache, &codex_cache).expect("symlink .codex outside HOME");
        let command = codex_cache.join("bin").join("codex");
        let profile = build_codex_perimeter_profile(&std::env::temp_dir(), &command)
            .expect("outside-HOME .codex symlink should be allowed");

        assert!(profile.reads_path(&command));
        assert!(profile.reads_path(&command_target));
    }

    #[test]
    fn codex_perimeter_profile_allows_codex_cache_command_dir() {
        if !inside_isolated_home_child() {
            run_isolated_home_child("codex_perimeter_profile_allows_codex_cache_command_dir");
            return;
        }

        let command = isolated_home()
            .join(CODEX_CACHE_DIR_NAME)
            .join("bin")
            .join("codex");
        let profile = build_codex_perimeter_profile(&std::env::temp_dir(), &command)
            .expect("Codex cache command dir should be allowed");

        assert!(profile.reads_path(&command));
        let sensitive_home_dir = isolated_home().join(".ssh");
        assert!(
            !profile
                .fs_read_paths()
                .iter()
                .any(|read_path| read_path.starts_with(&sensitive_home_dir)),
            "Codex command allowlist must not expose sibling HOME directories"
        );
    }

    #[test]
    fn codex_perimeter_profile_allows_system_command_dirs() {
        for command in [
            Path::new("/usr/local/bin/codex"),
            Path::new("/opt/codex/bin/codex"),
        ] {
            let profile = build_codex_perimeter_profile(&std::env::temp_dir(), command)
                .expect("system command dir should be allowed");

            assert!(profile.reads_path(command));
        }
    }

    #[test]
    fn codex_perimeter_env_allowlist_comes_from_execution_context() {
        let profile = build_codex_perimeter_profile(
            &std::env::temp_dir(),
            Path::new("/opt/homebrew/bin/codex"),
        )
        .expect("codex perimeter profile");

        assert!(profile.allows_env("NIX_SSL_CERT_FILE"));
        assert!(profile.allows_env("SSL_CERT_FILE"));
        assert!(!profile.allows_env("OPENAI_API_KEY"));
        assert!(!profile.allows_env("ANTHROPIC_API_KEY"));
        assert!(!profile.allows_env("LC_ALL"));
        assert!(!profile.allows_env("TMP"));
    }

    fn inside_isolated_home_child() -> bool {
        std::env::var_os("TA_PROVIDER_LLM_ISOLATED_HOME_CHILD").is_some()
    }

    fn isolated_home() -> PathBuf {
        PathBuf::from(
            std::env::var_os("TA_PROVIDER_LLM_TEST_HOME").expect("isolated HOME test env"),
        )
    }

    fn isolated_root() -> PathBuf {
        PathBuf::from(
            std::env::var_os("TA_PROVIDER_LLM_TEST_ROOT").expect("isolated root test env"),
        )
    }

    fn run_isolated_home_child(test_name: &str) {
        let root = unique_temp_root(test_name);
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create isolated HOME");

        let test_path = format!("families::codex_app_server::launch::tests::{test_name}");
        let output = Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg(&test_path)
            .arg("--nocapture")
            .env("TA_PROVIDER_LLM_ISOLATED_HOME_CHILD", "1")
            .env("TA_PROVIDER_LLM_TEST_HOME", &home)
            .env("TA_PROVIDER_LLM_TEST_ROOT", &root)
            .env("HOME", &home)
            .output()
            .expect("run isolated HOME test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "isolated HOME child test {test_name} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    fn unique_temp_root(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ta-provider-llm-{test_name}-{}-{unique}",
            std::process::id()
        ))
    }
}
