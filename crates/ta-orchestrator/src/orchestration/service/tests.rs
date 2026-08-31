use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::active_execution::{ActiveExecutionOwner, AttachHandleDisposition};
use super::*;
use crate::{RunId, SessionId};
use ta_host_platform::{HostCapabilities, HostOs, HostPlatform, LocalIpcKind, OsVersion};
use ta_protocol::wire::{
    AgentRuntimeModelId, ApprovalActor, ApprovalDecision, ApprovalId, ApprovalResolution,
    ApprovalResolutionReason, AuthProfileId, CapsuleRecipe, OutputContractKind, RunExecutionRoute,
};
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
    let generation = execution
        .live_execution_for(&run_id)
        .filter(|live_execution| live_execution.session_id == session_id)
        .expect("claimed run has an execution")
        .generation;
    execution
        .with_terminal_live_generation_lease_and_take_handle(
            &run_id,
            &session_id,
            generation,
            || Ok(()),
        )
        .expect("terminal lease should release the exact generation");
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

#[derive(Clone)]
struct ApprovalCountingHandle {
    approval_count: Arc<AtomicUsize>,
}

impl ExecutionHandle for ApprovalCountingHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn resolve_approval(&self, _resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        self.approval_count.fetch_add(1, Ordering::SeqCst);
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
        let generation = owner.claim_run(run_id.clone(), session_id);
        owner
            .attach_handle(
                &run_id,
                generation,
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
            1,
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
    let generation = owner.claim_run(run_id.clone(), session_id.clone());
    let disposition = owner
        .attach_handle(
            &run_id,
            generation,
            Arc::new(CountingHandle {
                cancel_count: cancel_count.clone(),
            }),
        )
        .expect("attach handle");
    assert_eq!(disposition, AttachHandleDisposition::Attached);
}

#[test]
fn active_execution_generation_retires_old_callbacks_before_replacement() {
    let owner = ActiveExecutionOwner::new();
    let run_id = RunId::new("run-generation").expect("run id");
    let session_id = SessionId::new("session-generation").expect("session id");
    let old = owner.claim_run(run_id.clone(), session_id.clone());
    let (result, _generation, _old_handle) = owner
        .replace_run_with_generation_lease(&run_id, &session_id, |generation| {
            Ok::<_, ()>(generation)
        })
        .expect("replace");
    let next = result.expect("replacement action should succeed");
    assert!(next > old);
    assert!(!owner.is_current_generation(&run_id, &session_id, old));
    assert!(owner.is_current_generation(&run_id, &session_id, next));
}

#[test]
fn active_execution_owner_preserves_current_generation_and_handle_when_replacement_action_fails() {
    let owner = ActiveExecutionOwner::new();
    let run_id = RunId::new("run-replacement-action-fails").expect("run id");
    let session_id = SessionId::new("session-replacement-action-fails").expect("session id");
    let old_generation = owner.claim_run(run_id.clone(), session_id.clone());
    let approval_count = Arc::new(AtomicUsize::new(0));
    let handle: Arc<dyn ExecutionHandle> = Arc::new(ApprovalCountingHandle {
        approval_count: approval_count.clone(),
    });
    owner
        .attach_handle(&run_id, old_generation, handle.clone())
        .expect("attach old handle");

    let (result, replacement_generation, returned_handle) = owner
        .replace_run_with_generation_lease(&run_id, &session_id, |_| {
            Err::<(), _>("durable replacement action failed")
        })
        .expect("existing owner should lease replacement");

    assert_eq!(result, Err("durable replacement action failed"));
    assert!(replacement_generation > old_generation);
    assert!(returned_handle.is_none());
    assert!(owner.is_current_generation(&run_id, &session_id, old_generation));
    let retained_handle = owner
        .handle_for_tests(&run_id, &session_id)
        .expect("failed replacement retains the old handle");
    assert!(Arc::ptr_eq(&retained_handle, &handle));
    owner
        .resolve_approval(
            &run_id,
            &session_id,
            ApprovalResolution::new(
                ApprovalId::new("approval-replacement-action-fails").expect("approval id"),
                run_id.clone(),
                ApprovalDecision::Approved,
                ApprovalResolutionReason::User,
                ApprovalActor::new("test-principal").expect("approval actor"),
                None,
            ),
        )
        .expect("retained handle remains owned and usable");
    assert_eq!(approval_count.load(Ordering::SeqCst), 1);
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

    for (runtime_profile_id, provider_id, model_id, auth_profile_id, expected_harness) in [
        (
            "runtime-openai-safe",
            "openai",
            "gpt-5.6-sol",
            Some("profile-openai-test"),
            AgentExecutionHarness::NativeLoop,
        ),
        (
            "runtime-codex-safe",
            "codex",
            "gpt-5.6-sol",
            Some("profile-codex-test"),
            AgentExecutionHarness::CodexAppServer,
        ),
        (
            "runtime-codex-acp-safe",
            "codex-acp",
            "model-a",
            None,
            AgentExecutionHarness::Acp {
                provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
            },
        ),
        (
            "runtime-deepseek-safe",
            "deepseek",
            "deepseek-v4-pro",
            None,
            AgentExecutionHarness::DeepSeekHarness,
        ),
    ] {
        let profile = execution
            .runtime_profile(
                &crate::RuntimeProfileId::new(runtime_profile_id).expect("runtime profile id"),
            )
            .expect("runtime profile should exist");
        let route = RunExecutionRoute {
            runtime_profile_id: profile.id.clone(),
            provider_id: profile.provider_id.clone(),
            model_id: Some(AgentRuntimeModelId::new(model_id).expect("model id")),
            auth_profile_id: auth_profile_id
                .map(|id| AuthProfileId::new(id).expect("auth profile id")),
            harness: crate::orchestration::run_harness_kind(&expected_harness),
        };

        let request = execution
            .build_execution_request(ProviderRunStart {
                runtime_profile: &profile,
                route: &route,
                session_id: &session_id,
                run_id: &run_id,
                objective: "test objective",
                execution_context: Arc::new(ta_store::default_test_execution_context()),
                native_history: None,
                output_contract: None,
                subagent_recipes: Vec::new(),
                attachments: Vec::new(),
                generation: 1,
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
    let route = RunExecutionRoute {
        runtime_profile_id: profile.id.clone(),
        provider_id: profile.provider_id.clone(),
        model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
        auth_profile_id: Some(AuthProfileId::new("profile-openai-test").expect("auth profile id")),
        harness: crate::orchestration::run_harness_kind(&AgentExecutionHarness::NativeLoop),
    };

    let request = execution
        .build_execution_request(ProviderRunStart {
            runtime_profile: &profile,
            route: &route,
            session_id: &SessionId::new("session-recipe-prompt").expect("session id"),
            run_id: &RunId::new("run-recipe-prompt").expect("run id"),
            objective: "test objective",
            execution_context: Arc::new(ta_store::default_test_execution_context()),
            native_history: None,
            output_contract: None,
            subagent_recipes: vec![CapsuleRecipe {
                id: "debug-native-subagent".to_string(),
                name: "Debug Native Subagent".to_string(),
                description: Some("Debugs a focused issue.".to_string()),
                contract: OutputContractKind::Debug,
                prompt_template: "Return a debug result.".to_string(),
                default_model: None,
            }],
            attachments: Vec::new(),
            generation: 1,
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
