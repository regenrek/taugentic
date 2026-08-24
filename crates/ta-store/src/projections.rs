use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    CapsuleResult, ConflictSummary, ExecutionContext, RunHarnessKind, RunId, RunSource, RunStatus,
    RuntimeProfileId, SessionId, SessionStatus, TrustState, ValidationError, Workspace,
    WorkspaceId, WorkspacePath, WorktreeInfo,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalProjection {
    pub id: String,
    pub client_name: String,
    pub credential_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProjection {
    pub id: SessionId,
    pub owner_client_name: String,
    pub owner_principal_id: String,
    pub current_session_authority_hash: String,
    #[serde(default)]
    pub current_session_authority_generation: u64,
    pub recovery_session_authority_hash: Option<String>,
    pub recovery_session_authority_generation: Option<u64>,
    pub title: String,
    pub status: SessionStatus,
    pub workspace_id: WorkspaceId,
}

/// Workspace projection persisted alongside sessions and runs. Wraps the
/// canonical wire `Workspace` shape verbatim so the daemon, store, and
/// renderer share one source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceProjection(pub Workspace);

impl WorkspaceProjection {
    pub fn new(workspace: Workspace) -> Self {
        Self(workspace)
    }

    pub fn into_inner(self) -> Workspace {
        self.0
    }

    pub fn id(&self) -> &WorkspaceId {
        &self.0.id
    }

    pub fn root_realpath(&self) -> &WorkspacePath {
        &self.0.root_realpath
    }

    pub fn display_name(&self) -> &str {
        &self.0.display_name
    }

    pub fn trust_state(&self) -> &TrustState {
        &self.0.trust_state
    }

    pub fn git_repo_root(&self) -> Option<&WorkspacePath> {
        self.0.git_repo_root.as_ref()
    }

    pub fn created_at(&self) -> &str {
        &self.0.created_at
    }

    pub fn last_used_at(&self) -> &str {
        &self.0.last_used_at
    }
}

impl From<Workspace> for WorkspaceProjection {
    fn from(value: Workspace) -> Self {
        Self(value)
    }
}

impl From<WorkspaceProjection> for Workspace {
    fn from(value: WorkspaceProjection) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunProjection {
    pub id: RunId,
    pub session_id: SessionId,
    pub runtime_profile_id: RuntimeProfileId,
    pub objective: String,
    pub status: RunStatus,
    #[serde(default)]
    pub harness: RunHarnessKind,
    #[serde(default)]
    pub source: RunSource,
    pub execution_context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapsuleResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_violation: Option<ValidationError>,
    #[serde(default)]
    pub started_at_ms: Option<u64>,
    #[serde(default)]
    pub ended_at_ms: Option<u64>,
    #[serde(default)]
    pub last_event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_summary: Option<ConflictSummary>,
}

impl RunProjection {
    pub fn with_commit_metadata(
        mut self,
        existing: Option<&RunProjection>,
        occurred_at_ms: u64,
        last_event_seq: Option<u64>,
    ) -> Self {
        self.started_at_ms = existing
            .and_then(|run| run.started_at_ms)
            .or_else(|| (!matches!(self.status, RunStatus::Queued)).then_some(occurred_at_ms));
        self.ended_at_ms = existing.and_then(|run| run.ended_at_ms).or_else(|| {
            matches!(
                self.status,
                RunStatus::Completed
                    | RunStatus::Failed
                    | RunStatus::BudgetExceeded
                    | RunStatus::Cancelled
            )
            .then_some(occurred_at_ms)
        });
        self.last_event_seq =
            last_event_seq.or_else(|| existing.and_then(|run| run.last_event_seq));
        self
    }
}

pub fn compute_session_status_from_runs(runs: &[RunProjection]) -> SessionStatus {
    if runs.is_empty() {
        return SessionStatus::Idle;
    }

    let mut has_completed_run = false;
    let mut has_cancelled_run = false;
    let mut has_failed_run = false;

    for run in runs {
        match run.status {
            RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval => {
                return SessionStatus::Running;
            }
            RunStatus::Completed => has_completed_run = true,
            RunStatus::Failed | RunStatus::BudgetExceeded => has_failed_run = true,
            RunStatus::Cancelled => has_cancelled_run = true,
        }
    }

    if has_failed_run {
        SessionStatus::Failed
    } else if has_completed_run && !has_cancelled_run {
        SessionStatus::Completed
    } else {
        SessionStatus::Idle
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ta_protocol::wire::AgentStreamTurnId;

    use super::*;

    #[test]
    fn run_projection_defaults_missing_source_to_user_when_context_is_present() {
        let mut value = json!({
            "id": "run-1",
            "session_id": "session-1",
            "runtime_profile_id": "runtime-openai-safe",
            "objective": "Ship native child runs",
            "status": "queued"
        });
        value["execution_context"] =
            serde_json::to_value(crate::default_test_execution_context()).expect("test context");
        let projection: RunProjection =
            serde_json::from_value(value).expect("run projection should decode");

        assert_eq!(projection.source, RunSource::default());
        assert_eq!(projection.harness, RunHarnessKind::Unknown);
    }

    #[test]
    fn run_projection_roundtrips_native_subagent_source() {
        let projection = RunProjection {
            id: RunId::new("run-child").expect("run id"),
            session_id: SessionId::new("session-1").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Review focused files".to_string(),
            status: RunStatus::Queued,
            harness: RunHarnessKind::Native,
            source: RunSource::NativeSubagent {
                parent_run_id: RunId::new("run-parent").expect("parent run id"),
                parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("parent turn id"),
                output_contract: None,
                model_id: None,
                sandbox_profile: None,
                recipe_id: None,
                workspace_scope: Default::default(),
                cleanup_policy: Default::default(),
                planned_write_files: Vec::new(),
            },
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        };

        let json = serde_json::to_value(&projection).expect("projection should serialize");
        let decoded: RunProjection =
            serde_json::from_value(json).expect("projection should deserialize");

        assert_eq!(decoded, projection);
    }

    #[test]
    fn run_source_forked_roundtrips_parent_event_seq() {
        let projection = RunProjection {
            id: RunId::new("run-fork").expect("run id"),
            session_id: SessionId::new("session-1").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Continue from parent checkpoint".to_string(),
            status: RunStatus::Queued,
            harness: RunHarnessKind::Native,
            source: RunSource::Forked {
                parent_run_id: RunId::new("run-parent").expect("parent run id"),
                parent_event_seq: 42,
            },
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        };

        let json = serde_json::to_value(&projection).expect("projection should serialize");
        let decoded: RunProjection =
            serde_json::from_value(json).expect("projection should deserialize");

        assert_eq!(decoded, projection);
    }

    #[test]
    fn run_projection_commit_metadata_tracks_lifecycle_bounds() {
        let run = RunProjection {
            id: RunId::new("run-1").expect("run id"),
            session_id: SessionId::new("session-1").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "resume".to_string(),
            status: RunStatus::Running,
            harness: RunHarnessKind::Native,
            source: RunSource::default(),
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        }
        .with_commit_metadata(None, 100, Some(7));

        let completed = RunProjection {
            status: RunStatus::Completed,
            ..run.clone()
        }
        .with_commit_metadata(Some(&run), 200, Some(9));

        assert_eq!(completed.started_at_ms, Some(100));
        assert_eq!(completed.ended_at_ms, Some(200));
        assert_eq!(completed.last_event_seq, Some(9));
    }

    fn run_with_status(id: &str, status: RunStatus) -> RunProjection {
        RunProjection {
            id: RunId::new(id).expect("run id"),
            session_id: SessionId::new("session-1").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: format!("Objective {id}"),
            status,
            harness: RunHarnessKind::Native,
            source: RunSource::default(),
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        }
    }

    #[test]
    fn rejecting_only_run_marks_session_failed() {
        let runs = vec![run_with_status("run-1", RunStatus::Failed)];

        assert_eq!(
            compute_session_status_from_runs(&runs),
            SessionStatus::Failed
        );
    }

    #[test]
    fn completing_all_runs_marks_session_completed() {
        let runs = vec![
            run_with_status("run-1", RunStatus::Completed),
            run_with_status("run-2", RunStatus::Completed),
        ];

        assert_eq!(
            compute_session_status_from_runs(&runs),
            SessionStatus::Completed
        );
    }

    #[test]
    fn mixed_terminal_states_resolves_to_failed_when_any_failed() {
        let runs = vec![
            run_with_status("run-1", RunStatus::Completed),
            run_with_status("run-2", RunStatus::Failed),
            run_with_status("run-3", RunStatus::Cancelled),
        ];

        assert_eq!(
            compute_session_status_from_runs(&runs),
            SessionStatus::Failed
        );
    }
}
