use super::*;

impl SqliteStore {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn save_seed_run(&mut self, run: RunProjection) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO runs (id, session_id, data_json, last_commit_id) VALUES (?, ?, ?, NULL)
                 ON CONFLICT(id) DO UPDATE SET session_id = excluded.session_id, data_json = excluded.data_json",
                params![
                    run.id.as_str(),
                    run.session_id.as_str(),
                    Self::encode("run_projection", &run)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "run",
                source,
            })?;
        Ok(())
    }

    pub(super) fn read_run_projection(
        &self,
        run_id: &RunId,
    ) -> Result<Option<RunProjection>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM runs WHERE id = ?",
                [run_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?;
        json.map(|json| Self::decode("run_projection", json))
            .transpose()
    }

    pub(super) fn read_run_projections(&self) -> Result<Vec<RunProjection>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM runs ORDER BY id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("run_projection", json))
            .collect()
    }

    pub(super) fn read_native_runs(
        &self,
        query: &NativeRunListQuery,
    ) -> Result<NativeRunListPage, StoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT data_json FROM runs
                 WHERE session_id = ?
                 ORDER BY CASE
                              WHEN json_valid(data_json)
                              THEN json_extract(data_json, '$.started_at_ms')
                              ELSE NULL
                          END DESC,
                          id DESC",
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?;
        let rows = stmt
            .query_map([query.session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "run_projection",
                source,
            })?;
        let runs = rows
            .into_iter()
            .map(|json| Self::decode("run_projection", json))
            .collect::<Result<Vec<RunProjection>, _>>()?;
        Ok(list_native_runs_from_projections(runs, query))
    }
}
