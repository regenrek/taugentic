use ta_store::PersistenceStore;

use crate::{
    WorkflowLoadParams, WorkflowReloadParams, WorkflowStatusResult, WorkflowValidateParams,
    WorkflowValidationReport,
};

use super::{AppService, AppServiceError};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn load_workflow(
        &self,
        params: &WorkflowLoadParams,
    ) -> Result<WorkflowStatusResult, AppServiceError> {
        let status = self.workflow.load(params)?;
        self.work_source_refresh_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(status)
    }

    pub fn workflow_status(&self) -> WorkflowStatusResult {
        self.workflow.status()
    }

    pub fn reload_workflow(
        &self,
        _params: &WorkflowReloadParams,
    ) -> Result<WorkflowStatusResult, AppServiceError> {
        let status = self.workflow.reload()?;
        self.work_source_refresh_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(status)
    }

    pub fn validate_workflow(
        &self,
        params: &WorkflowValidateParams,
    ) -> Result<WorkflowValidationReport, AppServiceError> {
        Ok(self.workflow.validate(params)?)
    }
}
