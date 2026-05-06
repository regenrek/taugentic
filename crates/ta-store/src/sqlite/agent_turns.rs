use super::*;

impl SqliteStore {
    pub(super) fn session_agent_turns_page_impl(
        &self,
        query: &SessionAgentTurnsPageQuery,
    ) -> Result<SessionAgentTurnsPage, StoreError> {
        let latest_activity_sequence = self
            .conn
            .query_row(
                "SELECT MAX(sequence) FROM events WHERE session_id = ?",
                params![query.session_id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "agent_turn_row",
                source,
            })?
            .map(|value| value as u64);

        let mut stmt = if query.before_sequence.is_some() {
            self.conn
                .prepare(
                    "SELECT data_json
                     FROM agent_turn_rows
                     WHERE session_id = ? AND sequence < ?
                     ORDER BY sequence DESC
                     LIMIT ?",
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "agent_turn_row",
                    source,
                })?
        } else {
            self.conn
                .prepare(
                    "SELECT data_json
                     FROM agent_turn_rows
                     WHERE session_id = ?
                     ORDER BY sequence DESC
                     LIMIT ?",
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "agent_turn_row",
                    source,
                })?
        };

        let limit_plus_one = query.limit.saturating_add(1) as i64;
        let row_jsons = if let Some(before_sequence) = query.before_sequence {
            stmt.query_map(
                params![
                    query.session_id.as_str(),
                    before_sequence as i64,
                    limit_plus_one
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "agent_turn_row",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "agent_turn_row",
                source,
            })?
        } else {
            stmt.query_map(params![query.session_id.as_str(), limit_plus_one], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|source| StoreError::QueryStore {
                entity: "agent_turn_row",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "agent_turn_row",
                source,
            })?
        };
        let rows = row_jsons
            .into_iter()
            .map(|json| Self::decode("agent_turn_row", json))
            .collect::<Result<Vec<AgentTurnRow>, _>>()?;

        let has_more = rows.len() > query.limit;
        let rows = if has_more {
            rows.into_iter().take(query.limit).collect::<Vec<_>>()
        } else {
            rows
        };

        Ok(SessionAgentTurnsPage {
            next_before_sequence: has_more.then(|| rows.last().map(row_sequence)).flatten(),
            latest_activity_sequence,
            rows,
        })
    }
}
