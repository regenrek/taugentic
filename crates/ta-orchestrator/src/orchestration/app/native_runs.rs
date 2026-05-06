use ta_store::{NativeRunListCursor, NativeRunListQuery, PersistenceStore};

use crate::{
    ForkRunRequest, ForkRunResult, ListNativeRunsRequest, ListNativeRunsResult,
    NATIVE_RUN_LIST_MAX_LIMIT,
};

use super::{AppService, AppServiceError, map_run_execution_error, project_run_list_entry};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
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
        let filter = request.filter.clone().unwrap_or_default();
        let store = self.store.lock().expect("app store should not be poisoned");
        let page = store.list_native_runs(&NativeRunListQuery {
            session_id: session_id.clone(),
            filter,
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
}
