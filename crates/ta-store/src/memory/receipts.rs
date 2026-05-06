use super::*;

impl ReceiptRepository for InMemoryStore {
    fn create(&mut self, input: CreateReceipt) -> Result<ContextReceipt, StoreError> {
        #[cfg(any(test, feature = "test-support"))]
        if self.fail_next_receipt_create {
            self.fail_next_receipt_create = false;
            return Err(StoreError::InvalidProvenance {
                message: "injected receipt create failure".to_string(),
            });
        }

        let receipt = build_returned_receipt(input)?;
        if let Some(unique_key) = receipt_unique_key(&receipt)?
            && let Some(existing) = self.receipts.values().find(|existing| {
                receipt_unique_key(existing).is_ok_and(|key| key.as_ref() == Some(&unique_key))
            })
        {
            return Ok(existing.clone());
        }

        self.receipts.insert(receipt.id.clone(), receipt.clone());
        Ok(receipt)
    }

    fn promote(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        let receipt = self.receipt_or_missing(receipt_id)?;
        let promoted = apply_promote(receipt.clone())?;
        if promoted == receipt {
            return Ok(promoted);
        }
        self.receipts.insert(receipt_id.clone(), promoted.clone());
        Ok(promoted)
    }

    fn quarantine(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        let receipt = self.receipt_or_missing(receipt_id)?;
        let quarantined = apply_quarantine(receipt.clone())?;
        if quarantined == receipt {
            return Ok(quarantined);
        }
        self.receipts
            .insert(receipt_id.clone(), quarantined.clone());
        Ok(quarantined)
    }

    fn receipt(&self, receipt_id: &ReceiptId) -> Result<Option<ContextReceipt>, StoreError> {
        Ok(self.receipts.get(receipt_id).cloned())
    }

    fn list(&self, query: &ReceiptListQuery) -> Result<Vec<ContextReceipt>, StoreError> {
        let Some(limit) = query.limit else {
            return Ok(self
                .receipts
                .values()
                .filter(|receipt| receipt_matches_query(receipt, query))
                .cloned()
                .collect());
        };

        let mut receipts = Vec::with_capacity(limit);
        for receipt in self.receipts.values() {
            if !receipt_matches_query(receipt, query) {
                continue;
            }
            if receipts.len() >= limit {
                break;
            }
            receipts.push(receipt.clone());
        }
        Ok(receipts)
    }
}

impl InMemoryStore {
    fn receipt_or_missing(&self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        self.receipts
            .get(receipt_id)
            .cloned()
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "context_receipt",
                key: receipt_id.clone(),
            })
    }
}
