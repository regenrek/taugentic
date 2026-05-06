use std::time::{SystemTime, UNIX_EPOCH};

use ta_store::{CommitReceiptEvent, ReceiptListQuery, StoreError};

use crate::{
    AppDeferredMutationResult, AppService, AppServiceError, CONTEXT_RECEIPT_LIST_MAX_LIMIT,
    ContextReceipt, ContextReceiptEvent, DaemonEvent, ListReceiptsRequest, ListReceiptsResult,
    PromoteReceiptRequest, QuarantineReceiptRequest,
};

use super::errors::map_receipt_store_error;

impl<S> AppService<S>
where
    S: ta_store::PersistenceStore + Send,
{
    pub fn list_receipts(
        &self,
        session_id: &crate::SessionId,
        request: &ListReceiptsRequest,
    ) -> Result<ListReceiptsResult, AppServiceError> {
        ensure_receipt_request_session(session_id, &request.session_id)?;
        let limit = receipt_list_limit(request.limit)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        let receipts = store.list(&ReceiptListQuery {
            session_id: session_id.clone(),
            run_id: request.run_id.clone(),
            state: request.state,
            kind: request.kind,
            parent_run_id: request.parent_run_id.clone(),
            limit: Some(limit),
        })?;
        Ok(ListReceiptsResult { receipts })
    }

    pub fn promote_receipt(
        &self,
        session_id: &crate::SessionId,
        request: &PromoteReceiptRequest,
    ) -> Result<AppDeferredMutationResult<ContextReceipt>, AppServiceError> {
        ensure_receipt_request_session(session_id, &request.session_id)?;
        self.transition_receipt(session_id, &request.receipt_id, ReceiptTransition::Promote)
    }

    pub fn quarantine_receipt(
        &self,
        session_id: &crate::SessionId,
        request: &QuarantineReceiptRequest,
    ) -> Result<AppDeferredMutationResult<ContextReceipt>, AppServiceError> {
        ensure_receipt_request_session(session_id, &request.session_id)?;
        self.transition_receipt(
            session_id,
            &request.receipt_id,
            ReceiptTransition::Quarantine,
        )
    }

    fn transition_receipt(
        &self,
        session_id: &crate::SessionId,
        receipt_id: &crate::ReceiptId,
        transition: ReceiptTransition,
    ) -> Result<AppDeferredMutationResult<ContextReceipt>, AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let existing = store
            .receipt(receipt_id)?
            .ok_or_else(|| AppServiceError::ReceiptNotFound(receipt_id.clone()))?;
        if existing.session_id != *session_id {
            return Err(AppServiceError::ReceiptSessionMismatch(receipt_id.clone()));
        }
        let receipt = match transition {
            ReceiptTransition::Promote => {
                store.promote(receipt_id).map_err(map_receipt_store_error)?
            }
            ReceiptTransition::Quarantine => store
                .quarantine(receipt_id)
                .map_err(map_receipt_store_error)?,
        };
        let event = store
            .commit_receipt_event(CommitReceiptEvent {
                session_id: session_id.clone(),
                event: DaemonEvent::ContextReceipt(transition.event(receipt.clone())),
                occurred_at_ms: current_time_ms()?,
            })?
            .event;
        Ok(AppDeferredMutationResult {
            body: receipt,
            deferred_records: vec![event],
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum ReceiptTransition {
    Promote,
    Quarantine,
}

impl ReceiptTransition {
    fn event(self, receipt: ContextReceipt) -> ContextReceiptEvent {
        match self {
            Self::Promote => ContextReceiptEvent::Promoted { receipt },
            Self::Quarantine => ContextReceiptEvent::Quarantined { receipt },
        }
    }
}

fn ensure_receipt_request_session(
    attached_session_id: &crate::SessionId,
    request_session_id: &crate::SessionId,
) -> Result<(), AppServiceError> {
    if attached_session_id == request_session_id {
        Ok(())
    } else {
        Err(AppServiceError::ReceiptSessionMismatch(
            request_session_id.as_str().to_string(),
        ))
    }
}

fn receipt_list_limit(limit: Option<u32>) -> Result<usize, AppServiceError> {
    let limit = limit.unwrap_or(CONTEXT_RECEIPT_LIST_MAX_LIMIT);
    if limit == 0 || limit > CONTEXT_RECEIPT_LIST_MAX_LIMIT {
        return Err(AppServiceError::InvalidReceiptListLimit {
            max: CONTEXT_RECEIPT_LIST_MAX_LIMIT,
        });
    }
    Ok(limit as usize)
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
