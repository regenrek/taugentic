use std::path::{Path, PathBuf};

use ta_exec::{NetworkPolicy, SandboxProfile};
use ta_protocol::wire::{
    AgentRuntimeModelId, EnvPolicy, ExecutionContext, NetworkPolicy as ContextNetworkPolicy,
    PermissionPolicy, ProcessExecPolicy, RuntimeExtensionState, WorkspaceCapabilityUnsupported,
};

use crate::{
    adapter::{AcpProcessConfig, DEFAULT_CANCEL_GRACE},
    descriptor::{AcpLaunchKind, AcpProviderSpec, validate_provider_id},
    error::AcpClientError,
    mcp::{extensions_include_http_mcp, extensions_to_mcp_servers},
    provider_env::ProviderEnv,
    search_path,
};

#[derive(Debug, Clone, Copy)]
pub struct AcpLaunchInput<'a> {
    pub execution_context: &'a ExecutionContext,
    pub runtime_extensions: &'a [RuntimeExtensionState],
    pub model_id: Option<&'a AgentRuntimeModelId>,
}

#[tracing::instrument(skip(input), fields(provider_id = %provider.provider_id(), work_dir = %input.execution_context.effective_cwd.as_str()))]
pub fn build_config(
    provider: &AcpProviderSpec,
    input: AcpLaunchInput<'_>,
) -> Result<AcpProcessConfig, AcpClientError> {
    validate_mcp_network_policy(
        provider,
        input.execution_context,
        extensions_include_http_mcp(input.runtime_extensions),
    )?;
    let env = ProviderEnv::from_process_env().child_env(provider);
    let command = search_path::resolve(provider.binary_name(), provider.env_override_var())?;
    let sandbox_profile = build_perimeter_profile(provider, input.execution_context, &command)?;
    match provider.launch_kind() {
        AcpLaunchKind::Codex => Ok(codex_config(provider, command, env, sandbox_profile, input)),
        AcpLaunchKind::Claude => Ok(claude_config(
            provider,
            command,
            env,
            sandbox_profile,
            input,
        )),
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
        AcpLaunchKind::Copilot => Ok(copilot_config(
            provider,
            command,
            env,
            sandbox_profile,
            input,
        )),
    }
}

pub fn build_perimeter_profile(
    provider: &AcpProviderSpec,
    execution_context: &ExecutionContext,
    command: &Path,
) -> Result<SandboxProfile, AcpClientError> {
    validate_execution_context(provider, execution_context)?;
    let workspace_cwd = execution_context.effective_cwd.as_path();
    require_absolute_path("workspace cwd", workspace_cwd)?;
    require_absolute_path("ACP command", command)?;

    workspace_cwd.canonicalize().map_err(|error| {
        AcpClientError::InvalidConfig(format!(
            "ACP perimeter sandbox workspace cwd must exist: {}: {error}",
            workspace_cwd.display()
        ))
    })?;
    let provider_cache = provider_cache_dir(provider)?;
    // The ACP process needs provider-control network access. Tool-network policy
    // is mapped into the provider and rejected when the vendor cannot enforce it.
    let mut profile = SandboxProfile::new()
        .network(NetworkPolicy::Open)
        .child_inherits_tty(false)
        .read_path(&provider_cache)
        .write_path(provider_cache);

    for root in &execution_context.sandbox_profile.read_roots {
        profile = profile.read_path(root.as_path());
    }
    for root in &execution_context.sandbox_profile.write_roots {
        profile = profile.write_path(root.as_path());
    }

    for path in system_read_paths() {
        profile = profile.read_path(path);
    }
    for path in command_read_paths(command) {
        profile = profile.read_path(path);
    }
    profile = match &execution_context.env_policy {
        EnvPolicy::Allowlist { vars } => {
            vars.iter().fold(profile, |profile, name| profile.env(name))
        }
        EnvPolicy::All => profile.inherit_all_env(),
    };

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

fn validate_execution_context(
    provider: &AcpProviderSpec,
    context: &ExecutionContext,
) -> Result<(), AcpClientError> {
    if let Some(variant) = context.workspace_scope.unsupported_dispatch_variant() {
        return Err(unsupported_policy(
            provider,
            "executionScope",
            variant,
            "ACP execution requires a local, worktree, or readonly workspace",
        ));
    }
    if !context.sandbox_profile.denied_roots.is_empty() {
        return Err(unsupported_policy(
            provider,
            "deniedRoots",
            "nested deny roots",
            "the ACP process sandbox cannot enforce deny roots nested inside granted roots",
        ));
    }
    match &context.sandbox_profile.process_exec {
        ProcessExecPolicy::AllowAll => {}
        ProcessExecPolicy::Denied => {
            return Err(unsupported_policy(
                provider,
                "processExec",
                "denied",
                "the ACP provider process cannot run while descendant execution is fully denied",
            ));
        }
        ProcessExecPolicy::Allowlist { .. } => {
            return Err(unsupported_policy(
                provider,
                "processExec",
                "binary allowlist",
                "ACP providers do not expose enforceable descendant binary allowlists",
            ));
        }
    }
    match (provider.launch_kind(), &context.network_policy) {
        (AcpLaunchKind::Codex, ContextNetworkPolicy::None)
            if matches!(
                context.permission_policy,
                PermissionPolicy::WorkspaceWriteWithApproval
                    | PermissionPolicy::RepoWriteWithApproval
                    | PermissionPolicy::Unrestricted
            ) =>
        {
            Err(unsupported_policy(
                provider,
                "network",
                "none with approval escalation or unrestricted filesystem access",
                "Codex escalations cannot separate approved filesystem access from tool network access",
            ))
        }
        (AcpLaunchKind::Codex, ContextNetworkPolicy::None | ContextNetworkPolicy::Open)
        | (
            AcpLaunchKind::Claude
            | AcpLaunchKind::Cursor
            | AcpLaunchKind::OpenCode
            | AcpLaunchKind::Copilot,
            ContextNetworkPolicy::Open,
        ) => Ok(()),
        (AcpLaunchKind::Codex, ContextNetworkPolicy::Loopback) => Err(unsupported_policy(
            provider,
            "network",
            "loopback",
            "Codex ACP does not expose destination-aware loopback enforcement",
        )),
        (AcpLaunchKind::Codex, ContextNetworkPolicy::Allowlist { .. }) => Err(unsupported_policy(
            provider,
            "network",
            "destination allowlist",
            "Codex ACP does not expose destination-aware domain enforcement",
        )),
        (_, ContextNetworkPolicy::None) => Err(unsupported_policy(
            provider,
            "network",
            "none",
            "this ACP provider shares one process for model transport and tool network access",
        )),
        (_, ContextNetworkPolicy::Loopback) => Err(unsupported_policy(
            provider,
            "network",
            "loopback",
            "this ACP provider does not expose destination-aware tool network enforcement",
        )),
        (_, ContextNetworkPolicy::Allowlist { .. }) => Err(unsupported_policy(
            provider,
            "network",
            "destination allowlist",
            "this ACP provider does not expose destination-aware tool network enforcement",
        )),
    }
}

fn validate_mcp_network_policy(
    provider: &AcpProviderSpec,
    context: &ExecutionContext,
    has_http_mcp: bool,
) -> Result<(), AcpClientError> {
    if has_http_mcp && !matches!(context.network_policy, ContextNetworkPolicy::Open) {
        return Err(unsupported_policy(
            provider,
            "httpMcpNetwork",
            "HTTP MCP with closed or destination-scoped network access",
            "the ACP provider owns the HTTP MCP connection outside the tool sandbox",
        ));
    }
    Ok(())
}

fn unsupported_policy(
    provider: &AcpProviderSpec,
    capability: &str,
    requested: &str,
    reason: &str,
) -> AcpClientError {
    AcpClientError::WorkspaceCapabilityUnsupported(WorkspaceCapabilityUnsupported {
        variant: context_variant(capability, requested),
        vendor: Some(provider.provider_id().to_string()),
        capability: capability.to_string(),
        requested: requested.to_string(),
        reason: reason.to_string(),
    })
}

fn context_variant(capability: &str, requested: &str) -> Option<String> {
    (capability == "executionScope").then(|| requested.to_string())
}

fn codex_config(
    provider: &AcpProviderSpec,
    command: PathBuf,
    env: Vec<(String, String)>,
    sandbox_profile: SandboxProfile,
    input: AcpLaunchInput<'_>,
) -> AcpProcessConfig {
    let args = codex_args(input.execution_context);
    AcpProcessConfig {
        flavor_id: provider.provider_id().to_string(),
        command,
        sandbox_profile,
        args,
        env,
        env_remove: Vec::new(),
        work_dir: input
            .execution_context
            .effective_cwd
            .as_path()
            .to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn claude_config(
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
        args: Vec::new(),
        env,
        env_remove: vec!["CLAUDECODE".to_string()],
        work_dir: input
            .execution_context
            .effective_cwd
            .as_path()
            .to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: Some(
            claude_session_mode(input.execution_context.permission_policy).to_string(),
        ),
        session_model_id: acp_session_model_id(input.model_id),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
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
        work_dir: input
            .execution_context
            .effective_cwd
            .as_path()
            .to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
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
        work_dir: input
            .execution_context
            .effective_cwd
            .as_path()
            .to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: None,
        session_model_id: acp_session_model_id(input.model_id),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn copilot_config(
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
        args: copilot_args(input.model_id),
        env,
        env_remove: Vec::new(),
        work_dir: input
            .execution_context
            .effective_cwd
            .as_path()
            .to_path_buf(),
        mcp_servers: extensions_to_mcp_servers(input.runtime_extensions),
        session_mode_id: Some(
            copilot_session_mode(input.execution_context.permission_policy).to_string(),
        ),
        session_model_id: acp_session_model_id(input.model_id),
        cancel_grace: DEFAULT_CANCEL_GRACE,
    }
}

fn acp_session_model_id(model_id: Option<&AgentRuntimeModelId>) -> Option<String> {
    model_id.map(|model_id| model_id.as_str().to_string())
}

pub(crate) fn codex_args(context: &ExecutionContext) -> Vec<String> {
    let (approval_policy, sandbox_mode) = codex_policy(context.permission_policy);
    let mut args = vec![
        "-c".to_string(),
        format!("approval_policy={approval_policy}"),
        "-c".to_string(),
        format!("sandbox_mode={sandbox_mode}"),
        "-c".to_string(),
        format!(
            "sandbox_workspace_write.network_access={}",
            matches!(context.network_policy, ContextNetworkPolicy::Open)
        ),
    ];
    if !context.sandbox_profile.write_roots.is_empty() {
        let writable_roots = context
            .sandbox_profile
            .write_roots
            .iter()
            .map(|path| {
                serde_json::to_string(path.as_str())
                    .expect("workspace paths must serialize as TOML-compatible strings")
            })
            .collect::<Vec<_>>()
            .join(",");
        args.extend([
            "-c".to_string(),
            format!("sandbox_workspace_write.writable_roots=[{writable_roots}]"),
        ]);
    }
    args
}

fn codex_policy(permission_policy: PermissionPolicy) -> (&'static str, &'static str) {
    match permission_policy {
        PermissionPolicy::ReadOnly => ("never", "read-only"),
        PermissionPolicy::WorkspaceWrite => ("never", "workspace-write"),
        PermissionPolicy::WorkspaceWriteWithApproval | PermissionPolicy::RepoWriteWithApproval => {
            ("on-request", "read-only")
        }
        PermissionPolicy::Unrestricted => ("never", "danger-full-access"),
    }
}

fn claude_session_mode(permission_policy: PermissionPolicy) -> &'static str {
    match permission_policy {
        PermissionPolicy::ReadOnly => "plan",
        PermissionPolicy::WorkspaceWriteWithApproval | PermissionPolicy::RepoWriteWithApproval => {
            "default"
        }
        PermissionPolicy::WorkspaceWrite | PermissionPolicy::Unrestricted => "bypassPermissions",
    }
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

fn copilot_session_mode(permission_policy: PermissionPolicy) -> &'static str {
    match permission_policy {
        PermissionPolicy::ReadOnly => COPILOT_MODE_PLAN,
        PermissionPolicy::WorkspaceWrite
        | PermissionPolicy::WorkspaceWriteWithApproval
        | PermissionPolicy::RepoWriteWithApproval
        | PermissionPolicy::Unrestricted => COPILOT_MODE_AGENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{
        SandboxProfile as ContextSandboxProfile, WorkspaceId, WorkspacePath, WorkspaceScope,
    };

    fn test_context(workspace: &Path) -> ExecutionContext {
        let root = WorkspacePath::canonicalize_existing(workspace).expect("workspace path");
        ExecutionContext {
            workspace_id: WorkspaceId::new("workspace-acp-launch").expect("workspace id"),
            workspace_root: root.clone(),
            effective_cwd: root.clone(),
            artifact_root: root.clone(),
            workspace_scope: WorkspaceScope::Local { root: root.clone() },
            sandbox_profile: ContextSandboxProfile {
                read_roots: vec![root.clone()],
                write_roots: vec![root],
                denied_roots: Vec::new(),
                process_exec: ProcessExecPolicy::AllowAll,
            },
            permission_policy: PermissionPolicy::WorkspaceWrite,
            network_policy: ContextNetworkPolicy::Open,
            env_policy: EnvPolicy::workspace_default(),
        }
    }

    #[test]
    fn perimeter_profile_allows_expected_acp_boundary() {
        use crate::provider_env::{ANTHROPIC_API_KEY_ENV, GEMINI_API_KEY_ENV, OPENAI_API_KEY_ENV};

        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
        let workspace = std::env::temp_dir().join("taugentic-acp-workspace-boundary");
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let command = Path::new("/opt/homebrew/bin/cursor-agent");

        let context = test_context(&workspace);
        let profile = build_perimeter_profile(&provider, &context, command).expect("profile");

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
        let error = build_perimeter_profile(
            &provider,
            &test_context(&std::env::temp_dir()),
            Path::new("codex-acp"),
        )
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
                &test_context(&std::env::temp_dir()),
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
            codex_policy(PermissionPolicy::WorkspaceWrite),
            ("never", "workspace-write")
        );
        assert_eq!(
            codex_policy(PermissionPolicy::WorkspaceWriteWithApproval),
            ("on-request", "read-only")
        );
        assert_eq!(
            codex_policy(PermissionPolicy::ReadOnly),
            ("never", "read-only")
        );
    }

    #[test]
    fn codex_args_map_network_and_writable_roots_from_context() {
        let mut context = test_context(&std::env::temp_dir());
        context.permission_policy = PermissionPolicy::WorkspaceWriteWithApproval;
        let args = codex_args(&context);

        assert!(
            args.iter()
                .any(|arg| arg == "sandbox_workspace_write.network_access=true")
        );
        assert!(args.iter().any(|arg| {
            arg.starts_with("sandbox_workspace_write.writable_roots=[")
                && arg.contains(context.effective_cwd.as_str())
        }));

        context.network_policy = ContextNetworkPolicy::None;
        let args = codex_args(&context);
        assert!(
            args.iter()
                .any(|arg| arg == "sandbox_workspace_write.network_access=false")
        );
    }

    #[test]
    fn codex_rejects_closed_network_when_approval_can_escalate() {
        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Codex);
        let mut context = test_context(&std::env::temp_dir());
        context.permission_policy = PermissionPolicy::WorkspaceWriteWithApproval;
        context.network_policy = ContextNetworkPolicy::None;

        let error = validate_execution_context(&provider, &context).expect_err("unsupported");

        assert!(matches!(
            error,
            AcpClientError::WorkspaceCapabilityUnsupported(detail)
                if detail.capability == "network"
                    && detail.requested.contains("approval escalation")
        ));
    }

    #[test]
    fn http_mcp_rejects_closed_network_before_provider_launch() {
        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Codex);
        let mut context = test_context(&std::env::temp_dir());
        context.network_policy = ContextNetworkPolicy::None;

        let error = validate_mcp_network_policy(&provider, &context, true)
            .expect_err("HTTP MCP needs open network");

        assert!(matches!(
            error,
            AcpClientError::WorkspaceCapabilityUnsupported(detail)
                if detail.capability == "httpMcpNetwork"
        ));
    }

    #[test]
    fn non_codex_provider_rejects_closed_tool_network_before_spawn() {
        let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
        let mut context = test_context(&std::env::temp_dir());
        context.network_policy = ContextNetworkPolicy::None;

        let error = build_perimeter_profile(
            &provider,
            &context,
            Path::new("/opt/homebrew/bin/cursor-agent"),
        )
        .expect_err("unsupported network policy");

        assert!(matches!(
            error,
            AcpClientError::WorkspaceCapabilityUnsupported(detail)
                if detail.vendor.as_deref() == Some("cursor") && detail.capability == "network"
        ));
    }

    #[test]
    fn claude_modes_map_every_permission_policy() {
        assert_eq!(
            claude_session_mode(PermissionPolicy::WorkspaceWrite),
            "bypassPermissions"
        );
        assert_eq!(
            claude_session_mode(PermissionPolicy::Unrestricted),
            "bypassPermissions"
        );
        assert_eq!(
            claude_session_mode(PermissionPolicy::WorkspaceWriteWithApproval),
            "default"
        );
        assert_eq!(
            claude_session_mode(PermissionPolicy::RepoWriteWithApproval),
            "default"
        );
        assert_eq!(claude_session_mode(PermissionPolicy::ReadOnly), "plan");
    }

    #[test]
    fn copilot_modes_map_every_permission_policy_to_protocol_uris() {
        assert_eq!(
            copilot_session_mode(PermissionPolicy::WorkspaceWrite),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            copilot_session_mode(PermissionPolicy::WorkspaceWriteWithApproval),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            copilot_session_mode(PermissionPolicy::RepoWriteWithApproval),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            copilot_session_mode(PermissionPolicy::Unrestricted),
            COPILOT_MODE_AGENT
        );
        assert_eq!(
            copilot_session_mode(PermissionPolicy::ReadOnly),
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
