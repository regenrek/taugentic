use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ta_protocol::wire::{
    EnvPolicy, TERMINAL_INPUT_MAX_BYTES, TERMINAL_MAX_COLS, TERMINAL_MAX_ROWS, TERMINAL_MIN_COLS,
    TERMINAL_MIN_ROWS, TerminalAttachParams, TerminalCloseParams, TerminalCloseResult,
    TerminalDetachParams, TerminalDetachResult, TerminalInputParams, TerminalInputResult,
    TerminalListParams, TerminalListResult, TerminalResizeParams, TerminalResizeResult,
    TerminalSpawnParams, TerminalSpawnResult,
};
use ta_store::PersistenceStore;

use super::{AppService, AppServiceError, sanitize_session_owner_principal_id};
use crate::workspace::terminal::TerminalRuntimeSubscription;

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn spawn_terminal(
        &self,
        owner_principal_id: &str,
        params: &TerminalSpawnParams,
    ) -> Result<TerminalSpawnResult, AppServiceError> {
        if !params.user_approved {
            return Err(AppServiceError::TerminalApprovalRequired);
        }
        validate_terminal_size(params.rows, params.cols)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let workspace = self.project_workspace(
            &owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let mut sandbox = ta_exec::SandboxProfile::new()
            .read_path(workspace.root_realpath.as_path())
            .write_path(workspace.root_realpath.as_path())
            .network(ta_exec::NetworkPolicy::Open)
            .child_inherits_tty(true);
        let EnvPolicy::Allowlist { vars } = EnvPolicy::workspace_default() else {
            unreachable!("workspace terminal environment policy is always an allowlist")
        };
        for name in vars {
            sandbox = sandbox.env(name);
        }
        let terminal = self.runtime.terminals.spawn(
            owner_principal_id,
            params.project_id.clone(),
            &workspace,
            params.rows,
            params.cols,
            sandbox,
        )?;
        Ok(TerminalSpawnResult { terminal })
    }

    pub fn list_terminals(
        &self,
        owner_principal_id: &str,
        params: &TerminalListParams,
    ) -> Result<TerminalListResult, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        self.project_workspace(
            &owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        Ok(TerminalListResult {
            terminals: self.runtime.terminals.list(
                &owner_principal_id,
                &params.project_id,
                &params.workspace_id,
            ),
        })
    }

    pub(crate) fn attach_terminal(
        &self,
        owner_principal_id: &str,
        params: &TerminalAttachParams,
        connection_id: usize,
    ) -> Result<TerminalRuntimeSubscription, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        self.runtime
            .terminals
            .attach(&owner_principal_id, &params.terminal_id, connection_id)
    }

    pub fn detach_terminal(
        &self,
        owner_principal_id: &str,
        params: &TerminalDetachParams,
        connection_id: usize,
    ) -> Result<TerminalDetachResult, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        Ok(TerminalDetachResult {
            detached: self.runtime.terminals.detach(
                &owner_principal_id,
                &params.terminal_id,
                connection_id,
            )?,
        })
    }

    pub fn terminal_input(
        &self,
        owner_principal_id: &str,
        params: &TerminalInputParams,
    ) -> Result<TerminalInputResult, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let bytes = BASE64
            .decode(&params.data_base64)
            .map_err(|_| AppServiceError::TerminalInvalidInput)?;
        if bytes.is_empty() || bytes.len() > TERMINAL_INPUT_MAX_BYTES {
            return Err(AppServiceError::TerminalInvalidInput);
        }
        self.runtime
            .terminals
            .input(&owner_principal_id, &params.terminal_id, &bytes)?;
        Ok(TerminalInputResult {
            accepted_bytes: bytes.len().try_into().unwrap_or(u32::MAX),
        })
    }

    pub fn resize_terminal(
        &self,
        owner_principal_id: &str,
        params: &TerminalResizeParams,
    ) -> Result<TerminalResizeResult, AppServiceError> {
        validate_terminal_size(params.rows, params.cols)?;
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let terminal = self.runtime.terminals.resize(
            &owner_principal_id,
            &params.terminal_id,
            params.rows,
            params.cols,
        )?;
        Ok(TerminalResizeResult { terminal })
    }

    pub fn close_terminal(
        &self,
        owner_principal_id: &str,
        params: &TerminalCloseParams,
    ) -> Result<TerminalCloseResult, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let terminal = self
            .runtime
            .terminals
            .close(&owner_principal_id, &params.terminal_id)?;
        Ok(TerminalCloseResult { terminal })
    }
}

fn validate_terminal_size(rows: u16, cols: u16) -> Result<(), AppServiceError> {
    if !(TERMINAL_MIN_ROWS..=TERMINAL_MAX_ROWS).contains(&rows)
        || !(TERMINAL_MIN_COLS..=TERMINAL_MAX_COLS).contains(&cols)
    {
        return Err(AppServiceError::TerminalInvalidSize { rows, cols });
    }
    Ok(())
}
