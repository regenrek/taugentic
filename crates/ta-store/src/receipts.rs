use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use ta_protocol::wire::{
    ContextReceipt, ReceiptId, ReceiptKind, ReceiptProvenance, ReceiptState, RunId, SessionId,
};

use crate::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateReceipt {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub parent_run_id: Option<RunId>,
    pub kind: ReceiptKind,
    pub provenance: ReceiptProvenance,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptListQuery {
    pub session_id: SessionId,
    pub run_id: Option<RunId>,
    pub state: Option<ReceiptState>,
    pub kind: Option<ReceiptKind>,
    pub parent_run_id: Option<RunId>,
    pub limit: Option<usize>,
}

pub trait ReceiptRepository {
    fn create(&mut self, input: CreateReceipt) -> Result<ContextReceipt, StoreError>;
    fn promote(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError>;
    fn quarantine(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError>;
    fn receipt(&self, receipt_id: &ReceiptId) -> Result<Option<ContextReceipt>, StoreError>;
    fn list(&self, query: &ReceiptListQuery) -> Result<Vec<ContextReceipt>, StoreError>;
}

pub(crate) fn build_returned_receipt(input: CreateReceipt) -> Result<ContextReceipt, StoreError> {
    let id = receipt_id_for_create(&input)?;
    Ok(ContextReceipt {
        id,
        session_id: input.session_id,
        run_id: input.run_id,
        parent_run_id: input.parent_run_id,
        kind: input.kind,
        provenance: input.provenance,
        state: ReceiptState::Returned,
        title: input.title,
        summary: input.summary,
        created_at_ms: current_time_ms()?,
        promoted_at_ms: None,
        quarantined_at_ms: None,
    })
}

pub(crate) fn receipt_id_for_create(input: &CreateReceipt) -> Result<ReceiptId, StoreError> {
    let Some(key) = unique_key_for_parts(
        &input.session_id,
        &input.run_id,
        input.kind,
        &input.provenance,
    )?
    else {
        return Ok(format!("receipt_{}", uuid::Uuid::new_v4().simple()));
    };
    Ok(format_receipt_id(&key))
}

pub(crate) fn receipt_unique_key(receipt: &ContextReceipt) -> Result<Option<String>, StoreError> {
    unique_key_for_parts(
        &receipt.session_id,
        &receipt.run_id,
        receipt.kind,
        &receipt.provenance,
    )
}

pub(crate) fn receipt_matches_query(receipt: &ContextReceipt, query: &ReceiptListQuery) -> bool {
    receipt.session_id == query.session_id
        && query
            .run_id
            .as_ref()
            .is_none_or(|run_id| receipt.run_id == *run_id)
        && query.state.is_none_or(|state| receipt.state == state)
        && query.kind.is_none_or(|kind| receipt.kind == kind)
        && query
            .parent_run_id
            .as_ref()
            .is_none_or(|parent_run_id| receipt.parent_run_id.as_ref() == Some(parent_run_id))
}

pub(crate) fn apply_promote(mut receipt: ContextReceipt) -> Result<ContextReceipt, StoreError> {
    if receipt.state == ReceiptState::Promoted {
        return Ok(receipt);
    }

    receipt.state = ReceiptState::Promoted;
    if receipt.promoted_at_ms.is_none() {
        receipt.promoted_at_ms = Some(current_time_ms()?);
    }
    Ok(receipt)
}

pub(crate) fn apply_quarantine(mut receipt: ContextReceipt) -> Result<ContextReceipt, StoreError> {
    match receipt.state {
        ReceiptState::Promoted => Err(StoreError::ReceiptTransitionViolation {
            receipt_id: receipt.id,
            detail: "cannot quarantine promoted receipt".to_string(),
        }),
        ReceiptState::Quarantined => Ok(receipt),
        ReceiptState::Returned => {
            receipt.state = ReceiptState::Quarantined;
            if receipt.quarantined_at_ms.is_none() {
                receipt.quarantined_at_ms = Some(current_time_ms()?);
            }
            Ok(receipt)
        }
    }
}

pub(crate) fn receipt_kind_storage(kind: ReceiptKind) -> &'static str {
    match kind {
        ReceiptKind::Evidence => "evidence",
        ReceiptKind::Patch => "patch",
        ReceiptKind::TestOutput => "testOutput",
        ReceiptKind::ReviewFinding => "reviewFinding",
        ReceiptKind::Artifact => "artifact",
        ReceiptKind::Risk => "risk",
        ReceiptKind::Blocker => "blocker",
        ReceiptKind::Summary => "summary",
    }
}

pub(crate) fn receipt_state_storage(state: ReceiptState) -> &'static str {
    match state {
        ReceiptState::Returned => "returned",
        ReceiptState::Promoted => "promoted",
        ReceiptState::Quarantined => "quarantined",
    }
}

fn unique_key_for_parts(
    session_id: &SessionId,
    run_id: &RunId,
    kind: ReceiptKind,
    provenance: &ReceiptProvenance,
) -> Result<Option<String>, StoreError> {
    validate_provenance_shape(provenance)?;

    if let Some(artifact_id) = provenance.artifact_id.as_ref() {
        return Ok(Some(format!(
            "artifact|{}|{}|{}|{}",
            session_id.as_str(),
            run_id.as_str(),
            receipt_kind_storage(kind),
            artifact_id.as_str()
        )));
    }

    match (provenance.event_seq, provenance.agent_turn_id.as_ref()) {
        (Some(event_seq), Some(agent_turn_id)) => Ok(Some(format!(
            "event-turn|{}|{}|{}|{}|{}",
            session_id.as_str(),
            run_id.as_str(),
            receipt_kind_storage(kind),
            event_seq,
            agent_turn_id.as_str()
        ))),
        _ => Ok(None),
    }
}

fn validate_provenance_shape(provenance: &ReceiptProvenance) -> Result<(), StoreError> {
    // `stream_cursor` is descriptive only; not part of identity validation or unique key.
    match (
        provenance.artifact_id.is_some(),
        provenance.event_seq.is_some(),
        provenance.agent_turn_id.is_some(),
    ) {
        (true, false, false) | (false, true, true) | (false, false, false) => Ok(()),
        (true, _, _) => Err(StoreError::InvalidProvenance {
            message: "artifact-derived receipts must not also set event_seq or agent_turn_id"
                .to_string(),
        }),
        (false, _, _) => Err(StoreError::InvalidProvenance {
            message: "event-derived receipts must set both event_seq and agent_turn_id".to_string(),
        }),
    }
}

fn format_receipt_id(unique_key: &str) -> ReceiptId {
    let digest = Sha256::digest(unique_key.as_bytes());
    let mut id = String::with_capacity("receipt_".len() + 32);
    id.push_str("receipt_");
    for byte in &digest[..16] {
        push_hex_byte(&mut id, *byte);
    }
    id
}

fn push_hex_byte(output: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(HEX[(byte >> 4) as usize] as char);
    output.push(HEX[(byte & 0x0f) as usize] as char);
}

fn current_time_ms() -> Result<u64, StoreError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::ReceiptClockBeforeUnixEpoch)?;
    elapsed
        .as_millis()
        .try_into()
        .map_err(|_| StoreError::ReceiptTimestampOutOfRange)
}
