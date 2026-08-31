use ta_store::{NativeRunListCursor, NativeRunListQuery, PersistenceStore};

use crate::{
    ContinueRunRequest, ContinueRunResult, ForkRunRequest, ForkRunResult, JoinRunRequest,
    JoinRunResult, ListNativeRunsRequest, ListNativeRunsResult, NATIVE_RUN_LIST_MAX_LIMIT,
    RunHarnessKind, RunLineageGraphRequest, RunLineageGraphResult, RunListFilter, SpawnRunRequest,
    SpawnRunResult, SwitchRouteAndResumeRequest, SwitchRouteAndResumeResult,
};

use super::{AppService, AppServiceError, map_run_execution_error, project_run_list_entry};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn run_lineage_graph(
        &self,
        session_id: &crate::SessionId,
        _request: &RunLineageGraphRequest,
    ) -> Result<RunLineageGraphResult, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(ta_store::run_lineage_graph_from_projections(
            store.runs()?,
            session_id,
        ))
    }

    pub fn list_native_runs(
        &self,
        session_id: &crate::SessionId,
        request: &ListNativeRunsRequest,
    ) -> Result<ListNativeRunsResult, AppServiceError> {
        if request.limit == 0 || request.limit > NATIVE_RUN_LIST_MAX_LIMIT {
            return Err(AppServiceError::InvalidNativeRunListLimit {
                max: NATIVE_RUN_LIST_MAX_LIMIT,
            });
        }
        let before = match request.cursor.as_deref() {
            Some(cursor) => Some(
                NativeRunListCursor::decode(cursor)
                    .ok_or(AppServiceError::InvalidNativeRunListCursor)?,
            ),
            None => None,
        };
        let filter = request.filter.clone().unwrap_or(RunListFilter {
            harness: Some(vec![RunHarnessKind::Native]),
            status: None,
            parent_run_id: None,
        });
        let store = self.store.lock().expect("app store should not be poisoned");
        let page = store.list_native_runs(&NativeRunListQuery {
            session_id: session_id.clone(),
            filter,
            include_children: false,
            before,
            limit: request.limit as usize,
        })?;

        Ok(ListNativeRunsResult {
            runs: page.runs.into_iter().map(project_run_list_entry).collect(),
            next_cursor: page.next_cursor.map(|cursor| cursor.encode()),
        })
    }

    pub fn fork_run(
        &self,
        session_id: &crate::SessionId,
        request: &ForkRunRequest,
    ) -> Result<ForkRunResult, AppServiceError> {
        self.run_execution
            .fork_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn continue_run(
        &self,
        session_id: &crate::SessionId,
        request: &ContinueRunRequest,
    ) -> Result<ContinueRunResult, AppServiceError> {
        self.run_execution
            .continue_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn switch_route_and_resume(
        &self,
        session_id: &crate::SessionId,
        request: &SwitchRouteAndResumeRequest,
    ) -> Result<SwitchRouteAndResumeResult, AppServiceError> {
        self.run_execution
            .switch_route_and_resume(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn spawn_run(
        &self,
        session_id: &crate::SessionId,
        request: &SpawnRunRequest,
    ) -> Result<SpawnRunResult, AppServiceError> {
        self.run_execution
            .spawn_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn join_run(
        &self,
        session_id: &crate::SessionId,
        request: &JoinRunRequest,
    ) -> Result<JoinRunResult, AppServiceError> {
        self.run_execution
            .join_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }
}
