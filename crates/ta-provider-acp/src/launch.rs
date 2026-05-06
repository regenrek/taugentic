use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use ta_exec::{NetworkPolicy, SandboxProfile};
use ta_protocol::wire::{AgentRuntimeModelId, RuntimeExtensionState, RuntimePolicyMode};

use crate::{
    adapter::{AcpProcessConfig, DEFAULT_CANCEL_GRACE},
    descriptor::{AcpLaunchKind, AcpProviderSpec, validate_provider_id},
    error::AcpClientError,
    mcp::{extensions_include_http_mcp, extensions_to_mcp_servers},
    mode_mapping::{ModeMapping, translate},
    provider_env::ProviderEnv,
    search_path,
};

#[derive(Debug, Clone, Copy)]
pub struct AcpLaunchInput<'a> {
    pub policy_mode: RuntimePolicyMode,
    pub working_directory: &'a Path,
    pub runtime_extensions: &'a [RuntimeExtensionState],
    pub model_id: Option<&'a AgentRuntimeModelId>,
}

#[tracing::instrument(skip(input), fields(provider_id = %provider.provider_id(), work_dir = %input.working_directory.display()))]
pub fn build_config(
    provider: &AcpProviderSpec,
    input: AcpLaunchInput<'_>,
) -> Result<AcpProcessConfig, AcpClientError> {
    let env = ProviderEnv::from_process_env().child_env(provider);
    let command = search_path::resolve(provider.binary_name(), provider.env_override_var())?;
    let sandbox_profile = build_perimeter_profile(provider, input.working_directory, &command)?;
    match provider.launch_kind() {
        AcpLaunchKind::Codex => Ok(codex_config(provider, command, env, sandbox_profile, input)),
        AcpLaunchKind::Claude => claude_config(provider, command, env, sandbox_profile, input),
        AcpLaunchKind::Cursor => Ok(cursor_config(
            provider,
            command,
            env,
            sandbox_profile,
            input,
        )),
        AcpLaunchKind::OpenCode => Ok(opencode_config(
            provider,
            command,
            env,
            sandbox_profile,
            input,
        )),
        AcpLaunchKind::Copilot => copilot_config(provider, command, env, sandbox_profile, input),
    }
}

pub fn build_perimeter_profile(
    provider: &AcpProviderSpec,
    workspace_cwd: &Path,
    command: &Path,
) -> Result<SandboxProfile, AcpClientError> {
    require_absolute_path("workspace cwd", workspace_cwd)?;
    require_absolute_path("ACP command", command)?;

    let workspace_root = workspace_cwd.canonicalize().map_err(|error| {
        AcpClientError::InvalidConfig(format!(
            "ACP perimeter sandbox workspace cwd must exist: {}: {error}",
            workspace_cwd.display()
        ))
    })?;
    let provider_cache = provider_cache_dir(provider)?;
    let mut profile = SandboxProfile::new()
        .network(NetworkPolicy::Open)
        .child_inherits_tty(false)
        .read_path(&workspace_root)
        .write_path(&workspace_root)
        .read_path(&provider_cache)
        .write_path(provider_cache);

    for path in system_read_paths() {
        profile = profile.read_path(path);
    }
    for path in command_read_paths(command) {
        profile = profile.read_path(path);
    }
    for name in perimeter_env_allowlist() {
        profile = profile.env(name);
    }

    Ok(profile)
}

fn require_absolute_path(label: &str, path: &Path) -> Result<(), AcpClientError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(AcpClientError::InvalidConfig(format!(
            "ACP perimeter sandbox requires absolute {label}: {}",
            path.display()
        )))
    }
}

fn provider_cache_dir(provider: &AcpProviderSpec) -> Result<PathBuf, AcpClientError> {
    validate_provider_id(provider.provider_id())
        .map_err(|error| AcpClientError::InvalidConfig(error.to_string()))?;
    let home = std::env::var_os("HOME").ok_or_else(|| {
        AcpClientError::InvalidConfig(
            "ACP perimeter sandbox requires HOME to scope the provider cache".to_string(),
        )
    })?;
    let home = PathBuf::from(home);
    require_absolute_path("HOME", &home)?;

    // Provider cache roots may be created after launch; validate the configured
    // root here and leave symlink/TOCTOU enforcement to the sandbox backend.
    Ok(home.join(".cache").join(provider.provider_id()))
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

fn command_read_paths(command: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(parent) = command.parent() {
        paths.push(parent.to_path_buf());
    }
    if let Ok(canonical) = command.canonicalize()
        && let Some(parent) = canonical.parent()
    {
        paths.push(parent.to_path_buf());
    }
    paths
}

fn perimeter_env_allowlist() -> [&'static str; 8] {
    [
        "PATH", "HOME", "LANG", "LC_ALL", "LC_CTYPE", "TMPDIR", "TEMP", "TMP",
    ]
}

fn codex_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> AcpProcessConfig {
    let mapping = codex_mode_mapping();
    let args = codex_args(
        input.policy_mode,
        extensions_include_http_mcp(input.runtime_extensions),
    );
    AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args,
        env,
        env_remove: Vec::new(),
        work_dir: input.working_directory.to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
        mode_mapping: mapping,
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn claude_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> Result<AcpProcessConfig, AcpClientError> {
    let mapping = claude_mode_mapping();
    Ok(AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args: Vec::new(),
        env,
        env_remove: vec!["CLAUDECODE".to_string()],
        work_dir: input.working_directory.to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: Some(translate(input.policy_mode, &mapping)?),
        session_model_id: acp_session_model_id(input.model_id),
        mode_mapping: mapping,
        cancel_grace: DEFAULT_CANCEL_GRACE,
    })
}

fn cursor_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> AcpProcessConfig {
    AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args: vec!["acp".to_string()],
        env,
        env_remove: Vec::new(),
        work_dir: input.working_directory.to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
        mode_mapping: ModeMapping::new(),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn opencode_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> AcpProcessConfig {
    AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args: vec!["acp".to_string()],
        env,
        env_remove: Vec::new(),
        work_dir: input.working_directory.to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
        mode_mapping: ModeMapping::new(),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn copilot_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> Result<AcpProcessConfig, AcpClientError> {
    let mapping = copilot_mode_mapping();
    Ok(AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args: copilot_args(input.model_id),
        env,
        env_remove: Vec::new(),
        work_dir: input.working_directory.to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: Some(translate(input.policy_mode, &mapping)?),
        session_model_id: acp_session_model_id(input.model_id),
        mode_mapping: mapping,
        cancel_grace: DEFAULT_CANCEL_GRACE,
    })
}

fn acp_session_model_id(model_id: Option<&AgentRuntimeModelId>) -> Option<String> {
    model_id.map(|model_id| model_id.as_str().to_string())
}

pub(crate) fn codex_args(policy_mode: RuntimePolicyMode, has_http_mcp: bool) -> Vec<String> {
    let (approval_policy, sandbox_mode) = codex_policy(policy_mode);
    let mut args = vec![
        "-c".to_string(),
        format!("approval_policy={approval_policy}"),
        "-c".to_string(),
        format!("sandbox_mode={sandbox_mode}"),
    ];
    if has_http_mcp {
        args.extend([
            "-c".to_string(),
            "sandbox_workspace_write.network_access=true".to_string(),
        ]);
    }
    args
}

fn codex_policy(policy_mode: RuntimePolicyMode) -> (&'static str, &'static str) {
    match policy_mode {
        RuntimePolicyMode::Allow => ("never", "danger-full-access"),
        RuntimePolicyMode::RequireApproval => ("on-request", "read-only"),
        RuntimePolicyMode::Deny => ("never", "read-only"),
    }
}

fn codex_mode_mapping() -> ModeMapping {
    HashMap::from([
        (RuntimePolicyMode::Allow, "full-access".to_string()),
        (RuntimePolicyMode::RequireApproval, "read-only".to_string()),
        (RuntimePolicyMode::Deny, "read-only".to_string()),
    ])
}

fn claude_mode_mapping() -> ModeMapping {
    HashMap::from([
        (RuntimePolicyMode::Allow, "bypassPermissions".to_string()),
        (RuntimePolicyMode::RequireApproval, "default".to_string()),
        (RuntimePolicyMode::Deny, "plan".to_string()),
    ])
}

const COPILOT_MODE_AGENT: &str = "https://agentclientprotocol.com/protocol/session-modes#agent";
const COPILOT_MODE_PLAN: &str = "https://agentclientprotocol.com/protocol/session-modes#plan";

fn copilot_args(model_id: Option<&AgentRuntimeModelId>) -> Vec<String> {
    let mut args = vec!["--acp".to_string()];
    if let Some(model_id) = model_id {
        args.push("--model".to_string());
        args.push(model_id.as_str().to_string());
    }
    args
}

fn copilot_mode_mapping() -> ModeMapping {
    HashMap::from([
        (RuntimePolicyMode::Allow, COPILOT_MODE_AGENT.to_string()),
        (
            RuntimePolicyMode::RequireApproval,
            COPILOT_MODE_AGENT.to_string(),
        ),
        (RuntimePolicyMode::Deny, COPILOT_MODE_PLAN.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perimeter_profile_allows_expected_acp_boundary() {
        use crate::provider_env::{ANTHROPIC_API_KEY_ENV, GEMINI_API_KEY_ENV, OPENAI_API_KEY_ENV};

        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
        let workspace = std::env::temp_dir().join("taugentic-acp-workspace-boundary");
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let command = Path::new("/opt/homebrew/bin/cursor-agent");

        let profile = build_perimeter_profile(&provider, &workspace, command).expect("profile");

        assert_eq!(profile.network_policy(), &NetworkPolicy::Open);
        assert!(!profile.child_inherits_tty_enabled());
        assert!(profile.reads_path(&workspace));
        assert!(profile.writes_path(&workspace));
        assert!(profile.allows_env("PATH"));
        assert!(profile.allows_env("HOME"));
        assert!(!profile.allows_env(ANTHROPIC_API_KEY_ENV));
        assert!(!profile.allows_env(OPENAI_API_KEY_ENV));
        assert!(!profile.allows_env(GEMINI_API_KEY_ENV));
        assert!(!profile.writes_path(Path::new("/tmp/.ssh/id_rsa")));
        if let Some(home) = std::env::var_os("HOME") {
            let cache = PathBuf::from(home)
                .join(".cache")
                .join(provider.provider_id());
            assert!(profile.reads_path(&cache));
            assert!(profile.writes_path(&cache));
            assert!(!profile.writes_path(&cache.with_file_name(".ssh")));
        }
    }

    #[test]
    fn perimeter_profile_rejects_relative_adapter_program() {
        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Codex);
        let error =
            build_perimeter_profile(&provider, &std::env::temp_dir(), Path::new("codex-acp"))
                .expect_err("relative command should fail closed");

        assert!(matches!(
            error,
            AcpClientError::InvalidConfig(message) if message.contains("absolute ACP command")
        ));
    }

    #[test]
    fn perimeter_profile_rejects_relative_home() {
        const CHILD_MARKER: &str = "TA_PROVIDER_ACP_RELATIVE_HOME_CHILD";

        if std::env::var_os(CHILD_MARKER).is_some() {
            let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
            let error = build_perimeter_profile(
                &provider,
                &std::env::temp_dir(),
                Path::new("/opt/homebrew/bin/cursor-agent"),
            )
            .expect_err("relative HOME should fail closed");

            assert!(matches!(
                error,
                AcpClientError::InvalidConfig(message) if message.contains("absolute HOME")
            ));
            return;
        }

        // Keep HOME mutation isolated to a child test process; the workspace
        // forbids unsafe code and Rust 2024 treats process env mutation as unsafe.
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg("launch::tests::perimeter_profile_rejects_relative_home")
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
    fn codex_policy_maps_runtime_modes() {
        assert_eq!(
            codex_policy(RuntimePolicyMode::Allow),
            ("never", "danger-full-access")
        );
        assert_eq!(
            codex_policy(RuntimePolicyMode::RequireApproval),
            ("on-request", "read-only")
        );
        assert_eq!(
            codex_policy(RuntimePolicyMode::Deny),
            ("never", "read-only")
        );
    }

    #[test]
    fn codex_args_enable_network_for_http_mcp() {
        let args = codex_args(RuntimePolicyMode::RequireApproval, true);

        assert!(
            args.iter()
                .any(|arg| arg == "sandbox_workspace_write.network_access=true")
        );
    }

    #[test]
    fn claude_modes_match_runtime_policy_modes() {
        let mapping = claude_mode_mapping();
        assert_eq!(
            translate(RuntimePolicyMode::Allow, &mapping).expect("allow"),
            "bypassPermissions"
        );
        assert_eq!(
            translate(RuntimePolicyMode::RequireApproval, &mapping).expect("approval"),
            "default"
        );
        assert_eq!(
            translate(RuntimePolicyMode::Deny, &mapping).expect("deny"),
            "plan"
        );
    }

    #[test]
    fn copilot_modes_use_acp_protocol_uris() {
        let mapping = copilot_mode_mapping();
        assert_eq!(
            translate(RuntimePolicyMode::Allow, &mapping).expect("allow"),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            translate(RuntimePolicyMode::RequireApproval, &mapping).expect("approval"),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            translate(RuntimePolicyMode::Deny, &mapping).expect("deny"),
            COPILOT_MODE_PLAN
        );
    }

    #[test]
    fn copilot_args_pass_custom_model_only_when_selected() {
        assert_eq!(copilot_args(None), ["--acp"]);

        let model_id = ta_protocol::wire::AgentRuntimeModelId::new("gpt-4.1").expect("model id");

        assert_eq!(
            copilot_args(Some(&model_id)),
            ["--acp", "--model", "gpt-4.1"]
        );
    }
}
