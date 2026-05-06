use super::*;

impl SqliteStore {
    fn all_events(&self) -> Result<Vec<EventRecord>, StoreError> {
        self.query_events(
            "SELECT sequence, session_id, occurred_at_ms, payload_json FROM events ORDER BY sequence ASC",
            [],
        )
    }

    fn events_for_session_result(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<EventRecord>, StoreError> {
        self.query_events(
            "SELECT sequence, session_id, occurred_at_ms, payload_json
             FROM events
             WHERE session_id = ?
             ORDER BY sequence ASC",
            [session_id.as_str()],
        )
    }

    fn run_events_after_sequence(
        &self,
        query: &RunEventRangeQuery,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<EventRecord>, StoreError> {
        self.query_events(
            "SELECT sequence, session_id, occurred_at_ms, payload_json
             FROM events
             WHERE session_id = ? AND sequence > ?
             ORDER BY sequence ASC
             LIMIT ?",
            params![
                query.session_id.as_str(),
                i64::try_from(after_sequence).unwrap_or(i64::MAX),
                i64::try_from(limit).unwrap_or(i64::MAX),
            ],
        )
    }

    fn latest_run_event_sequence(
        &self,
        query: &RunEventRangeQuery,
    ) -> Result<Option<u64>, StoreError> {
        const SCAN_BATCH_LIMIT: usize = 256;

        let mut before_sequence = None;
        loop {
            let batch = if let Some(before_sequence) = before_sequence {
                self.query_events(
                    "SELECT sequence, session_id, occurred_at_ms, payload_json
                     FROM events
                     WHERE session_id = ? AND sequence < ?
                     ORDER BY sequence DESC
                     LIMIT ?",
                    params![
                        query.session_id.as_str(),
                        i64::try_from(before_sequence).unwrap_or(i64::MAX),
                        SCAN_BATCH_LIMIT as i64,
                    ],
                )?
            } else {
                self.query_events(
                    "SELECT sequence, session_id, occurred_at_ms, payload_json
                     FROM events
                     WHERE session_id = ?
                     ORDER BY sequence DESC
                     LIMIT ?",
                    params![query.session_id.as_str(), SCAN_BATCH_LIMIT as i64],
                )?
            };

            if batch.is_empty() {
                return Ok(None);
            }

            for record in &batch {
                if event_run_id(&record.payload) == Some(&query.run_id) {
                    return Ok(Some(record.sequence));
                }
            }

            before_sequence = batch.last().map(|record| record.sequence);
            if batch.len() < SCAN_BATCH_LIMIT {
                return Ok(None);
            }
        }
    }

    fn query_events<P>(&self, sql: &str, params: P) -> Result<Vec<EventRecord>, StoreError>
    where
        P: rusqlite::Params,
    {
        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|source| StoreError::QueryStore {
                entity: "events",
                source,
            })?;
        let rows = stmt
            .query_map(params, |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|source| StoreError::QueryStore {
                entity: "events",
                source,
            })?;

        let mut records = Vec::new();
        for row in rows {
            let (sequence, session_id, occurred_at_ms, payload_json) =
                row.map_err(|source| StoreError::QueryStore {
                    entity: "events",
                    source,
                })?;
            let payload = Self::decode("daemon_event", payload_json)?;
            let session_id =
                SessionId::new(session_id).map_err(|error| StoreError::DecodeRecord {
                    entity: "session_id",
                    source: serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        error.to_string(),
                    )),
                })?;
            records.push(EventRecord {
                sequence: sequence as u64,
                session_id,
                occurred_at_ms: occurred_at_ms as u64,
                payload,
            });
        }
        Ok(records)
    }

    pub(super) fn next_sequence(&self) -> Result<u64, StoreError> {
        let next = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "event_sequence",
                source,
            })?;
        Ok(next as u64)
    }

    pub(super) fn events_for_session_tx(
        tx: &rusqlite::Transaction<'_>,
        session_id: &SessionId,
    ) -> Result<Vec<EventRecord>, StoreError> {
        let mut stmt = tx
            .prepare(
                "SELECT sequence, session_id, occurred_at_ms, payload_json
                 FROM events
                 WHERE session_id = ?
                 ORDER BY sequence ASC",
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "events",
                source,
            })?;
        let rows = stmt
            .query_map([session_id.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|source| StoreError::QueryStore {
                entity: "events",
                source,
            })?;

        let mut records = Vec::new();
        for row in rows {
            let (sequence, session_id, occurred_at_ms, payload_json) =
                row.map_err(|source| StoreError::QueryStore {
                    entity: "events",
                    source,
                })?;
            let payload = Self::decode("daemon_event", payload_json)?;
            let session_id =
                SessionId::new(session_id).map_err(|error| StoreError::DecodeRecord {
                    entity: "session_id",
                    source: serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        error.to_string(),
                    )),
                })?;
            records.push(EventRecord {
                sequence: sequence as u64,
                session_id,
                occurred_at_ms: occurred_at_ms as u64,
                payload,
            });
        }
        Ok(records)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn append_seed_event(&mut self, event: EventRecord) -> Result<(), StoreError> {
        if let Some(row) = apply_agent_stream_event(
            &mut self.in_flight_assistant_turns,
            &mut self.in_flight_tool_calls,
            &event,
        )? {
            self.conn
                .execute(
                    "INSERT INTO agent_turn_rows (sequence, session_id, data_json) VALUES (?, ?, ?)",
                    params![
                        row_sequence(&row) as i64,
                        row_session_id(&row).as_str(),
                        Self::encode("agent_turn_row", &row)?
                    ],
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "agent_turn_row",
                    source,
                })?;
        }
        let changed = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO events (sequence, session_id, occurred_at_ms, payload_json)
                 VALUES (?, ?, ?, ?)",
                params![
                    event.sequence as i64,
                    event.session_id.as_str(),
                    event.occurred_at_ms as i64,
                    Self::encode("daemon_event", &event.payload)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "event",
                source,
            })?;
        if changed == 0 {
            return Err(StoreError::DuplicateRecord {
                entity: "event",
                key: event.sequence.to_string(),
            });
        }
        self.next_runtime_sequence = self
            .next_runtime_sequence
            .max(event.sequence.saturating_add(1));
        Ok(())
    }
}

impl EventLogRepository for SqliteStore {
    fn events(&self) -> Result<Vec<EventRecord>, StoreError> {
        self.all_events()
    }

    fn events_tail_desc(&self, limit: usize) -> Result<Vec<EventRecord>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let capped = limit.min(8192);
        self.query_events(
            "SELECT sequence, session_id, occurred_at_ms, payload_json
             FROM events
             ORDER BY sequence DESC
             LIMIT ?",
            params![i64::try_from(capped).unwrap_or(i64::MAX)],
        )
    }

    fn events_for_session(&self, session_id: &SessionId) -> Result<Vec<EventRecord>, StoreError> {
        self.events_for_session_result(session_id)
    }

    fn approvals_for_session(
        &self,
        query: &SessionApprovalQuery,
    ) -> Result<Vec<ApprovalRequest>, StoreError> {
        let events = self.events_for_session(&query.session_id)?;
        let lifecycle = ApprovalLifecycleState::fold_session_records(events.iter())?;
        Ok(lifecycle.approvals_for_query(query))
    }

    fn approval_lookup(
        &self,
        session_id: &SessionId,
        approval_id: &ta_protocol::wire::ApprovalId,
    ) -> Result<crate::SessionApprovalLookup, StoreError> {
        let events = self.events_for_session(session_id)?;
        let lifecycle = ApprovalLifecycleState::fold_session_records(events.iter())?;
        Ok(lifecycle.lookup(approval_id))
    }

    fn session_event_page(
        &self,
        query: &SessionEventPageQuery,
    ) -> Result<SessionEventPage, StoreError> {
        let mut latest_sequence = None;
        let mut records = Vec::with_capacity(query.limit);
        let mut has_more = false;
        for record in self
            .events_for_session(&query.session_id)?
            .into_iter()
            .rev()
        {
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
                records.push(record);
            } else {
                has_more = true;
                break;
            }
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
        for record in self.events_for_session(&query.session_id)? {
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
            records.push(record);
        }
        Ok(SessionEventRange {
            records,
            latest_sequence,
        })
    }

    fn read_run_events(&self, query: &RunEventRangeQuery) -> Result<RunEventRange, StoreError> {
        const REPLAY_BATCH_SCAN_LIMIT: usize = 256;

        let latest_sequence = self.latest_run_event_sequence(query)?;
        if query.limit == 0 {
            return Ok(RunEventRange {
                records: Vec::new(),
                latest_sequence,
            });
        }

        let mut after_sequence = query.after_sequence.unwrap_or(0);
        let mut records = Vec::with_capacity(query.limit.min(1024));
        while records.len() < query.limit {
            let batch_limit = REPLAY_BATCH_SCAN_LIMIT;
            let batch = self.run_events_after_sequence(query, after_sequence, batch_limit)?;
            let batch_len = batch.len();
            let Some(last) = batch.last() else {
                break;
            };
            after_sequence = last.sequence;

            for record in batch {
                if event_run_id(&record.payload) == Some(&query.run_id) {
                    records.push(record);
                    if records.len() == query.limit {
                        break;
                    }
                }
            }

            if batch_len < batch_limit {
                break;
            }
        }

        Ok(RunEventRange {
            records,
            latest_sequence,
        })
    }

    fn session_agent_turns_page(
        &self,
        query: &SessionAgentTurnsPageQuery,
    ) -> Result<SessionAgentTurnsPage, StoreError> {
        self.session_agent_turns_page_impl(query)
    }
}
