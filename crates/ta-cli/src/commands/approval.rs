use ta_daemon_client::DaemonClient;
use ta_protocol::wire::{
    ApprovalDecision, ApprovalId, DaemonApprovalDecideParams, ListApprovalsQuery, SessionId,
};

use crate::{args::ApprovalCommands, error::CliError, output::CommandOutput};

const CLI_CLIENT_NAME: &str = "ta-cli";
const CLI_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(
    daemon_client: &DaemonClient,
    command: ApprovalCommands,
) -> Result<Option<CommandOutput>, CliError> {
    match command {
        ApprovalCommands::List { session } => {
            let session_id = parse_session_id(session)?;
            let mut client =
                daemon_client.connect_persistent(CLI_CLIENT_NAME, CLI_CLIENT_VERSION)?;
            let _ = client.attach_session(session_id)?;
            let approvals = client.list_approvals(ListApprovalsQuery {
                run_id: None,
                approval_id: None,
            })?;
            Ok(Some(CommandOutput::ApprovalList(approvals.items)))
        }
        ApprovalCommands::Decide {
            session,
            approval,
            decision,
            commentary,
        } => {
            let session_id = parse_session_id(session)?;
            let approval_id = parse_approval_id(approval)?;
            let mut client =
                daemon_client.connect_persistent(CLI_CLIENT_NAME, CLI_CLIENT_VERSION)?;
            let _ = client.attach_session(session_id)?;
            let decided = client.decide_approval(DaemonApprovalDecideParams {
                approval_id,
                decision: parse_decision(decision)?,
                commentary,
            })?;
            Ok(Some(CommandOutput::ApprovalDecide(decided.run)))
        }
    }
}

fn parse_session_id(value: String) -> Result<SessionId, CliError> {
    SessionId::new(value).map_err(|error| CliError::InvalidInput(error.to_string()))
}

fn parse_approval_id(value: String) -> Result<ApprovalId, CliError> {
    ApprovalId::new(value).map_err(|error| CliError::InvalidInput(error.to_string()))
}

fn parse_decision(value: String) -> Result<ApprovalDecision, CliError> {
    match value.as_str() {
        "approved" => Ok(ApprovalDecision::Approved),
        "rejected" => Ok(ApprovalDecision::Rejected),
        _ => Err(CliError::InvalidInput(format!(
            "approval decision must be approved or rejected, got: {value}"
        ))),
    }
}
