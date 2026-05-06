use ta_protocol::wire::{CapsuleResult, OutputContractKind};

pub(crate) fn parse_completion_result(
    output_contract: Option<OutputContractKind>,
    assistant_text: &str,
) -> Option<CapsuleResult> {
    let _ = output_contract?;

    let trimmed = assistant_text.trim();
    if trimmed.is_empty() {
        return None;
    }

    serde_json::from_str::<CapsuleResult>(trimmed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{DebugResult, OutputContractKind};

    #[test]
    fn contract_completion_parse_failure_delegates_to_daemon_validation() {
        assert_eq!(
            parse_completion_result(Some(OutputContractKind::Debug), ""),
            None
        );
        assert_eq!(
            parse_completion_result(Some(OutputContractKind::Debug), "not json"),
            None
        );
    }

    #[test]
    fn valid_contract_completion_decodes_capsule_result() {
        let result = CapsuleResult::Debug(DebugResult {
            reproduced: false,
            root_cause: None,
            evidence_receipt_ids: Vec::new(),
            patch_receipt_id: None,
            confidence: 0.75,
            blockers: Vec::new(),
        });
        let json = serde_json::to_string(&result).expect("capsule result should serialize");

        assert_eq!(
            parse_completion_result(Some(OutputContractKind::Debug), &json),
            Some(result)
        );
    }
}
