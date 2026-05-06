use super::*;

impl PrincipalRepository for InMemoryStore {
    fn principal_by_credential_hash(
        &self,
        credential_hash: &str,
    ) -> Result<Option<PrincipalProjection>, StoreError> {
        Ok(self
            .principals
            .values()
            .find(|principal| principal.credential_hash == credential_hash)
            .cloned())
    }

    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError> {
        self.principals.insert(principal.id.clone(), principal);
        Ok(())
    }
}
