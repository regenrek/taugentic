use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::active_execution::{ActiveExecutionOwner, AttachHandleDisposition};
use super::*;
use crate::{RunId, SessionId};
use ta_host_platform::{HostCapabilities, HostOs, HostPlatform, LocalIpcKind, OsVersion};
use ta_protocol::wire::{AgentRuntimeModelId, AuthProfileId, CapsuleRecipe, OutputContractKind};
use ta_provider_acp::descriptor::{AcpLaunchKind, AcpProviderSpec};
use taugentic_agent::{AgentExecutionHarness, ExecutionError, ExecutionHandle};

#[test]
fn runtime_service_keeps_runtime_host_as_the_capability_source() {
    let host_platform = fixture_host_platform();
    let runtime = RuntimeService::from_host_platform(host_platform.clone());

    assert_eq!(runtime.host_platform, host_platform);
    assert_eq!(
        runtime.capabilities().clone(),
        LaneCapabilities::from_host_platform(&runtime.host_platform)
    );
}

#[test]
fn active_execution_owner_tracks_live_running_runs() {
    let runtime = RuntimeService::from_host_platform(fixture_host_platform());
    let run_id = RunId::new("run-1").expect("run id");
    let session_id = SessionId::new("session-1").expect("session id");
    let execution = runtime.run_execution_runtime();
    execution.claim_live_run(run_id.clone(), session_id.clone());
    let live_execution = execution
        .live_execution_for(&run_id)
        .expect("run should have active execution");
    assert_eq!(live_execution.session_id, session_id.clone());
    assert!(execution.is_live_run_running(&run_id, &session_id));
    assert!(execution.release_live_run(&run_id));
    assert!(execution.live_execution_for(&run_id).is_none());
}

#[derive(Clone)]
struct CountingHandle {
    cancel_count: Arc<AtomicUsize>,
}

impl ExecutionHandle for CountingHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.cancel_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn active_execution_owner_cancels_attached_handle_on_drop() {
    let cancel_count = Arc::new(AtomicUsize::new(0));
    {
        let owner = ActiveExecutionOwner::new();
        let run_id = RunId::new("run-drop").expect("run id");
        let session_id = SessionId::new("session-drop").expect("session id");
        owner.claim_run(run_id.clone(), session_id);
        owner
            .attach_handle(
                &run_id,
                Arc::new(CountingHandle {
                    cancel_count: cancel_count.clone(),
                }),
            )
            .expect("attach handle");
    }

    assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
}

#[test]
fn active_execution_owner_rejects_attach_without_claimed_slot() {
    let owner = ActiveExecutionOwner::new();
    let error = owner
        .attach_handle(
            &RunId::new("run-missing").expect("run id"),
            Arc::new(CountingHandle {
                cancel_count: Arc::new(AtomicUsize::new(0)),
            }),
        )
        .expect_err("missing slot must fail");

    assert!(error.contains("active execution missing while attaching handle"));
}

#[test]
fn active_execution_owner_marks_cancel_requested_before_activation() {
    let cancel_count = Arc::new(AtomicUsize::new(0));
    let owner = ActiveExecutionOwner::new();
    let run_id = RunId::new("run-cancel-before-activate").expect("run id");
    let session_id = SessionId::new("session-cancel-before-activate").expect("session id");
    owner.claim_run(run_id.clone(), session_id.clone());
    owner
        .cancel_run(&run_id, &session_id)
        .expect("cancel should mark pending request");

    let disposition = owner
        .attach_handle(
            &run_id,
            Arc::new(CountingHandle {
                cancel_count: cancel_count.clone(),
            }),
        )
        .expect("attach handle after cancel");

    assert_eq!(disposition, AttachHandleDisposition::CancelRequested);
    assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_run_request_uses_registry_profile_provider_and_harness() {
    let runtime = RuntimeService::from_host_platform_with_paths(
        fixture_host_platform(),
        RuntimeExecutionPaths {
            artifact_root: PathBuf::from("/tmp/taugentic-artifacts"),
        },
    );
    let execution = runtime.run_execution_runtime();
    let session_id = SessionId::new("session-harness-request").expect("session id");
    let run_id = RunId::new("run-harness-request").expect("run id");

    for (runtime_profile_id, provider_id, expected_harness) in [
        (
            "runtime-openai-safe",
            "openai",
            AgentExecutionHarness::NativeLoop,
        ),
        (
            "runtime-codex-safe",
            "codex",
            AgentExecutionHarness::CodexAppServer,
        ),
        (
            "runtime-codex-acp-safe",
            "codex-acp",
            AgentExecutionHarness::Acp {
                provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
            },
        ),
    ] {
        let profile = execution
            .runtime_profile(
                &crate::RuntimeProfileId::new(runtime_profile_id).expect("runtime profile id"),
            )
            .expect("runtime profile should exist");

        let request = execution
            .build_execution_request(ProviderRunStart {
                runtime_profile: &profile,
                session_id: &session_id,
                run_id: &run_id,
                objective: "test objective",
                execution_context: Arc::new(ta_store::default_test_execution_context()),
                fork_initial_state: None,
                output_contract: None,
                model_id: None,
                subagent_recipes: Vec::new(),
            })
            .expect("request should build");

        assert_eq!(request.runtime_profile_id, profile.id);
        assert_eq!(request.provider_id, profile.provider_id);
        assert_eq!(request.provider_id.as_str(), provider_id);
        assert_eq!(request.execution_harness, expected_harness);
        if expected_harness.is_native() {
            let system_prompt = request
                .system_prompt
                .as_deref()
                .expect("native run should include delegation system prompt");
            assert!(system_prompt.starts_with("# Delegation guidelines"));
            assert!(system_prompt.contains("\"subagent\" tool"));
        } else {
            assert_eq!(request.system_prompt, None);
        }
    }
}

#[test]
fn native_provider_run_request_uses_recipe_aware_delegation_prompt() {
    let runtime = RuntimeService::from_host_platform_with_paths(
        fixture_host_platform(),
        RuntimeExecutionPaths {
            artifact_root: PathBuf::from("/tmp/taugentic-artifacts"),
        },
    );
    let execution = runtime.run_execution_runtime();
    let profile = execution
        .runtime_profile(&crate::RuntimeProfileId::new("runtime-openai-safe").expect("profile id"))
        .expect("runtime profile should exist");

    let request = execution
        .build_execution_request(ProviderRunStart {
            runtime_profile: &profile,
            session_id: &SessionId::new("session-recipe-prompt").expect("session id"),
            run_id: &RunId::new("run-recipe-prompt").expect("run id"),
            objective: "test objective",
            execution_context: Arc::new(ta_store::default_test_execution_context()),
            fork_initial_state: None,
            output_contract: None,
            model_id: None,
            subagent_recipes: vec![CapsuleRecipe {
                id: "debug-native-subagent".to_string(),
                name: "Debug Native Subagent".to_string(),
                description: Some("Debugs a focused issue.".to_string()),
                contract: OutputContractKind::Debug,
                prompt_template: "Return a debug result.".to_string(),
                default_model: None,
            }],
        })
        .expect("request should build");

    let system_prompt = request
        .system_prompt
        .as_deref()
        .expect("native run should include delegation system prompt");
    assert!(system_prompt.contains("Prefer a recipeId"));
    assert!(!system_prompt.contains("Available recipes"));
    assert!(!system_prompt.contains("debug-native-subagent"));
}

#[test]
fn provider_run_request_rejects_stale_model_and_auth_refs() {
    let runtime = RuntimeService::from_host_platform_with_paths(
        fixture_host_platform(),
        RuntimeExecutionPaths {
            artifact_root: PathBuf::from("/tmp/taugentic-artifacts"),
        },
    );
    let execution = runtime.run_execution_runtime();
    let session_id = SessionId::new("session-normalized-request").expect("session id");
    let run_id = RunId::new("run-normalized-request").expect("run id");
    let mut profile = execution
        .runtime_profile(
            &crate::RuntimeProfileId::new("runtime-openai-safe").expect("runtime profile id"),
        )
        .expect("runtime profile should exist");
    profile.model_id = Some(AgentRuntimeModelId::new("missing-model").expect("model id"));
    profile.auth_profile_id =
        Some(AuthProfileId::new("missing-auth-profile").expect("auth profile id"));

    let model_error = execution
        .build_execution_request(ProviderRunStart {
            runtime_profile: &profile,
            session_id: &session_id,
            run_id: &run_id,
            objective: "test objective",
            execution_context: Arc::new(ta_store::default_test_execution_context()),
            fork_initial_state: None,
            output_contract: None,
            model_id: None,
            subagent_recipes: Vec::new(),
        })
        .expect_err("request should reject a stale model ref");
    assert!(matches!(
        model_error,
        crate::orchestration::AgentRuntimeServiceError::UnknownModel {
            ref provider_id,
            ref model_id,
        } if provider_id == "openai" && model_id == "missing-model"
    ));

    profile.model_id = None;
    let auth_error = execution
        .build_execution_request(ProviderRunStart {
            runtime_profile: &profile,
            session_id: &session_id,
            run_id: &run_id,
            objective: "test objective",
            execution_context: Arc::new(ta_store::default_test_execution_context()),
            fork_initial_state: None,
            output_contract: None,
            model_id: None,
            subagent_recipes: Vec::new(),
        })
        .expect_err("request should reject a stale auth profile ref");
    assert!(matches!(
        auth_error,
        crate::orchestration::AgentRuntimeServiceError::UnknownAuthProfile {
            ref provider_id,
            ref auth_profile_id,
        } if provider_id == "openai" && auth_profile_id == "missing-auth-profile"
    ));
}

fn fixture_host_platform() -> HostPlatform {
    HostPlatform {
        os: HostOs::Linux,
        version: OsVersion::parse("6.9.0"),
        edition: None,
        linux_distribution: None,
        capabilities: HostCapabilities {
            local_ipc: LocalIpcKind::UnixDomainSocket {
                runtime_dir: PathBuf::from("/tmp/taugentic"),
            },
            sandbox: ta_host_platform::SandboxKind::LinuxLandlockBwrap,
            supports_unix_peer_credentials: true,
            supports_launchd_user_services: false,
            supports_systemd_user_services: true,
            supports_windows_service_control: false,
        },
    }
}
