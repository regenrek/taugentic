use ta_protocol::wire::{
    EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy,
    SandboxProfile, TrustState, Workspace, WorkspaceId, WorkspacePath, WorkspaceScope,
};

fn canonical_repo_path() -> WorkspacePath {
    WorkspacePath::canonicalize_existing(
        std::env::current_dir().expect("test process should have a current directory"),
    )
    .expect("current directory should canonicalize")
}

#[test]
fn workspace_path_rejects_relative_paths() {
    let error = WorkspacePath::new("relative/path").expect_err("relative path should fail");
    let json = serde_json::to_value(&error).expect("workspace path error should serialize");

    assert_eq!(error.to_string(), "WorkspacePathNotAbsolute: relative/path");
    assert_eq!(json["code"], "WorkspacePathNotAbsolute");
}

#[test]
fn workspace_path_deserialization_does_not_probe_filesystem() {
    let path = std::env::temp_dir().join("taugentic-wsec-nonexistent-workspace");
    let path = path.to_string_lossy().into_owned();

    let decoded: WorkspacePath =
        serde_json::from_value(serde_json::json!(path)).expect("absolute wire path should decode");

    assert_eq!(decoded.as_str(), path);
}

#[test]
fn workspace_roundtrips_through_json() {
    let root = canonical_repo_path();
    let workspace = Workspace {
        id: WorkspaceId::new("workspace-01").expect("workspace id should be valid"),
        root_realpath: root.clone(),
        display_name: "taugentic".to_string(),
        trust_state: TrustState::UserConfirmed {
            confirmed_at: "2026-05-09T13:00:00Z".to_string(),
        },
        git_repo_root: Some(root),
        created_at: "2026-05-09T13:00:00Z".to_string(),
        last_used_at: "2026-05-09T13:05:00Z".to_string(),
    };

    let json = serde_json::to_value(&workspace).expect("workspace should serialize");
    let decoded: Workspace = serde_json::from_value(json.clone()).expect("workspace roundtrip");

    assert_eq!(decoded, workspace);
    assert_eq!(json["rootRealpath"], workspace.root_realpath.as_str());
    assert_eq!(json["trustState"]["state"], "userConfirmed");
}

#[test]
fn execution_context_roundtrips_with_all_workspace_scope_variants() {
    let root = canonical_repo_path();
    let scopes = [
        WorkspaceScope::Local { root: root.clone() },
        WorkspaceScope::Worktree {
            root: root.clone(),
            worktree: root.clone(),
            branch: "wsec/test".to_string(),
        },
        WorkspaceScope::Readonly { root: root.clone() },
        WorkspaceScope::Remote { root: root.clone() },
        WorkspaceScope::Container { root: root.clone() },
        WorkspaceScope::Ephemeral { root: root.clone() },
    ];

    for scope in scopes {
        let context = ExecutionContext {
            workspace_id: WorkspaceId::new("workspace-01").expect("workspace id should be valid"),
            workspace_root: root.clone(),
            effective_cwd: root.clone(),
            artifact_root: root.clone(),
            workspace_scope: scope,
            sandbox_profile: SandboxProfile {
                read_roots: vec![root.clone()],
                write_roots: vec![root.clone()],
                denied_roots: Vec::new(),
                process_exec: ProcessExecPolicy::Allowlist {
                    binaries: vec!["git".to_string()],
                },
            },
            permission_policy: PermissionPolicy::Unrestricted,
            network_policy: NetworkPolicy::Allowlist {
                domains: vec!["example.com".to_string()],
            },
            env_policy: EnvPolicy::workspace_default(),
        };

        let json = serde_json::to_value(&context).expect("execution context should serialize");
        let decoded: ExecutionContext =
            serde_json::from_value(json).expect("execution context roundtrip");

        assert_eq!(decoded, context);
    }
}

#[test]
fn policy_variants_roundtrip_through_json() {
    for policy in [
        PermissionPolicy::ReadOnly,
        PermissionPolicy::WorkspaceWrite,
        PermissionPolicy::WorkspaceWriteWithApproval,
        PermissionPolicy::RepoWriteWithApproval,
        PermissionPolicy::Unrestricted,
    ] {
        let json = serde_json::to_value(policy).expect("permission policy should serialize");
        let decoded: PermissionPolicy =
            serde_json::from_value(json).expect("permission policy roundtrip");
        assert_eq!(decoded, policy);
    }

    for policy in [
        NetworkPolicy::None,
        NetworkPolicy::Loopback,
        NetworkPolicy::Allowlist {
            domains: vec!["example.com".to_string()],
        },
        NetworkPolicy::Open,
    ] {
        let json = serde_json::to_value(&policy).expect("network policy should serialize");
        let decoded: NetworkPolicy =
            serde_json::from_value(json).expect("network policy roundtrip");
        assert_eq!(decoded, policy);
    }

    for policy in [
        EnvPolicy::Allowlist {
            vars: vec![
                "PATH".to_string(),
                "HOME".to_string(),
                "USER".to_string(),
                "LANG".to_string(),
                "TZ".to_string(),
            ],
        },
        EnvPolicy::All,
    ] {
        let json = serde_json::to_value(&policy).expect("env policy should serialize");
        let decoded: EnvPolicy = serde_json::from_value(json).expect("env policy roundtrip");
        assert_eq!(decoded, policy);
    }

    for policy in [
        ProcessExecPolicy::Denied,
        ProcessExecPolicy::Allowlist {
            binaries: vec!["git".to_string()],
        },
        ProcessExecPolicy::AllowAll,
    ] {
        let json = serde_json::to_value(&policy).expect("process policy should serialize");
        let decoded: ProcessExecPolicy =
            serde_json::from_value(json).expect("process policy roundtrip");
        assert_eq!(decoded, policy);
    }
}

#[test]
fn forward_scope_variants_report_unsupported_dispatch_variant() {
    let root = canonical_repo_path();

    assert_eq!(
        WorkspaceScope::Remote { root: root.clone() }.unsupported_dispatch_variant(),
        Some("remote")
    );
    assert_eq!(
        WorkspaceScope::Container { root: root.clone() }.unsupported_dispatch_variant(),
        Some("container")
    );
    assert_eq!(
        WorkspaceScope::Ephemeral { root }.unsupported_dispatch_variant(),
        Some("ephemeral")
    );
}
