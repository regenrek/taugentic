use super::*;

impl SqliteStore {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn save_seed_principal(
        &mut self,
        principal: PrincipalProjection,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO principals (id, credential_hash, data_json) VALUES (?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET credential_hash = excluded.credential_hash, data_json = excluded.data_json",
                params![
                    principal.id.as_str(),
                    principal.credential_hash.as_str(),
                    Self::encode("principal_projection", &principal)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "principal",
                source,
            })?;
        Ok(())
    }
}

impl PrincipalRepository for SqliteStore {
    fn principal_by_credential_hash(
        &self,
        credential_hash: &str,
    ) -> Result<Option<PrincipalProjection>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM principals WHERE credential_hash = ?",
                [credential_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "principal_projection",
                source,
            })?;
        json.map(|json| Self::decode("principal_projection", json))
            .transpose()
    }

    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO principals (id, credential_hash, data_json) VALUES (?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET credential_hash = excluded.credential_hash, data_json = excluded.data_json",
                params![
                    principal.id.as_str(),
                    principal.credential_hash.as_str(),
                    Self::encode("principal_projection", &principal)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "principal_projection",
                source,
            })?;
        Ok(())
    }
}
