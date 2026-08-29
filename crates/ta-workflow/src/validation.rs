use std::{fs, path::Path};

use ta_protocol::wire::{
    WORKFLOW_KIND_V1, WorkflowBudgetLimits, WorkflowDefinition, WorkflowSourceKind,
    WorkflowValidationError, WorkflowValidationReport,
};

use crate::yaml_keys::duplicate_key_errors;

pub fn load_workflow_file(path: &Path) -> WorkflowValidationReport {
    match fs::read_to_string(path) {
        Ok(contents) => validate_workflow_yaml(&contents),
        Err(error) => invalid("$", format!("failed to read workflow file: {error}")),
    }
}

pub fn validate_workflow_yaml(contents: &str) -> WorkflowValidationReport {
    validate(contents)
}

pub(crate) fn parse_valid_workflow(
    contents: &str,
) -> Result<WorkflowDefinition, WorkflowValidationReport> {
    let report = validate_workflow_yaml(contents);
    if !report.valid {
        return Err(report);
    }
    serde_yaml::from_str(contents).map_err(|error| invalid("$", error.to_string()))
}

fn validate(contents: &str) -> WorkflowValidationReport {
    let mut errors = duplicate_key_errors(contents);
    let workflow = match serde_yaml::from_str::<WorkflowDefinition>(contents) {
        Ok(workflow) => workflow,
        Err(error) => {
            errors.push(WorkflowValidationError {
                path: "$".to_string(),
                message: error.to_string(),
            });
            return report(errors);
        }
    };

    validate_workflow(&workflow, &mut errors);
    report(errors)
}

fn validate_workflow(workflow: &WorkflowDefinition, errors: &mut Vec<WorkflowValidationError>) {
    if workflow.kind != WORKFLOW_KIND_V1 {
        push(errors, "$.kind", format!("kind must be {WORKFLOW_KIND_V1}"));
    }
    if workflow.name.trim().is_empty() {
        push(errors, "$.name", "name must not be empty");
    }
    if workflow.source.active_states.is_empty() {
        push(
            errors,
            "$.source.active_states",
            "at least one active state is required",
        );
    }
    if workflow.source.terminal_states.is_empty() {
        push(
            errors,
            "$.source.terminal_states",
            "at least one terminal state is required",
        );
    }
    if workflow.source.kind == WorkflowSourceKind::GithubIssues && workflow.source.repo.is_none() {
        push(
            errors,
            "$.source.repo",
            "github_issues source requires repo in owner/name form",
        );
    }
    if workflow.source.kind == WorkflowSourceKind::GithubIssues
        && workflow.source.code_host_account_id.is_none()
    {
        push(
            errors,
            "$.source.code_host_account_id",
            "github_issues source requires an explicit code-host account id",
        );
    }
    validate_positive_u32(
        workflow.orchestrator.max_concurrent_missions,
        "$.orchestrator.max_concurrent_missions",
        errors,
    );
    validate_positive_u32(
        workflow.orchestrator.max_capsules_per_mission,
        "$.orchestrator.max_capsules_per_mission",
        errors,
    );
    validate_positive_u64(
        workflow.orchestrator.retry.initial_ms,
        "$.orchestrator.retry.initial_ms",
        errors,
    );
    validate_positive_u64(
        workflow.orchestrator.retry.max_ms,
        "$.orchestrator.retry.max_ms",
        errors,
    );
    if workflow.orchestrator.retry.initial_ms > workflow.orchestrator.retry.max_ms {
        push(
            errors,
            "$.orchestrator.retry",
            "initial_ms must be less than or equal to max_ms",
        );
    }
    for (index, host) in workflow.policy.network_allowlist.iter().enumerate() {
        if host.trim().is_empty() || host.contains('/') || host.contains(':') {
            push(
                errors,
                format!("$.policy.network_allowlist[{index}]"),
                "network allowlist entries must be hostnames without scheme or path",
            );
        }
    }
    if workflow.runtime_profiles.is_empty() {
        push(
            errors,
            "$.runtime_profiles",
            "at least one runtime profile is required for background sources",
        );
    }
    for capsule_kind in workflow.runtime_profiles.keys() {
        let profile_path = format!("$.runtime_profiles.{capsule_kind}");
        if capsule_kind.trim().is_empty() {
            push(errors, &profile_path, "capsule kind must not be empty");
        }
    }
    if workflow.outputs.required.is_empty() {
        push(
            errors,
            "$.outputs.required",
            "at least one required output is required",
        );
    }
    validate_budget_limits(
        "$.budgets.per_capsule",
        &workflow.budgets.per_capsule,
        errors,
    );
    validate_budget_limits(
        "$.budgets.per_orchestrator",
        &workflow.budgets.per_orchestrator,
        errors,
    );
    validate_budget_limits(
        "$.budgets.per_workflow",
        &workflow.budgets.per_workflow,
        errors,
    );
}

fn validate_budget_limits(
    path: &str,
    limits: &WorkflowBudgetLimits,
    errors: &mut Vec<WorkflowValidationError>,
) {
    if matches!(limits.max_tokens, Some(0)) {
        push(
            errors,
            format!("{path}.max_tokens"),
            "max_tokens must be greater than zero",
        );
    }
    if let Some(cost) = limits.max_cost_usd
        && (!cost.is_finite() || cost < 0.0)
    {
        push(
            errors,
            format!("{path}.max_cost_usd"),
            "max_cost_usd must be finite and non-negative",
        );
    }
    if matches!(limits.max_wall_time_ms, Some(0)) {
        push(
            errors,
            format!("{path}.max_wall_time_ms"),
            "max_wall_time_ms must be greater than zero",
        );
    }
}

fn validate_positive_u32(value: u32, path: &str, errors: &mut Vec<WorkflowValidationError>) {
    if value == 0 {
        push(errors, path, "value must be greater than zero");
    }
}

fn validate_positive_u64(value: u64, path: &str, errors: &mut Vec<WorkflowValidationError>) {
    if value == 0 {
        push(errors, path, "value must be greater than zero");
    }
}

fn invalid(path: impl Into<String>, message: impl Into<String>) -> WorkflowValidationReport {
    WorkflowValidationReport {
        valid: false,
        errors: vec![WorkflowValidationError {
            path: path.into(),
            message: message.into(),
        }],
    }
}

fn report(errors: Vec<WorkflowValidationError>) -> WorkflowValidationReport {
    WorkflowValidationReport {
        valid: errors.is_empty(),
        errors,
    }
}

fn push(
    errors: &mut Vec<WorkflowValidationError>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    errors.push(WorkflowValidationError {
        path: path.into(),
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = r#"
kind: taugentic.workflow/v1
name: default-github-implementation
source:
  kind: github_issues
  repo: regenrek/taugentic
  code_host_account_id: code-host-account-test
  active_states: ["ready"]
  terminal_states: ["done", "cancelled"]
orchestrator:
  max_concurrent_missions: 8
  max_capsules_per_mission: 6
  retry:
    initial_ms: 10000
    max_ms: 300000
policy:
  approvals:
    file_write: ask
    process: ask_for_sensitive
    network: allowlist
  network_allowlist:
    - github.com
    - api.github.com
runtime_profiles:
  scout:
    provider: codex
    model: gpt-5.6-sol
  implementer:
    provider: openai
    model: gpt-5.6-sol
outputs:
  required:
    - evidence
    - tests
    - patch_or_blocker
    - risk_summary
budgets:
  per_capsule:
    max_tokens: 100000
  per_orchestrator:
    max_wall_time_ms: 3600000
  per_workflow:
    max_cost_usd: 25.0
"#;

    #[test]
    fn parses_golden_workflow_yaml() {
        let report = validate_workflow_yaml(GOLDEN);
        assert_eq!(report.errors, Vec::new());
        assert!(report.valid);
        let workflow = parse_valid_workflow(GOLDEN).expect("workflow should parse");
        assert_eq!(workflow.name, "default-github-implementation");
        assert_eq!(workflow.runtime_profiles.len(), 2);
    }

    #[test]
    fn collects_multiple_validation_errors() {
        let yaml = r#"
kind: taugentic.workflow/v1
name: invalid
source:
  kind: github_issues
  repo: regenrek/taugentic
  code_host_account_id: code-host-account-test
  active_states: ["ready"]
  terminal_states: ["done"]
orchestrator:
  max_concurrent_missions: 0
  max_capsules_per_mission: 6
  retry:
    initial_ms: 10000
    max_ms: 300000
policy:
  approvals:
    file_write: ask
    process: ask
    network: allowlist
  network_allowlist:
    - https://github.com/path
runtime_profiles: {}
outputs:
  required: []
budgets:
  per_capsule: {}
  per_orchestrator: {}
  per_workflow: {}
"#;

        let report = validate_workflow_yaml(yaml);

        assert!(!report.valid);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.path == "$.orchestrator.max_concurrent_missions")
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.path == "$.runtime_profiles")
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.path == "$.policy.network_allowlist[0]")
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.path == "$.outputs.required")
        );
    }

    #[test]
    fn rejects_duplicate_keys() {
        let yaml = GOLDEN.replace(
            "name: default-github-implementation",
            "name: first\nname: second",
        );

        let report = validate_workflow_yaml(&yaml);

        assert!(!report.valid);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.message.contains("duplicate key"))
        );
    }
}
