use super::{AppService, AppServiceError};
use std::time::{SystemTime, UNIX_EPOCH};
use ta_protocol::wire::{
    ThreadWorkspaceQuery, ThreadWorkspaceResult, ThreadWorkspaceUpdateCommand,
};
use ta_store::PersistenceStore;

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn thread_workspace(
        &self,
        session_id: &ta_protocol::wire::SessionId,
        _query: &ThreadWorkspaceQuery,
    ) -> Result<ThreadWorkspaceResult, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let record = store.thread_workspace(session_id)?.unwrap_or_else(|| {
            ta_store::ThreadWorkspaceRecord {
                session_id: session_id.clone(),
                goal: String::new(),
                plan: String::new(),
                notes: String::new(),
                recap: String::new(),
                pins: Vec::new(),
                work_log: Vec::new(),
            }
        });
        Ok(record)
    }
    pub fn update_thread_workspace(
        &self,
        session_id: &ta_protocol::wire::SessionId,
        command: &ThreadWorkspaceUpdateCommand,
    ) -> Result<ThreadWorkspaceResult, AppServiceError> {
        let event = command.mutation.clone();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppServiceError::SystemClockBeforeUnixEpoch)?
            .as_millis() as u64;
        let mut store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.append_thread_workspace_event(session_id, now, event)?)
    }
}
