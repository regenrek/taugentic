use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::{AuthProfileId, AuthProfilePreferences};

use crate::{AuthProfileProjection, AuthProfileRepository, SqliteStore, StoreError};

impl AuthProfileRepository for SqliteStore {
    fn auth_profile(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<Option<AuthProfileProjection>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM auth_profiles WHERE id = ?1",
                params![auth_profile_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?
            .map(|value| Self::decode("auth profile", value))
            .transpose()
    }

    fn auth_profiles(&self) -> Result<Vec<AuthProfileProjection>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT data_json FROM auth_profiles \
                 ORDER BY provider_id, auth_method_id, sort_order, id",
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        rows.map(|row| {
            row.map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
            .and_then(|value| Self::decode("auth profile", value))
        })
        .collect()
    }

    fn save_auth_profile(&mut self, profile: AuthProfileProjection) -> Result<(), StoreError> {
        let data_json = Self::encode("auth profile", &profile)?;
        self.conn
            .execute(
                "INSERT INTO auth_profiles \
                 (id, auth_method_id, provider_id, sort_order, is_default, data_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET \
                 auth_method_id = excluded.auth_method_id, \
                 provider_id = excluded.provider_id, \
                 sort_order = excluded.sort_order, \
                 is_default = excluded.is_default, \
                 data_json = excluded.data_json",
                params![
                    profile.id().as_str(),
                    profile.auth_method_id().as_str(),
                    profile.profile.profile.provider_id.as_str(),
                    i64::from(profile.profile.preferences.order),
                    profile.profile.preferences.is_default,
                    data_json,
                ],
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    fn replace_auth_profile_preferences(
        &mut self,
        auth_profile_id: &AuthProfileId,
        preferences: AuthProfilePreferences,
    ) -> Result<(), StoreError> {
        let target =
            self.auth_profile(auth_profile_id)?
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "auth profile",
                    key: auth_profile_id.as_str().to_string(),
                })?;
        let provider_id = target.profile.profile.provider_id.clone();
        let auth_method_id = target.profile.profile.auth_method_id.clone();
        let mut group = self
            .auth_profiles()?
            .into_iter()
            .filter(|profile| {
                profile.profile.profile.provider_id == provider_id
                    && profile.profile.profile.auth_method_id == auth_method_id
            })
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
        let transaction = self
            .conn
            .transaction()
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        for profile in group {
            let data_json = Self::encode("auth profile", &profile)?;
            transaction.execute(
                "UPDATE auth_profiles SET sort_order = ?2, is_default = ?3, data_json = ?4 WHERE id = ?1",
                params![profile.id().as_str(), i64::from(profile.profile.preferences.order), profile.profile.preferences.is_default, data_json],
            ).map_err(|source| StoreError::PrepareStore { path: self.path.clone(), source })?;
        }
        transaction
            .commit()
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    fn remove_auth_profile(&mut self, auth_profile_id: &AuthProfileId) -> Result<bool, StoreError> {
        self.conn
            .execute(
                "DELETE FROM auth_profiles WHERE id = ?1",
                params![auth_profile_id.as_str()],
            )
            .map(|changed| changed == 1)
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
    }
}
