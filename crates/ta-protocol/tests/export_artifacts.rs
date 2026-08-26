use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

#[test]
fn export_protocol_artifacts_write_generated_index_and_schema_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let export_root = std::env::temp_dir().join(format!("ta-protocol-test-{unique}"));

    ta_protocol::export_protocol_artifacts(&export_root).expect("protocol export should succeed");

    let index = fs::read_to_string(export_root.join("generated/index.ts"))
        .expect("generated index should exist");
    assert!(index.contains("export type { PublicApprovalResolution }"));
    assert!(index.contains("export type { ArtifactEvent }"));
    assert!(index.contains("export type { ArtifactSummary }"));
    assert!(index.contains("export type { DaemonApprovalDecideParams }"));
    assert!(index.contains("export type { DaemonApprovalDecideResult }"));
    assert!(index.contains("export type { GetArtifactQuery }"));
    assert!(index.contains("export type { DaemonSessionOpenResult }"));
    assert!(index.contains("export type { DelegateRequest }"));
    assert!(index.contains("export type { SessionAuthority }"));
    assert!(index.contains("export type { PublicActivityPageResult }"));
    assert!(index.contains("export type { PublicDaemonEventEnvelope }"));
    assert!(index.contains("export type { AgentStreamEvent }"));
    assert!(index.contains("export type { AgentStreamFrame }"));
    assert!(index.contains("export type { OutputContractKind }"));
    assert!(index.contains("export type { CapsuleResult }"));
    assert!(index.contains("export type { DebugResult }"));
    assert!(index.contains("export type { PatchResult }"));
    assert!(index.contains("export type { ReviewResult }"));
    assert!(index.contains("export type { TestResult }"));
    assert!(index.contains("export type { PlanResult }"));
    assert!(index.contains("export type { ValidationError }"));
    assert!(index.contains("export type { CapsuleRecipe }"));
    assert!(index.contains("export type { RecipeValidationError }"));
    assert!(index.contains("export type { RecipeResolutionError }"));
    assert!(index.contains("export type { SessionOverviewQuery }"));
    assert!(index.contains("export type { SessionOverviewResult }"));
    assert!(index.contains("export type { SessionOverview }"));
    assert!(index.contains("export type { StartRunCommand }"));
    assert!(index.contains("export type { DaemonRunCancelParams }"));
    assert!(index.contains("export type { AgentRuntimeSnapshot }"));
    assert!(index.contains("export type { RuntimeProfilePatch }"));
    assert!(index.contains("export type { AgentRuntimeStrategyInfo }"));
    assert!(index.contains("export type { AuthProfileState }"));
    assert!(index.contains("export type { Workspace }"));
    assert!(index.contains("export type { WorkspaceId }"));
    assert!(index.contains("export type { WorkspacePath }"));
    assert!(index.contains("export type { WorkspacePathError }"));
    assert!(index.contains("export type { ExecutionContext }"));
    assert!(index.contains("export type { WorkspaceScope }"));
    assert!(index.contains("export type { PermissionPolicy }"));
    assert!(index.contains("export type { EnvPolicy }"));
    assert!(!index.contains("export type { ApprovalEvent }"));
    assert!(!index.contains("export type { ApprovalResolution }"));
    assert!(!index.contains("export type { ActivityPageItem }"));
    assert!(!index.contains("export type { ActivityPageResult }"));
    assert!(!index.contains("export type { DaemonEvent }"));
    assert!(!index.contains("export type { DaemonEventEnvelope }"));
    assert!(!index.contains("export type { Query }"));
    assert!(!index.contains("export type { CancelRunCommand }"));
    assert!(!index.contains("export type { DecideApprovalCommand }"));
    assert!(index.contains("PROTOCOL_VERSION"));
    assert!(index.contains("DAEMON_SOCKET_NAME_ENV_VAR"));
    assert!(index.contains("METHOD_DAEMON_APPROVAL_DECIDE"));
    assert!(index.contains("METHOD_DAEMON_ARTIFACT_GET"));
    assert!(index.contains("METHOD_DAEMON_ARTIFACT_LIST"));
    assert!(index.contains("METHOD_DAEMON_SESSION_OVERVIEW"));
    assert!(index.contains("METHOD_DAEMON_RUN_START"));
    assert!(index.contains("METHOD_DAEMON_RUN_CANCEL"));
    assert!(index.contains("METHOD_DAEMON_SESSION_OPEN"));
    assert!(index.contains("METHOD_DAEMON_AGENT_RUNTIME_GET"));
    assert!(index.contains("METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH"));
    assert!(index.contains("METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN"));
    assert!(index.contains("export const DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT = 8;"));
    assert!(index.contains("export const MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT = 8;"));

    let workspace_selector = fs::read_to_string(export_root.join("generated/WorkspaceSelector.ts"))
        .expect("workspace selector should exist");
    assert!(workspace_selector.contains("byProject"));
    assert!(workspace_selector.contains("projectId: ProjectId"));
    assert!(workspace_selector.contains("workspaceId: WorkspaceId"));

    let runtime = fs::read_to_string(export_root.join("generated/runtime.ts"))
        .expect("generated runtime schema module should exist");
    assert!(runtime.contains("PublicApprovalResolution"));
    assert!(runtime.contains("PROTOCOL_JSON_SCHEMAS"));
    assert!(runtime.contains("ArtifactEvent"));
    assert!(runtime.contains("AgentStreamEvent"));
    assert!(runtime.contains("AgentStreamFrame"));
    assert!(runtime.contains("OutputContractKind"));
    assert!(runtime.contains("CapsuleResult"));
    assert!(runtime.contains("DebugResult"));
    assert!(runtime.contains("PatchResult"));
    assert!(runtime.contains("ReviewResult"));
    assert!(runtime.contains("TestResult"));
    assert!(runtime.contains("PlanResult"));
    assert!(runtime.contains("ValidationError"));
    assert!(runtime.contains("CapsuleRecipe"));
    assert!(runtime.contains("RecipeValidationError"));
    assert!(runtime.contains("RecipeResolutionError"));
    assert!(runtime.contains("PublicActivityPageResult"));
    assert!(runtime.contains("PublicDaemonEventEnvelope"));
    assert!(runtime.contains("SessionOverviewQuery"));
    assert!(runtime.contains("SessionOverviewResult"));
    assert!(runtime.contains("DaemonApprovalDecideResult"));
    assert!(runtime.contains("GetArtifactQuery"));
    assert!(runtime.contains("DaemonSessionOpenResult"));
    assert!(runtime.contains("DelegateRequest"));
    assert!(runtime.contains("SessionAuthority"));
    assert!(runtime.contains("DaemonStatusResult"));
    assert!(runtime.contains("SessionOverview"));
    assert!(runtime.contains("DaemonRunCancelParams"));
    assert!(runtime.contains("\"logPath\""));
    assert!(runtime.contains("AgentRuntimeSnapshot"));
    assert!(runtime.contains("RuntimeProfilePatch"));
    assert!(runtime.contains("AgentRuntimeStrategyInfo"));
    assert!(runtime.contains("AuthProfileState"));
    assert!(runtime.contains("Workspace"));
    assert!(runtime.contains("ExecutionContext"));
    assert!(runtime.contains("WorkspaceScope"));
    assert!(
        !runtime.contains("\"format\": \"float\""),
        "runtime schema bundle must stay AJV-compatible"
    );
    assert!(!runtime.contains("\n  ApprovalEvent:"));
    assert!(!runtime.contains("\n  ApprovalResolution:"));
    assert!(!runtime.contains("\n  ActivityPageItem:"));
    assert!(!runtime.contains("\n  ActivityPageResult:"));
    assert!(!runtime.contains("\n  DaemonEvent:"));
    assert!(!runtime.contains("\n  DaemonEventEnvelope:"));
    assert!(!runtime.contains("\n  Query:"));
    assert!(!runtime.contains("\n  CancelRunCommand:"));
    assert!(!runtime.contains("\n  DecideApprovalCommand:"));

    let runtime_index = fs::read_to_string(export_root.join("generated/index.js"))
        .expect("generated runtime index should exist");
    assert!(runtime_index.contains("METHOD_DAEMON_AGENT_RUNTIME_GET"));
    assert!(runtime_index.contains("METHOD_DAEMON_RUN_CANCEL"));

    let command_binding = fs::read_to_string(export_root.join("generated/StartRunCommand.ts"))
        .expect("start run binding should exist");
    assert!(command_binding.contains("export type StartRunCommand"));
    assert!(!command_binding.contains("sandboxProfile"));
    let cancel_binding = fs::read_to_string(export_root.join("generated/DaemonRunCancelParams.ts"))
        .expect("daemon run cancel binding should exist");
    assert!(cancel_binding.contains("export type DaemonRunCancelParams"));
    let delegate_binding = fs::read_to_string(export_root.join("generated/DelegateRequest.ts"))
        .expect("delegate request binding should exist");
    assert!(delegate_binding.contains("modelId?: AgentRuntimeModelId | null"));
    assert!(!delegate_binding.contains("sandboxProfile"));
    assert!(delegate_binding.contains("recipeId?: string | null"));
    let agent_runtime_binding =
        fs::read_to_string(export_root.join("generated/AgentRuntimeSnapshot.ts"))
            .expect("agent runtime snapshot binding should exist");
    assert!(agent_runtime_binding.contains("export type AgentRuntimeSnapshot"));
    let run_event_delta_binding =
        fs::read_to_string(export_root.join("generated/RunEventDelta.ts"))
            .expect("run event delta binding should exist");
    assert!(run_event_delta_binding.contains("seq: string"));
    for entry in fs::read_dir(export_root.join("generated")).expect("generated dir should exist") {
        let entry = entry.expect("generated entry should be readable");
        if entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("ts")
        {
            continue;
        }
        let binding = fs::read_to_string(entry.path()).expect("generated binding should read");
        assert!(
            !binding.contains("bigint"),
            "generated TypeScript must match the serialized JSON representation: {}",
            entry.path().display()
        );
        assert!(
            binding.lines().all(|line| line == line.trim_end()),
            "generated TypeScript must not contain trailing whitespace: {}",
            entry.path().display()
        );
    }
    assert!(
        !export_root.join("generated/generated").exists(),
        "typescript export should not create a nested generated directory"
    );

    let run_status_schema = fs::read_to_string(export_root.join("generated/schema/RunStatus.json"))
        .expect("run status schema should exist");
    assert!(run_status_schema.contains("\"title\": \"RunStatus\""));
    let cancel_schema =
        fs::read_to_string(export_root.join("generated/schema/DaemonRunCancelParams.json"))
            .expect("daemon run cancel schema should exist");
    assert!(cancel_schema.contains("\"title\": \"DaemonRunCancelParams\""));
    let capsule_result_schema =
        fs::read_to_string(export_root.join("generated/schema/CapsuleResult.json"))
            .expect("capsule result schema should exist");
    assert!(capsule_result_schema.contains("\"title\": \"CapsuleResult\""));
    assert!(capsule_result_schema.contains("\"const\": \"custom\""));
    assert!(
        !capsule_result_schema.contains("\"format\": \"float\""),
        "runtime AJV rejects non-standard float formats"
    );

    let debug_result_schema_text =
        fs::read_to_string(export_root.join("generated/schema/DebugResult.json"))
            .expect("debug result schema should exist");
    let debug_result_schema: Value =
        serde_json::from_str(&debug_result_schema_text).expect("debug schema should parse");
    let confidence_schema = &debug_result_schema["properties"]["confidence"];
    assert_eq!(confidence_schema["type"].as_str(), Some("number"));
    assert_eq!(confidence_schema["minimum"].as_f64(), Some(0.0));
    assert_eq!(confidence_schema["maximum"].as_f64(), Some(1.0));
    assert!(
        confidence_schema.get("format").is_none(),
        "confidence schema must stay AJV-compatible"
    );

    let validation_error_schema =
        fs::read_to_string(export_root.join("generated/schema/ValidationError.json"))
            .expect("validation error schema should exist");
    assert!(
        !validation_error_schema.contains("\"format\": \"float\""),
        "validation errors must not export AJV-unknown float formats"
    );
    assert!(validation_error_schema.contains("\"const\": \"testCountsInconsistent\""));
    assert!(validation_error_schema.contains("\"sumOfParts\""));
    assert!(validation_error_schema.contains("\"const\": \"planStepDependencyOutOfRange\""));
    assert!(validation_error_schema.contains("\"stepIndex\""));
    assert!(validation_error_schema.contains("\"totalSteps\""));
    assert!(!validation_error_schema.contains("\"sum_of_parts\""));
    assert!(!validation_error_schema.contains("\"step_index\""));
    assert!(!validation_error_schema.contains("\"total_steps\""));

    let recipe_schema = fs::read_to_string(export_root.join("generated/schema/CapsuleRecipe.json"))
        .expect("capsule recipe schema should exist");
    assert!(recipe_schema.contains("\"title\": \"CapsuleRecipe\""));
    assert!(recipe_schema.contains("\"promptTemplate\""));
    assert!(recipe_schema.contains("\"defaultModel\""));
    assert!(!recipe_schema.contains("\"defaultSandboxProfile\""));
    assert!(!recipe_schema.contains("\"prompt_template\""));
    assert!(!recipe_schema.contains("\"default_model\""));
    assert!(!recipe_schema.contains("\"default_sandbox_profile\""));

    let recipe_error_schema =
        fs::read_to_string(export_root.join("generated/schema/RecipeValidationError.json"))
            .expect("recipe validation error schema should exist");
    assert!(recipe_error_schema.contains("\"const\": \"invalidIdCharacters\""));
    assert!(recipe_error_schema.contains("\"value\""));

    let recipe_resolution_error_schema =
        fs::read_to_string(export_root.join("generated/schema/RecipeResolutionError.json"))
            .expect("recipe resolution error schema should exist");
    assert!(recipe_resolution_error_schema.contains("\"const\": \"unknownRecipeId\""));
    assert!(recipe_resolution_error_schema.contains("\"recipeContract\""));

    let session_authority_schema =
        fs::read_to_string(export_root.join("generated/schema/SessionAuthority.json"))
            .expect("session authority schema should exist");
    assert!(session_authority_schema.contains("\"title\": \"string\""));
    let runtime_profile_patch_schema =
        fs::read_to_string(export_root.join("generated/schema/RuntimeProfilePatch.json"))
            .expect("runtime profile patch schema should exist");
    assert!(runtime_profile_patch_schema.contains("\"title\": \"RuntimeProfilePatch\""));
    let agent_runtime_snapshot_schema =
        fs::read_to_string(export_root.join("generated/schema/AgentRuntimeSnapshot.json"))
            .expect("agent runtime snapshot schema should exist");
    assert!(agent_runtime_snapshot_schema.contains("\"title\": \"AgentRuntimeSnapshot\""));
    let execution_context_schema =
        fs::read_to_string(export_root.join("generated/schema/ExecutionContext.json"))
            .expect("execution context schema should exist");
    assert!(execution_context_schema.contains("\"title\": \"ExecutionContext\""));
    assert!(execution_context_schema.contains("\"workspaceScope\""));
    assert!(execution_context_schema.contains("\"permissionPolicy\""));

    fs::remove_dir_all(&export_root).expect("temp export directory should be removable");
}
