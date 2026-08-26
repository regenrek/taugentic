use ta_protocol::wire::AuthProfileId;

use crate::{AuthProfileProjection, AuthProfileRepository, InMemoryStore, StoreError};

impl AuthProfileRepository for InMemoryStore {
    fn auth_profile(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<Option<AuthProfileProjection>, StoreError> {
        Ok(self.auth_profiles.get(auth_profile_id).cloned())
    }

    fn auth_profiles(&self) -> Result<Vec<AuthProfileProjection>, StoreError> {
        let mut profiles = self.auth_profiles.values().cloned().collect::<Vec<_>>();
        profiles.sort_by(|left, right| {
            left.profile
                .profile
                .provider_id
                .cmp(&right.profile.profile.provider_id)
                .then_with(|| left.auth_method_id().cmp(right.auth_method_id()))
                .then_with(|| left.order.cmp(&right.order))
                .then_with(|| left.id().cmp(right.id()))
        });
        Ok(profiles)
    }

    fn save_auth_profile(&mut self, profile: AuthProfileProjection) -> Result<(), StoreError> {
        self.auth_profiles.insert(profile.id().clone(), profile);
        Ok(())
    }

    fn remove_auth_profile(&mut self, auth_profile_id: &AuthProfileId) -> Result<bool, StoreError> {
        Ok(self.auth_profiles.remove(auth_profile_id).is_some())
    }
}
