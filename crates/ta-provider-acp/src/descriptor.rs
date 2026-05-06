#![warn(missing_docs)]
//! Canonical ACP provider descriptors.

use std::{collections::BTreeMap, sync::Arc};

use ta_protocol::provider_id;
use thiserror::Error;

/// Builtin ACP launch families supported by the provider adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AcpLaunchKind {
    /// Zed's Codex ACP server.
    Codex,
    /// Zed's Claude Code ACP server.
    Claude,
    /// Cursor Agent's ACP mode.
    Cursor,
    /// OpenCode's ACP mode.
    OpenCode,
    /// GitHub Copilot CLI's ACP mode.
    Copilot,
}

/// Errors raised while composing ACP provider descriptors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AcpDescriptorError {
    /// More than one provider descriptor used the same provider id.
    #[error("duplicate ACP provider descriptor: {0}")]
    DuplicateProviderId(String),
    /// A provider id cannot be used as a single cache-path segment.
    #[error("invalid ACP provider id {provider_id:?}: {reason}")]
    InvalidProviderId {
        /// Invalid provider id supplied by the descriptor.
        provider_id: String,
        /// Stable validation reason.
        reason: &'static str,
    },
}

/// Validates the canonical provider id/cache segment contract.
pub fn validate_provider_id(provider_id: &str) -> Result<(), AcpDescriptorError> {
    provider_id::validate_provider_id(provider_id).map_err(|error| {
        AcpDescriptorError::InvalidProviderId {
            provider_id: error.provider_id().to_string(),
            reason: error.reason(),
        }
    })
}

/// Immutable description of an ACP provider and its launch contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcpProviderDescriptor {
    provider_id: Arc<str>,
    display_name: Arc<str>,
    runtime_profile_label: Arc<str>,
    selector_group_label: Arc<str>,
    launch_kind: AcpLaunchKind,
    binary_name: Arc<str>,
    env_override_var: Arc<str>,
    install_command: Option<Arc<str>>,
    upgrade_command: Option<Arc<str>>,
    auth_command: Option<Arc<str>>,
    selector_sort_order: u16,
    doc_url: Arc<str>,
    setup_steps: Arc<[Arc<str>]>,
    icon_key: Arc<str>,
    auth_description: Option<Arc<str>>,
}

impl AcpProviderDescriptor {
    /// Builds a descriptor for an ACP provider.
    pub fn new(
        provider_id: impl Into<Arc<str>>,
        display_name: impl Into<Arc<str>>,
        runtime_profile_label: impl Into<Arc<str>>,
        launch_kind: AcpLaunchKind,
        binary_name: impl Into<Arc<str>>,
        env_override_var: impl Into<Arc<str>>,
    ) -> Result<Self, AcpDescriptorError> {
        let provider_id = provider_id.into();
        validate_provider_id(&provider_id)?;
        let display_name = display_name.into();
        let runtime_profile_label = runtime_profile_label.into();
        Ok(Self {
            provider_id,
            display_name: display_name.clone(),
            runtime_profile_label,
            selector_group_label: display_name,
            launch_kind,
            binary_name: binary_name.into(),
            env_override_var: env_override_var.into(),
            install_command: None,
            upgrade_command: None,
            auth_command: None,
            selector_sort_order: u16::MAX,
            doc_url: Arc::from("https://agentclientprotocol.com/"),
            setup_steps: Arc::from([]),
            icon_key: Arc::from("acp"),
            auth_description: None,
        })
    }

    /// Returns this descriptor with UI/support metadata attached.
    #[must_use]
    pub fn with_metadata(mut self, metadata: AcpProviderMetadata) -> Self {
        self.selector_group_label = metadata.selector_group_label;
        self.install_command = metadata.install_command;
        self.upgrade_command = metadata.upgrade_command;
        self.auth_command = metadata.auth_command;
        self.selector_sort_order = metadata.selector_sort_order;
        self.doc_url = metadata.doc_url;
        self.setup_steps = metadata.setup_steps;
        self.icon_key = metadata.icon_key;
        self.auth_description = metadata.auth_description;
        self
    }

    /// Stable provider id used by runtime profiles.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Human display name for the provider.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Runtime profile label prefix for this provider.
    #[must_use]
    pub fn runtime_profile_label(&self) -> &str {
        &self.runtime_profile_label
    }

    /// UI selector group label for this provider.
    #[must_use]
    pub fn selector_group_label(&self) -> &str {
        &self.selector_group_label
    }

    /// Launch family used by the ACP adapter.
    #[must_use]
    pub const fn launch_kind(&self) -> AcpLaunchKind {
        self.launch_kind
    }

    /// Executable name to resolve from PATH.
    #[must_use]
    pub fn binary_name(&self) -> &str {
        &self.binary_name
    }

    /// Environment variable used to override the executable path.
    #[must_use]
    pub fn env_override_var(&self) -> &str {
        &self.env_override_var
    }

    /// Optional install command displayed to users.
    #[must_use]
    pub fn install_command(&self) -> Option<&str> {
        self.install_command.as_deref()
    }

    /// Optional upgrade command displayed to users.
    #[must_use]
    pub fn upgrade_command(&self) -> Option<&str> {
        self.upgrade_command.as_deref()
    }

    /// Optional authentication command displayed to users.
    #[must_use]
    pub fn auth_command(&self) -> Option<&str> {
        self.auth_command.as_deref()
    }

    /// Sort order for runtime profile selector groups.
    #[must_use]
    pub const fn selector_sort_order(&self) -> u16 {
        self.selector_sort_order
    }

    /// Documentation URL for setup guidance.
    #[must_use]
    pub fn doc_url(&self) -> &str {
        &self.doc_url
    }

    /// Setup steps displayed to users.
    #[must_use]
    pub fn setup_steps(&self) -> &[Arc<str>] {
        &self.setup_steps
    }

    /// Icon key used by clients.
    #[must_use]
    pub fn icon_key(&self) -> &str {
        &self.icon_key
    }

    /// Optional authentication guidance.
    #[must_use]
    pub fn auth_description(&self) -> Option<&str> {
        self.auth_description.as_deref()
    }
}

/// Optional UI/support metadata for an ACP provider descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProviderMetadata {
    selector_group_label: Arc<str>,
    install_command: Option<Arc<str>>,
    upgrade_command: Option<Arc<str>>,
    auth_command: Option<Arc<str>>,
    selector_sort_order: u16,
    doc_url: Arc<str>,
    setup_steps: Arc<[Arc<str>]>,
    icon_key: Arc<str>,
    auth_description: Option<Arc<str>>,
}

impl AcpProviderMetadata {
    /// Builds metadata for an ACP provider descriptor.
    #[must_use]
    pub fn new(
        selector_group_label: impl Into<Arc<str>>,
        selector_sort_order: u16,
        doc_url: impl Into<Arc<str>>,
        setup_steps: impl IntoIterator<Item = impl Into<Arc<str>>>,
        icon_key: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            selector_group_label: selector_group_label.into(),
            install_command: None,
            upgrade_command: None,
            auth_command: None,
            selector_sort_order,
            doc_url: doc_url.into(),
            setup_steps: setup_steps
                .into_iter()
                .map(Into::into)
                .collect::<Vec<Arc<str>>>()
                .into(),
            icon_key: icon_key.into(),
            auth_description: None,
        }
    }

    /// Returns this metadata with install, upgrade, and auth commands attached.
    #[must_use]
    pub fn with_commands(
        mut self,
        install_command: Option<impl Into<Arc<str>>>,
        upgrade_command: Option<impl Into<Arc<str>>>,
        auth_command: Option<impl Into<Arc<str>>>,
    ) -> Self {
        self.install_command = install_command.map(Into::into);
        self.upgrade_command = upgrade_command.map(Into::into);
        self.auth_command = auth_command.map(Into::into);
        self
    }

    /// Returns this metadata with auth copy attached.
    #[must_use]
    pub fn with_auth_description(mut self, auth_description: Option<impl Into<Arc<str>>>) -> Self {
        self.auth_description = auth_description.map(Into::into);
        self
    }
}

/// Stable shareable handle to an ACP provider descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcpProviderSpec(Arc<AcpProviderDescriptor>);

impl AcpProviderSpec {
    /// Builds a provider spec from an owned descriptor.
    #[must_use]
    pub fn new(descriptor: AcpProviderDescriptor) -> Self {
        Self(Arc::new(descriptor))
    }

    /// Builds a provider spec from an existing descriptor pointer.
    #[must_use]
    pub fn from_arc(descriptor: Arc<AcpProviderDescriptor>) -> Self {
        Self(descriptor)
    }

    /// Builds the provider spec for a builtin ACP launch family.
    #[must_use]
    pub fn from_builtin(launch_kind: AcpLaunchKind) -> Self {
        Self::new(builtin_descriptor(launch_kind))
    }

    /// Returns the underlying descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &AcpProviderDescriptor {
        &self.0
    }

    /// Returns a cloned descriptor pointer.
    #[must_use]
    pub fn descriptor_arc(&self) -> Arc<AcpProviderDescriptor> {
        self.0.clone()
    }

    /// Stable provider id used by runtime profiles.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        self.0.provider_id()
    }

    /// Human display name for the provider.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.0.display_name()
    }

    /// Runtime profile label prefix for this provider.
    #[must_use]
    pub fn runtime_profile_label(&self) -> &str {
        self.0.runtime_profile_label()
    }

    /// Launch family used by the ACP adapter.
    #[must_use]
    pub fn launch_kind(&self) -> AcpLaunchKind {
        self.0.launch_kind()
    }

    /// Executable name to resolve from PATH.
    #[must_use]
    pub fn binary_name(&self) -> &str {
        self.0.binary_name()
    }

    /// Environment variable used to override the executable path.
    #[must_use]
    pub fn env_override_var(&self) -> &str {
        self.0.env_override_var()
    }
}

/// Registry of ACP provider specs keyed by provider id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProviderRegistry {
    provider_order: Vec<AcpProviderSpec>,
    providers_by_id: BTreeMap<Arc<str>, AcpProviderSpec>,
}

impl AcpProviderRegistry {
    /// Builds a registry from builtin descriptors plus custom descriptors.
    pub fn new(
        custom_descriptors: impl IntoIterator<Item = AcpProviderSpec>,
    ) -> Result<Self, AcpDescriptorError> {
        Self::from_specs(
            builtin_provider_descriptors()
                .into_iter()
                .chain(custom_descriptors),
        )
    }

    /// Builds a registry from the supplied descriptors only.
    pub fn from_specs(
        descriptors: impl IntoIterator<Item = AcpProviderSpec>,
    ) -> Result<Self, AcpDescriptorError> {
        let mut provider_order = Vec::new();
        let mut providers_by_id = BTreeMap::new();
        for provider in descriptors {
            let provider_id = Arc::<str>::from(provider.provider_id());
            if providers_by_id
                .insert(provider_id.clone(), provider.clone())
                .is_some()
            {
                return Err(AcpDescriptorError::DuplicateProviderId(
                    provider_id.to_string(),
                ));
            }
            provider_order.push(provider);
        }
        Ok(Self {
            provider_order,
            providers_by_id,
        })
    }

    /// Returns all registered providers in descriptor insertion order.
    #[must_use]
    pub fn providers(&self) -> Vec<AcpProviderSpec> {
        self.provider_order.clone()
    }

    /// Returns a provider by id.
    #[must_use]
    pub fn provider(&self, provider_id: &str) -> Option<&AcpProviderSpec> {
        self.providers_by_id.get(provider_id)
    }
}

/// Returns the canonical builtin ACP provider descriptors.
#[must_use]
pub fn builtin_provider_descriptors() -> Vec<AcpProviderSpec> {
    [
        AcpLaunchKind::Codex,
        AcpLaunchKind::Claude,
        AcpLaunchKind::Cursor,
        AcpLaunchKind::OpenCode,
        AcpLaunchKind::Copilot,
    ]
    .into_iter()
    .map(AcpProviderSpec::from_builtin)
    .collect()
}

fn builtin_descriptor(launch_kind: AcpLaunchKind) -> AcpProviderDescriptor {
    match launch_kind {
        AcpLaunchKind::Codex => AcpProviderDescriptor::new(
            "codex-acp",
            "Codex CLI",
            "Codex ACP",
            launch_kind,
            "codex-acp",
            "TAUGENTIC_CODEX_ACP_BIN",
        )
        .expect("builtin provider id is valid")
        .with_metadata(
            AcpProviderMetadata::new(
                "Codex ACP",
                0,
                "https://github.com/zed-industries/codex-acp",
                [
                    "Install `@zed-industries/codex-acp` globally with npm",
                    "Run `codex` once to authenticate with OpenAI",
                ],
                "codex",
            )
            .with_commands(
                Some("npm install -g @zed-industries/codex-acp"),
                Some("npm update -g @zed-industries/codex-acp"),
                Some("codex"),
            )
            .with_auth_description(Some("Authenticate with Codex CLI")),
        ),
        AcpLaunchKind::Claude => AcpProviderDescriptor::new(
            "claude-acp",
            "Claude Code",
            "Claude ACP",
            launch_kind,
            "claude-agent-acp",
            "TAUGENTIC_CLAUDE_ACP_BIN",
        )
        .expect("builtin provider id is valid")
        .with_metadata(
            AcpProviderMetadata::new(
                "Claude ACP",
                1,
                "https://github.com/zed-industries/claude-agent-acp",
                [
                    "Install `@zed-industries/claude-agent-acp` globally with npm",
                    "Run `claude` once to verify Claude Code authentication",
                ],
                "claude",
            )
            .with_commands(
                Some("npm install -g @zed-industries/claude-agent-acp"),
                Some("npm update -g @zed-industries/claude-agent-acp"),
                Some("claude"),
            )
            .with_auth_description(Some("Authenticate with Claude Code")),
        ),
        AcpLaunchKind::Cursor => AcpProviderDescriptor::new(
            "cursor",
            "Cursor",
            "Cursor ACP",
            launch_kind,
            "cursor-agent",
            "TAUGENTIC_CURSOR_ACP_BIN",
        )
        .expect("builtin provider id is valid")
        .with_metadata(
            AcpProviderMetadata::new(
                "Cursor ACP",
                2,
                "https://docs.cursor.com/en/cli/overview",
                [
                    "Install Cursor and ensure `cursor-agent` is on PATH",
                    "Run `cursor-agent` once to verify Cursor authentication",
                ],
                "cursor",
            )
            .with_commands(None::<Arc<str>>, None::<Arc<str>>, None::<Arc<str>>),
        ),
        AcpLaunchKind::OpenCode => AcpProviderDescriptor::new(
            "opencode",
            "OpenCode",
            "OpenCode ACP",
            launch_kind,
            "opencode",
            "TAUGENTIC_OPENCODE_ACP_BIN",
        )
        .expect("builtin provider id is valid")
        .with_metadata(
            AcpProviderMetadata::new(
                "OpenCode ACP",
                3,
                "https://opencode.ai/docs/acp",
                [
                    "Install `opencode-ai` globally with npm",
                    "Run `opencode auth login` to authenticate OpenCode providers",
                ],
                "opencode",
            )
            .with_commands(
                Some("npm install -g opencode-ai"),
                Some("opencode upgrade"),
                Some("opencode auth login"),
            )
            .with_auth_description(Some("Authenticate OpenCode providers")),
        ),
        AcpLaunchKind::Copilot => AcpProviderDescriptor::new(
            "copilot-acp",
            "GitHub Copilot CLI",
            "Copilot ACP",
            launch_kind,
            "copilot",
            "TAUGENTIC_COPILOT_ACP_BIN",
        )
        .expect("builtin provider id is valid")
        .with_metadata(
            AcpProviderMetadata::new(
                "Copilot ACP",
                4,
                "https://github.com/github/copilot-cli",
                [
                    "Install `@github/copilot` globally with npm",
                    "Run `copilot login` to authenticate with GitHub Copilot",
                ],
                "copilot",
            )
            .with_commands(
                Some("npm install -g @github/copilot"),
                Some("npm update -g @github/copilot"),
                Some("copilot login"),
            )
            .with_auth_description(Some("Authenticate with GitHub Copilot")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_descriptors_are_canonical() {
        let providers = builtin_provider_descriptors();
        let ids = providers
            .iter()
            .map(|provider| provider.provider_id())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            [
                "codex-acp",
                "claude-acp",
                "cursor",
                "opencode",
                "copilot-acp"
            ]
        );
        for provider in providers {
            assert!(!provider.display_name().is_empty());
            assert!(!provider.runtime_profile_label().is_empty());
            assert!(!provider.binary_name().is_empty());
            assert!(!provider.env_override_var().is_empty());
            assert!(!provider.descriptor().selector_group_label().is_empty());
            let _ = provider.descriptor().install_command();
            let _ = provider.descriptor().upgrade_command();
            let _ = provider.descriptor().auth_command();
            let _ = provider.descriptor().selector_sort_order();
            assert!(!provider.descriptor().doc_url().is_empty());
            assert!(!provider.descriptor().setup_steps().is_empty());
            assert!(!provider.descriptor().icon_key().is_empty());
            let _ = provider.descriptor().auth_description();
        }
    }

    #[test]
    fn registry_accepts_custom_descriptor() {
        let custom = AcpProviderSpec::new(
            AcpProviderDescriptor::new(
                "custom-acp",
                "Custom ACP",
                "Custom ACP",
                AcpLaunchKind::Cursor,
                "custom-acp",
                "CUSTOM_ACP_BIN",
            )
            .expect("custom provider id"),
        );

        let registry = AcpProviderRegistry::new([custom.clone()]).expect("registry");

        assert!(registry.provider("codex-acp").is_some());
        assert_eq!(registry.provider("custom-acp"), Some(&custom));
    }

    #[test]
    fn registry_rejects_duplicate_provider_ids() {
        let duplicate = AcpProviderSpec::new(
            AcpProviderDescriptor::new(
                "codex-acp",
                "Duplicate Codex",
                "Duplicate Codex",
                AcpLaunchKind::Codex,
                "codex-acp",
                "TAUGENTIC_CODEX_ACP_BIN",
            )
            .expect("duplicate descriptor"),
        );

        let error = AcpProviderRegistry::new([duplicate]).expect_err("duplicate should reject");

        assert_eq!(
            error,
            AcpDescriptorError::DuplicateProviderId("codex-acp".to_string())
        );
    }

    #[test]
    fn provider_ids_are_safe_cache_segments() {
        for valid in provider_id::VALID_PROVIDER_ID_TEST_CASES {
            validate_provider_id(valid).expect("valid provider id");
        }
        for invalid in provider_id::INVALID_PROVIDER_ID_TEST_CASES {
            assert!(matches!(
                validate_provider_id(invalid),
                Err(AcpDescriptorError::InvalidProviderId { .. })
            ));
        }
    }

    #[test]
    fn descriptor_rejects_unsafe_provider_id() {
        let error = AcpProviderDescriptor::new(
            "../escape",
            "Escape ACP",
            "Escape ACP",
            AcpLaunchKind::Cursor,
            "escape-acp",
            "ESCAPE_ACP_BIN",
        )
        .expect_err("unsafe provider id should fail closed");

        assert!(matches!(
            error,
            AcpDescriptorError::InvalidProviderId { provider_id, .. }
                if provider_id == "../escape"
        ));
    }
}
