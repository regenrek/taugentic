use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use ta_protocol::wire::{
    WorkflowDefinition, WorkflowLoadParams, WorkflowLoadedStatus, WorkflowReloadOutcome,
    WorkflowStatusResult, WorkflowValidateParams, WorkflowValidationReport,
};
use thiserror::Error;

use crate::validation::{parse_valid_workflow, validate_workflow_yaml};

#[derive(Debug, Clone, Default)]
pub struct WorkflowManager {
    state: Arc<RwLock<WorkflowState>>,
}

#[derive(Debug, Clone, Default)]
struct WorkflowState {
    current: Option<Arc<WorkflowDefinition>>,
    path: Option<PathBuf>,
    version: u64,
    last_reload: Option<WorkflowReloadOutcome>,
}

#[derive(Debug, Error)]
pub enum WorkflowManagerError {
    #[error("workflow path is not configured")]
    MissingPath,
    #[error("workflow.validate requires exactly one of path or contents")]
    InvalidValidateParams,
    #[error("workflow file read failed: {0}")]
    ReadFailed(#[from] std::io::Error),
}

impl WorkflowManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<Arc<WorkflowDefinition>> {
        self.state
            .read()
            .expect("workflow state should not be poisoned")
            .current
            .clone()
    }

    pub fn load(
        &self,
        params: &WorkflowLoadParams,
    ) -> Result<WorkflowStatusResult, WorkflowManagerError> {
        let path = PathBuf::from(params.path.trim());
        let contents = fs::read_to_string(&path)?;
        match parse_valid_workflow(&contents) {
            Ok(workflow) => {
                self.swap(path, workflow);
                Ok(self.status())
            }
            Err(report) => {
                self.record_failed_reload(report);
                Ok(self.status())
            }
        }
    }

    pub fn reload(&self) -> Result<WorkflowStatusResult, WorkflowManagerError> {
        let path = self
            .state
            .read()
            .expect("workflow state should not be poisoned")
            .path
            .clone()
            .ok_or(WorkflowManagerError::MissingPath)?;
        let contents = fs::read_to_string(&path)?;
        match parse_valid_workflow(&contents) {
            Ok(workflow) => {
                self.swap(path, workflow);
                Ok(self.status())
            }
            Err(report) => {
                self.record_failed_reload(report);
                Ok(self.status())
            }
        }
    }

    pub fn validate(
        &self,
        params: &WorkflowValidateParams,
    ) -> Result<WorkflowValidationReport, WorkflowManagerError> {
        match (params.path.as_deref(), params.contents.as_deref()) {
            (Some(path), None) => Ok(validate_path(Path::new(path))?),
            (None, Some(contents)) => Ok(validate_workflow_yaml(contents)),
            _ => Err(WorkflowManagerError::InvalidValidateParams),
        }
    }

    pub fn status(&self) -> WorkflowStatusResult {
        let state = self
            .state
            .read()
            .expect("workflow state should not be poisoned");
        WorkflowStatusResult {
            loaded: state
                .current
                .as_ref()
                .zip(state.path.as_ref())
                .map(|(workflow, path)| WorkflowLoadedStatus {
                    name: workflow.name.clone(),
                    path: path.display().to_string(),
                    source_kind: workflow.source.kind,
                    runtime_profile_count: u32::try_from(workflow.runtime_profiles.len())
                        .unwrap_or(u32::MAX),
                    version: state.version,
                }),
            last_reload: state.last_reload.clone(),
        }
    }

    fn swap(&self, path: PathBuf, workflow: WorkflowDefinition) {
        let mut state = self
            .state
            .write()
            .expect("workflow state should not be poisoned");
        let prev_name = state.current.as_ref().map(|current| current.name.clone());
        state.version = state.version.saturating_add(1);
        state.path = Some(path);
        state.current = Some(Arc::new(workflow.clone()));
        state.last_reload = Some(WorkflowReloadOutcome::Reloaded {
            name: workflow.name,
            prev_name,
            version: state.version,
        });
    }

    fn record_failed_reload(&self, report: WorkflowValidationReport) {
        self.state
            .write()
            .expect("workflow state should not be poisoned")
            .last_reload = Some(WorkflowReloadOutcome::Failed {
            errors: report.errors,
        });
    }
}

fn validate_path(path: &Path) -> Result<WorkflowValidationReport, WorkflowManagerError> {
    Ok(validate_workflow_yaml(&fs::read_to_string(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow(name: &str) -> String {
        format!(
            r#"
kind: taugentic.workflow/v1
name: {name}
source:
  kind: github_issues
  repo: regenrek/taugentic
  code_host_account_id: code-host-account-test
  active_states: ["ready"]
  terminal_states: ["done"]
orchestrator:
  max_concurrent_missions: 2
  max_capsules_per_mission: 3
  retry:
    initial_ms: 1000
    max_ms: 10000
policy:
  approvals:
    file_write: ask
    process: ask
    network: allowlist
  network_allowlist: [github.com]
runtime_profiles:
  implementer:
    provider: codex
    model: gpt-5.6-sol
outputs:
  required: [tests, patch_or_blocker]
budgets:
  per_capsule: {{}}
  per_orchestrator: {{}}
  per_workflow: {{}}
"#
        )
    }

    #[test]
    fn reload_swaps_to_valid_workflow() {
        let manager = WorkflowManager::new();
        let file = tempfile::NamedTempFile::new().expect("temp file");
        fs::write(file.path(), workflow("first")).expect("write workflow");
        manager
            .load(&WorkflowLoadParams {
                path: file.path().display().to_string(),
            })
            .expect("load");
        fs::write(file.path(), workflow("second")).expect("rewrite workflow");

        let status = manager.reload().expect("reload");

        assert_eq!(status.loaded.expect("loaded").name, "second");
    }

    #[test]
    fn reload_failure_keeps_last_known_good() {
        let manager = WorkflowManager::new();
        let file = tempfile::NamedTempFile::new().expect("temp file");
        fs::write(file.path(), workflow("first")).expect("write workflow");
        manager
            .load(&WorkflowLoadParams {
                path: file.path().display().to_string(),
            })
            .expect("load");
        fs::write(file.path(), "kind: nope").expect("rewrite workflow");

        let status = manager.reload().expect("reload");

        assert_eq!(status.loaded.expect("loaded").name, "first");
        assert!(matches!(
            status.last_reload,
            Some(WorkflowReloadOutcome::Failed { .. })
        ));
    }
}
