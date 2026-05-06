use super::*;

impl SessionAuthorityRepository for InMemoryStore {
    fn rotate_session_authority(
        &mut self,
        session_id: &SessionId,
        owner_principal_id: &str,
        presented_authority_hash: &str,
        next_authority_hash: &str,
    ) -> Result<Option<SessionProjection>, StoreError> {
        let Some(existing) = self.sessions.get(session_id).cloned() else {
            return Ok(None);
        };
        if existing.owner_principal_id != owner_principal_id {
            return Ok(None);
        }
        let matches_current = existing.current_session_authority_hash == presented_authority_hash;
        let matches_recovery = existing
            .recovery_session_authority_hash
            .as_deref()
            .is_some_and(|hash| hash == presented_authority_hash);
        if !matches_current && !matches_recovery {
            return Ok(None);
        }
        let next_generation = existing
            .current_session_authority_generation
            .saturating_add(1);
        let recovery_session_authority_hash = if matches_current {
            Some(existing.current_session_authority_hash.clone())
        } else {
            None
        };
        let recovery_session_authority_generation = if matches_current {
            Some(existing.current_session_authority_generation)
        } else {
            None
        };

        let updated = SessionProjection {
            current_session_authority_hash: next_authority_hash.to_string(),
            current_session_authority_generation: next_generation,
            recovery_session_authority_hash,
            recovery_session_authority_generation,
            ..existing
        };
        self.sessions.insert(session_id.clone(), updated.clone());
        Ok(Some(updated))
    }
}
