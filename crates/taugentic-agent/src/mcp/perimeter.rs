use std::{
    fs,
    path::{Path, PathBuf},
};

use ta_exec::{NetworkPolicy, SandboxProfile};
use ta_protocol::provider_id;

use crate::ExecutionError;

/// Builds the MCP stdio sandbox perimeter with base parent-env rehydration only.
///
/// `env_allowlist` is intentionally limited to baseline OS variables. MCP
/// `spec.env` is the user-controlled secret bridge into the child and
/// deliberately bypasses that allowlist, so operators should configure only the
/// server-specific secrets each MCP server actually needs.
pub fn build_mcp_perimeter_profile(
    server_id: &str,
    workspace_cwd: &Path,
    command: &Path,
) -> Result<SandboxProfile, ExecutionError> {
    validate_mcp_server_id(server_id)?;
    require_absolute_path("workspace cwd", workspace_cwd)?;
    require_absolute_path("MCP stdio command", command)?;

    let workspace_root = workspace_cwd.canonicalize().map_err(|error| {
        ExecutionError::InvalidConfig(format!(
            "MCP perimeter sandbox workspace cwd must exist: {}: {error}",
            workspace_cwd.display()
        ))
    })?;
    let home = home_dir()?;
    let cache_dir = mcp_cache_dir(&home, server_id)?;
    validate_mcp_cache_symlink_target(&home, &cache_dir)?;
    let temp_dir = std::env::temp_dir();
    require_absolute_path("temp dir", &temp_dir)?;

    // Most third-party MCP servers need network access. Linux remains fail-closed
    // through the sandbox backend until the network allowlist slice lands.
    let mut profile = SandboxProfile::new()
        .network(NetworkPolicy::Open)
        .child_inherits_tty(false)
        .read_path(&workspace_root)
        .write_path(&workspace_root)
        .read_path(&cache_dir)
        .write_path(&cache_dir)
        .read_path(&temp_dir)
        .write_path(temp_dir);

    for path in system_read_paths() {
        profile = profile.read_path(path);
    }
    for path in command_read_paths(command, &home, &workspace_root, &cache_dir)? {
        profile = profile.read_path(path);
    }
    for name in perimeter_env_allowlist() {
        profile = profile.env(name);
    }

    Ok(profile)
}

fn validate_mcp_server_id(server_id: &str) -> Result<(), ExecutionError> {
    provider_id::validate_provider_id(server_id).map_err(|error| {
        ExecutionError::InvalidConfig(format!(
            "invalid MCP server id {:?}: {}",
            error.provider_id(),
            error.reason()
        ))
    })
}

fn require_absolute_path(label: &str, path: &Path) -> Result<(), ExecutionError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ExecutionError::InvalidConfig(format!(
            "MCP perimeter sandbox requires absolute {label}: {}",
            path.display()
        )))
    }
}

fn home_dir() -> Result<PathBuf, ExecutionError> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        ExecutionError::InvalidConfig(
            "MCP perimeter sandbox requires HOME to scope the MCP cache".to_string(),
        )
    })?;
    let home = PathBuf::from(home);
    require_absolute_path("HOME", &home)?;
    Ok(home)
}

fn mcp_cache_dir(home: &Path, server_id: &str) -> Result<PathBuf, ExecutionError> {
    validate_mcp_server_id(server_id)?;
    Ok(home.join(".cache").join(server_id))
}

fn validate_mcp_cache_symlink_target(home: &Path, cache_dir: &Path) -> Result<(), ExecutionError> {
    let metadata = match fs::symlink_metadata(cache_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ExecutionError::InvalidConfig(format!(
                "MCP perimeter sandbox could not inspect MCP cache path: {}: {error}",
                cache_dir.display()
            )));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(());
    }

    let target = cache_dir.canonicalize().map_err(|error| {
        ExecutionError::InvalidConfig(format!(
            "MCP perimeter sandbox refuses unresolved MCP cache symlink: {}: {error}",
            cache_dir.display()
        ))
    })?;
    let home_roots = candidate_paths(home);
    let cache_roots = candidate_paths(cache_dir);
    if home_roots
        .iter()
        .any(|home_root| target.starts_with(home_root))
        && !cache_roots
            .iter()
            .any(|cache_root| target.starts_with(cache_root))
    {
        return Err(ExecutionError::InvalidConfig(format!(
            "MCP perimeter sandbox refuses MCP cache symlink target under HOME outside the server cache: cache={}, target={}",
            cache_dir.display(),
            target.display(),
        )));
    }

    Ok(())
}

fn system_read_paths() -> Vec<PathBuf> {
    [
        "/bin",
        "/sbin",
        "/usr",
        "/System",
        "/Library/Apple",
        "/private/var/db",
        "/opt/homebrew",
        "/usr/local",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn command_read_paths(
    command: &Path,
    home: &Path,
    workspace_root: &Path,
    cache_dir: &Path,
) -> Result<Vec<PathBuf>, ExecutionError> {
    let mut paths = Vec::new();
    if let Some(parent) = command.parent() {
        push_command_read_path(
            &mut paths,
            command,
            parent,
            home,
            workspace_root,
            cache_dir,
            "command parent",
        )?;
    }
    if let Ok(canonical) = command.canonicalize() {
        if let Some(parent) = canonical.parent() {
            push_command_read_path(
                &mut paths,
                command,
                parent,
                home,
                workspace_root,
                cache_dir,
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
            workspace_root,
            cache_dir,
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
    workspace_root: &Path,
    cache_dir: &Path,
    label: &str,
) -> Result<(), ExecutionError> {
    validate_command_read_path(command, path, home, workspace_root, cache_dir, label)?;
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
    workspace_root: &Path,
    cache_dir: &Path,
    label: &str,
) -> Result<(), ExecutionError> {
    if is_disallowed_home_command_path(path, home, workspace_root, cache_dir) {
        return Err(ExecutionError::InvalidConfig(format!(
            "MCP perimeter sandbox refuses to grant {label} under HOME outside workspace or server cache: command={}, path={}, workspace={}, cache={}",
            command.display(),
            path.display(),
            workspace_root.display(),
            cache_dir.display(),
        )));
    }
    Ok(())
}

fn is_disallowed_home_command_path(
    path: &Path,
    home: &Path,
    workspace_root: &Path,
    cache_dir: &Path,
) -> bool {
    candidate_paths(home)
        .iter()
        .any(|home_root| path.starts_with(home_root))
        && !candidate_paths(workspace_root)
            .iter()
            .any(|workspace| path.starts_with(workspace))
        && !candidate_paths(cache_dir)
            .iter()
            .any(|cache| path.starts_with(cache))
}

fn candidate_paths(path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![path.to_path_buf()];
    if let Ok(canonical) = path.canonicalize()
        && !paths.iter().any(|existing| existing == &canonical)
    {
        paths.push(canonical);
    }
    paths
}

fn perimeter_env_allowlist() -> [&'static str; 4] {
    ["PATH", "HOME", "LANG", "TMPDIR"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn mcp_perimeter_profile_allows_expected_boundary() {
        let workspace = std::env::temp_dir();
        let command = Path::new("/opt/homebrew/bin/mcp-server");

        let profile =
            build_mcp_perimeter_profile("github-mcp", &workspace, command).expect("profile");

        assert_eq!(profile.network_policy(), &NetworkPolicy::Open);
        assert!(!profile.child_inherits_tty_enabled());
        assert!(profile.reads_path(&workspace));
        assert!(profile.writes_path(&workspace));
        assert!(profile.reads_path(Path::new("/usr/bin/env")));
        assert!(profile.reads_path(command));
        assert!(profile.writes_path(&std::env::temp_dir()));
        if let Some(home) = std::env::var_os("HOME") {
            let cache = PathBuf::from(home).join(".cache").join("github-mcp");
            assert!(profile.reads_path(&cache));
            assert!(profile.writes_path(&cache));
            assert!(!profile.writes_path(&cache.with_file_name(".ssh")));
        }
    }

    #[test]
    fn mcp_perimeter_profile_rejects_relative_command() {
        let error =
            build_mcp_perimeter_profile("github-mcp", &std::env::temp_dir(), Path::new("npx"))
                .expect_err("relative command should fail closed");

        assert!(matches!(
            error,
            ExecutionError::InvalidConfig(message)
                if message.contains("absolute MCP stdio command")
        ));
    }

    #[test]
    fn mcp_perimeter_profile_rejects_relative_home() {
        const CHILD_MARKER: &str = "TAUGENTIC_AGENT_MCP_RELATIVE_HOME_CHILD";

        if std::env::var_os(CHILD_MARKER).is_some() {
            let error = build_mcp_perimeter_profile(
                "github-mcp",
                &std::env::temp_dir(),
                Path::new("/usr/bin/mcp-server"),
            )
            .expect_err("relative HOME should fail closed");

            assert!(matches!(
                error,
                ExecutionError::InvalidConfig(message) if message.contains("absolute HOME")
            ));
            return;
        }

        let test_name = "mcp::perimeter::tests::mcp_perimeter_profile_rejects_relative_home";
        let output = Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg(test_name)
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
    fn mcp_server_id_uses_shared_segment_contract() {
        for valid in provider_id::VALID_PROVIDER_ID_TEST_CASES {
            validate_mcp_server_id(valid).expect("valid provider id");
        }
        for invalid in provider_id::INVALID_PROVIDER_ID_TEST_CASES {
            assert!(matches!(
                validate_mcp_server_id(invalid),
                Err(ExecutionError::InvalidConfig(_))
            ));
        }
    }

    #[test]
    fn mcp_perimeter_profile_rejects_escape_server_id() {
        let error = build_mcp_perimeter_profile(
            "../escape",
            &std::env::temp_dir(),
            Path::new("/usr/bin/mcp-server"),
        )
        .expect_err("unsafe server id should fail closed");

        assert!(matches!(
            error,
            ExecutionError::InvalidConfig(message)
                if message.contains("invalid MCP server id")
                    && message.contains("../escape")
        ));
    }

    #[test]
    fn mcp_perimeter_env_allowlist_is_base_only() {
        let profile = build_mcp_perimeter_profile(
            "github-mcp",
            &std::env::temp_dir(),
            Path::new("/opt/homebrew/bin/mcp-server"),
        )
        .expect("profile");

        assert_eq!(profile.env_allowlist(), ["PATH", "HOME", "LANG", "TMPDIR"]);
        assert!(!profile.allows_env("GITHUB_TOKEN"));
        assert!(!profile.allows_env("OPENAI_API_KEY"));
        assert!(!profile.allows_env("ANTHROPIC_API_KEY"));
        assert!(!profile.allows_env("LC_ALL"));
        assert!(!profile.allows_env("TMP"));
    }
}
