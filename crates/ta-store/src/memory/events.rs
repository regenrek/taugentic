use super::*;

impl EventLogRepository for InMemoryStore {
    fn events(&self) -> Result<Vec<EventRecord>, StoreError> {
        Ok(self.events.values().cloned().collect())
    }

    fn events_tail_desc(&self, limit: usize) -> Result<Vec<EventRecord>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut records: Vec<EventRecord> = self.events.values().cloned().collect();
        records.sort_by_key(|record| record.sequence);
        records.reverse();
        records.truncate(limit);
        Ok(records)
    }

    fn events_for_session(&self, session_id: &SessionId) -> Result<Vec<EventRecord>, StoreError> {
        Ok(self
            .events
            .values()
            .filter(|event| event.session_id == *session_id)
            .cloned()
            .collect())
    }

    fn approvals_for_session(
        &self,
        query: &SessionApprovalQuery,
    ) -> Result<Vec<ApprovalRequest>, StoreError> {
        let lifecycle = ApprovalLifecycleState::fold_session_records(
            self.events
                .values()
                .filter(|record| record.session_id == query.session_id),
        )?;
        Ok(lifecycle.approvals_for_query(query))
    }

    fn approval_lookup(
        &self,
        session_id: &SessionId,
        approval_id: &ApprovalId,
    ) -> Result<crate::SessionApprovalLookup, StoreError> {
        let lifecycle = ApprovalLifecycleState::fold_session_records(
            self.events
                .values()
                .filter(|record| record.session_id == *session_id),
        )?;
        Ok(lifecycle.lookup(approval_id))
    }

    fn session_event_page(
        &self,
        query: &SessionEventPageQuery,
    ) -> Result<SessionEventPage, StoreError> {
        let mut latest_sequence = None;
        let mut records = Vec::with_capacity(query.limit);
        let mut has_more = false;

        for record in self.events.values().rev() {
            if record.session_id != query.session_id {
                continue;
            }
            if !query.kinds.is_empty() && !query.kinds.contains(&record.payload.kind()) {
                continue;
            }

            latest_sequence.get_or_insert(record.sequence);

            if query
                .before_sequence
                .is_some_and(|before_sequence| record.sequence >= before_sequence)
            {
                continue;
            }

            if records.len() < query.limit {
                records.push(record.clone());
                continue;
            }

            has_more = true;
            break;
        }

        Ok(SessionEventPage {
            next_before_sequence: has_more
                .then(|| records.last().map(|record| record.sequence))
                .flatten(),
            latest_sequence,
            records,
        })
    }

    fn session_event_range(
        &self,
        query: &SessionEventRangeQuery,
    ) -> Result<SessionEventRange, StoreError> {
        let mut latest_sequence = None;
        let mut records = Vec::new();

        for record in self.events.values() {
            if record.session_id != query.session_id {
                continue;
            }
            if !query.kinds.is_empty() && !query.kinds.contains(&record.payload.kind()) {
                continue;
            }

            latest_sequence = Some(record.sequence);

            if query
                .after_sequence
                .is_some_and(|after_sequence| record.sequence <= after_sequence)
            {
                continue;
            }

            if query
                .up_to_sequence
                .is_some_and(|up_to_sequence| record.sequence > up_to_sequence)
            {
                continue;
            }

            records.push(record.clone());
        }

        Ok(SessionEventRange {
            records,
            latest_sequence,
        })
    }

    fn read_run_events(&self, query: &RunEventRangeQuery) -> Result<RunEventRange, StoreError> {
        Ok(run_event_range_from_records(
            self.events.values().cloned(),
            query,
        ))
    }

    fn session_agent_turns_page(
        &self,
        query: &SessionAgentTurnsPageQuery,
    ) -> Result<SessionAgentTurnsPage, StoreError> {
        let mut latest_activity_sequence = None;
        for record in self.events.values().rev() {
            if record.session_id == query.session_id {
                latest_activity_sequence = Some(record.sequence);
                break;
            }
        }

        let mut rows = Vec::with_capacity(query.limit);
        let mut has_more = false;
        for row in self.agent_turn_rows.values().rev() {
            if row_session_id(row) != &query.session_id {
                continue;
            }
            let sequence = row_sequence(row);
            if query
                .before_sequence
                .is_some_and(|before_sequence| sequence >= before_sequence)
            {
                continue;
            }
            if rows.len() == query.limit {
                has_more = true;
                break;
            }
            rows.push(row.clone());
        }

        let next_before_sequence = if has_more {
            rows.last().map(row_sequence)
        } else {
            None
        };

        Ok(SessionAgentTurnsPage {
            rows,
            next_before_sequence,
            latest_activity_sequence,
        })
    }
}
