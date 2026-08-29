use ta_protocol::wire::{AuthProfileId, AuthProfilePreferences};

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
                .then_with(|| {
                    left.profile
                        .preferences
                        .order
                        .cmp(&right.profile.preferences.order)
                })
                .then_with(|| left.id().cmp(right.id()))
        });
        Ok(profiles)
    }

    fn save_auth_profile(&mut self, profile: AuthProfileProjection) -> Result<(), StoreError> {
        self.auth_profiles.insert(profile.id().clone(), profile);
        Ok(())
    }

    fn replace_auth_profile_preferences(
        &mut self,
        auth_profile_id: &AuthProfileId,
        preferences: AuthProfilePreferences,
    ) -> Result<(), StoreError> {
        let target = self
            .auth_profiles
            .get(auth_profile_id)
            .cloned()
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "auth profile",
                key: auth_profile_id.as_str().to_string(),
            })?;
        let provider_id = target.profile.profile.provider_id.clone();
        let auth_method_id = target.profile.profile.auth_method_id.clone();
        let mut group = self
            .auth_profiles
            .values()
            .filter(|profile| {
                profile.profile.profile.provider_id == provider_id
                    && profile.profile.profile.auth_method_id == auth_method_id
            })
            .cloned()
            .collect::<Vec<_>>();
        group.sort_by(|left, right| {
            left.profile
                .preferences
                .order
                .cmp(&right.profile.preferences.order)
                .then_with(|| left.id().cmp(right.id()))
        });
        if preferences.order as usize >= group.len() {
            return Err(StoreError::AuthProfilePreferenceOrderOutOfRange {
                order: preferences.order,
                group_len: group.len(),
            });
        }
        let target_index = group
            .iter()
            .position(|profile| profile.id() == auth_profile_id)
            .expect("target is in its group");
        let mut target = group.remove(target_index);
        target.profile.preferences.label = preferences.label;
        target.profile.preferences.is_default = preferences.is_default;
        group.insert(preferences.order as usize, target);
        for (order, profile) in group.iter_mut().enumerate() {
            profile.profile.preferences.order = order as u32;
            if preferences.is_default {
                profile.profile.preferences.is_default = profile.id() == auth_profile_id;
            } else if profile.id() == auth_profile_id {
                profile.profile.preferences.is_default = false;
            }
        }
        for profile in group {
            self.auth_profiles.insert(profile.id().clone(), profile);
        }
        Ok(())
    }

    fn remove_auth_profile(&mut self, auth_profile_id: &AuthProfileId) -> Result<bool, StoreError> {
        Ok(self.auth_profiles.remove(auth_profile_id).is_some())
    }
}
