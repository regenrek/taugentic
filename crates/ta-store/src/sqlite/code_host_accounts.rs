use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::{CodeHostAccountId, CodeHostProviderKind};

use crate::{CodeHostAccountProjection, CodeHostAccountRepository, SqliteStore, StoreError};

impl CodeHostAccountRepository for SqliteStore {
    fn code_host_account(
        &self,
        account_id: &CodeHostAccountId,
    ) -> Result<Option<CodeHostAccountProjection>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM code_host_accounts WHERE id = ?1",
                params![account_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })?
            .map(|value| Self::decode("code host account", value))
            .transpose()
    }

    fn code_host_accounts(&self) -> Result<Vec<CodeHostAccountProjection>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT data_json FROM code_host_accounts \
                 ORDER BY owner_principal_id, provider, display_name, id",
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })?;
        rows.map(|row| {
            row.map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })
            .and_then(|value| Self::decode("code host account", value))
        })
        .collect()
    }

    fn save_code_host_account(
        &mut self,
        account: CodeHostAccountProjection,
    ) -> Result<(), StoreError> {
        let provider = match account.account.provider {
            CodeHostProviderKind::GitHub => "github",
        };
        let data_json = Self::encode("code host account", &account)?;
        self.conn
            .execute(
                "INSERT INTO code_host_accounts \
                 (id, owner_principal_id, provider, display_name, data_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET \
                 owner_principal_id = excluded.owner_principal_id, \
                 provider = excluded.provider, \
                 display_name = excluded.display_name, \
                 data_json = excluded.data_json",
                params![
                    account.id().as_str(),
                    account.owner_principal_id,
                    provider,
                    account.account.display_name,
                    data_json
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })?;
        Ok(())
    }

    fn remove_code_host_account(
        &mut self,
        account_id: &CodeHostAccountId,
    ) -> Result<bool, StoreError> {
        self.conn
            .execute(
                "DELETE FROM code_host_accounts WHERE id = ?1",
                params![account_id.as_str()],
            )
            .map(|changed| changed == 1)
            .map_err(|source| StoreError::QueryStore {
                entity: "code host account",
                source,
            })
    }
}
