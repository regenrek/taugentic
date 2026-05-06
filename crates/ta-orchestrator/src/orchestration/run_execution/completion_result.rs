use ta_protocol::wire::{
    CapsuleResult, DaemonEvent, OutputContractKind, ReceiptKind, RunStatus, ValidationError,
    validate_result_against_contract,
};
use ta_store::EventRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionReceiptState {
    Promoted,
    Quarantined,
}

pub(super) trait OutputContractKindLabel {
    fn completion_label(self) -> &'static str;
}

impl OutputContractKindLabel for OutputContractKind {
    fn completion_label(self) -> &'static str {
        match self {
            OutputContractKind::Debug => "Debug",
            OutputContractKind::Patch => "Patch",
            OutputContractKind::Review => "Review",
            OutputContractKind::Test => "Test",
            OutputContractKind::Plan => "Plan",
            OutputContractKind::Custom => "Custom",
        }
    }
}

pub(super) fn validate_completion_payload(
    output_contract: Option<OutputContractKind>,
    result: Option<&CapsuleResult>,
) -> Result<(), ValidationError> {
    match (output_contract, result) {
        (Some(contract), Some(result)) => validate_result_against_contract(contract, result),
        (Some(contract), None) => Err(ValidationError::Custom(format!(
            "output contract {contract:?} requires a matching CapsuleResult"
        ))),
        (None, _) => Ok(()),
    }
}

pub(super) fn completion_event_sequence(records: &[EventRecord]) -> Option<u64> {
    records.iter().find_map(|record| match record.payload {
        DaemonEvent::Run(ta_protocol::wire::RunEvent {
            status: RunStatus::Completed | RunStatus::Failed,
            ..
        }) => Some(record.sequence),
        _ => None,
    })
}

pub(super) fn receipt_kind_for_result(result: &CapsuleResult) -> ReceiptKind {
    receipt_kind_for_contract(result.contract_kind())
}

pub(super) fn receipt_kind_for_contract(contract: OutputContractKind) -> ReceiptKind {
    match contract {
        OutputContractKind::Debug => ReceiptKind::Summary,
        OutputContractKind::Patch => ReceiptKind::Patch,
        OutputContractKind::Review => ReceiptKind::ReviewFinding,
        OutputContractKind::Test => ReceiptKind::TestOutput,
        OutputContractKind::Plan => ReceiptKind::Summary,
        OutputContractKind::Custom => ReceiptKind::Artifact,
    }
}
