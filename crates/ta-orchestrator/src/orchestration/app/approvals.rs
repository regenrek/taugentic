use ta_store::{PersistenceStore, SessionApprovalQuery, SessionEventPageQuery};

use crate::{
    ApprovalActor, ApprovalSnapshotResult, DaemonApprovalDecideParams, ListApprovalsQuery,
    RunSummary, to_event_cursor,
};

use super::{
    AppDeferredMutationResult, AppService, AppServiceError, map_run_execution_error,
    map_run_mutation_result,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn list_approvals(
        &self,
        session_id: &crate::SessionId,
        query: &ListApprovalsQuery,
    ) -> Result<ApprovalSnapshotResult, AppServiceError> {
        self.run_execution
            .expire_pending_approvals_for_session(session_id)
            .map_err(map_run_execution_error)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        let page = store.session_event_page(&SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 1,
            kinds: Vec::new(),
        })?;
        Ok(ApprovalSnapshotResult {
            items: store.approvals_for_session(&SessionApprovalQuery {
                session_id: session_id.clone(),
                run_id: query.run_id.clone(),
                approval_id: query.approval_id.clone(),
            })?,
            latest_cursor: page
                .latest_sequence
                .map(|sequence| to_event_cursor(&self.daemon_instance_id, session_id, sequence)),
        })
    }

    pub fn decide_approval(
        &self,
        session_id: &crate::SessionId,
        actor: &ApprovalActor,
        params: &DaemonApprovalDecideParams,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .decide_approval(session_id.clone(), actor.clone(), params.clone())
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }
}
