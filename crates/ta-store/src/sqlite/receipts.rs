use super::*;
use rusqlite::{params_from_iter, types::Value};

impl ReceiptRepository for SqliteStore {
    fn create(&mut self, input: CreateReceipt) -> Result<ContextReceipt, StoreError> {
        let receipt = build_returned_receipt(input)?;
        if let Some(unique_key) = receipt_unique_key(&receipt)?
            && let Some(existing) = self.receipt_by_unique_key(&receipt, &unique_key)?
        {
            return Ok(existing);
        }

        self.insert_receipt(&receipt)?;
        Ok(receipt)
    }

    fn promote(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        let receipt = self.receipt_or_missing(receipt_id)?;
        let promoted = apply_promote(receipt.clone())?;
        if promoted == receipt {
            return Ok(promoted);
        }
        self.update_receipt_state(&promoted)?;
        Ok(promoted)
    }

    fn quarantine(&mut self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        let receipt = self.receipt_or_missing(receipt_id)?;
        let quarantined = apply_quarantine(receipt.clone())?;
        if quarantined == receipt {
            return Ok(quarantined);
        }
        self.update_receipt_state(&quarantined)?;
        Ok(quarantined)
    }

    fn receipt(&self, receipt_id: &ReceiptId) -> Result<Option<ContextReceipt>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM context_receipts WHERE id = ?",
                [receipt_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        json.map(|json| Self::decode("context_receipt", json))
            .transpose()
    }

    fn list(&self, query: &ReceiptListQuery) -> Result<Vec<ContextReceipt>, StoreError> {
        let mut sql = String::from("SELECT data_json FROM context_receipts WHERE session_id = ?");
        let mut values = vec![Value::Text(query.session_id.as_str().to_string())];
        if let Some(run_id) = query.run_id.as_ref() {
            sql.push_str(" AND run_id = ?");
            values.push(Value::Text(run_id.as_str().to_string()));
        }
        if let Some(state) = query.state {
            sql.push_str(" AND state = ?");
            values.push(Value::Text(receipt_state_storage(state).to_string()));
        }
        if let Some(kind) = query.kind {
            sql.push_str(" AND kind = ?");
            values.push(Value::Text(receipt_kind_storage(kind).to_string()));
        }
        if let Some(parent_run_id) = query.parent_run_id.as_ref() {
            sql.push_str(" AND parent_run_id = ?");
            values.push(Value::Text(parent_run_id.as_str().to_string()));
        }
        sql.push_str(" ORDER BY id ASC");
        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT ?");
            values.push(Value::Integer(limit as i64));
        }

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        let rows = stmt
            .query_map(params_from_iter(values), |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("context_receipt", json))
            .collect()
    }
}

impl SqliteStore {
    fn insert_receipt(&self, receipt: &ContextReceipt) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO context_receipts (
                    id,
                    session_id,
                    run_id,
                    parent_run_id,
                    state,
                    kind,
                    provenance_json,
                    data_json,
                    created_at_ms,
                    promoted_at_ms,
                    quarantined_at_ms,
                    last_commit_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
                params![
                    receipt.id.as_str(),
                    receipt.session_id.as_str(),
                    receipt.run_id.as_str(),
                    receipt.parent_run_id.as_ref().map(|run_id| run_id.as_str()),
                    receipt_state_storage(receipt.state),
                    receipt_kind_storage(receipt.kind),
                    Self::encode("receipt_provenance", &receipt.provenance)?,
                    Self::encode("context_receipt", receipt)?,
                    ms_to_i64(receipt.created_at_ms)?,
                    receipt.promoted_at_ms.map(ms_to_i64).transpose()?,
                    receipt.quarantined_at_ms.map(ms_to_i64).transpose()?,
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        Ok(())
    }

    fn update_receipt_state(&self, receipt: &ContextReceipt) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE context_receipts
                 SET state = ?,
                     data_json = ?,
                     promoted_at_ms = ?,
                     quarantined_at_ms = ?
                 WHERE id = ?",
                params![
                    receipt_state_storage(receipt.state),
                    Self::encode("context_receipt", receipt)?,
                    receipt.promoted_at_ms.map(ms_to_i64).transpose()?,
                    receipt.quarantined_at_ms.map(ms_to_i64).transpose()?,
                    receipt.id.as_str(),
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        Ok(())
    }

    fn receipt_or_missing(&self, receipt_id: &ReceiptId) -> Result<ContextReceipt, StoreError> {
        self.receipt(receipt_id)?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "context_receipt",
                key: receipt_id.clone(),
            })
    }

    fn receipt_by_unique_key(
        &self,
        receipt: &ContextReceipt,
        unique_key: &str,
    ) -> Result<Option<ContextReceipt>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM context_receipts
                 WHERE session_id = ? AND run_id = ? AND kind = ?
                 ORDER BY id ASC",
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        let rows = stmt
            .query_map(
                params![
                    receipt.session_id.as_str(),
                    receipt.run_id.as_str(),
                    receipt_kind_storage(receipt.kind),
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;

        for row in rows {
            let existing = Self::decode(
                "context_receipt",
                row.map_err(|source| StoreError::QueryStore {
                    entity: "context_receipt",
                    source,
                })?,
            )?;
            if receipt_unique_key(&existing)?.as_deref() == Some(unique_key) {
                return Ok(Some(existing));
            }
        }

        Ok(None)
    }
}

fn ms_to_i64(value: u64) -> Result<i64, StoreError> {
    value
        .try_into()
        .map_err(|_| StoreError::ReceiptTimestampOutOfRange)
}
