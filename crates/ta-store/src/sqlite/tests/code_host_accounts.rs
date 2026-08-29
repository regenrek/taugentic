use rusqlite::Connection;
use ta_protocol::wire::{CodeHostAccount, CodeHostAccountId, CodeHostProviderKind};

use super::*;
use crate::{CodeHostAccountProjection, CodeHostAccountRepository, PrincipalRepository};

fn projection(id: &str, owner: &str, display_name: &str) -> CodeHostAccountProjection {
    CodeHostAccountProjection {
        owner_principal_id: owner.to_string(),
        account: CodeHostAccount {
            id: CodeHostAccountId::new(id).expect("account id"),
            provider: CodeHostProviderKind::GitHub,
            display_name: display_name.to_string(),
            account_login: format!("login-{id}"),
            host: "github.com".to_string(),
        },
    }
}

#[test]
fn code_host_account_metadata_survives_reopen_without_secret_columns() {
    let path = test_db_path("code-host-account");
    let mut store = SqliteStore::open(&path).expect("store should open");
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: "principal-one".to_string(),
            client_name: "sqlite-tests".to_string(),
            credential_hash: "credential-hash-code-host".to_string(),
        },
    )
    .expect("principal should save");
    let expected = projection("account-one", "principal-one", "Work profile");
    store
        .save_code_host_account(expected.clone())
        .expect("account should save");
    drop(store);

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    assert_eq!(
        reopened
            .code_host_account(expected.id())
            .expect("account should load"),
        Some(expected)
    );
    drop(reopened);

    let connection = Connection::open(&path).expect("database should open");
    let mut statement = connection
        .prepare("PRAGMA table_info(code_host_accounts)")
        .expect("schema should inspect");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("columns should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns should decode");
    assert_eq!(
        columns,
        [
            "id",
            "owner_principal_id",
            "provider",
            "display_name",
            "data_json"
        ]
    );
    assert!(
        !columns
            .iter()
            .any(|column| column.contains("token") || column.contains("secret"))
    );
    drop(statement);
    drop(connection);
    let _ = std::fs::remove_file(path);
}

#[test]
fn code_host_account_foreign_key_rejects_unknown_principal() {
    let path = test_db_path("code-host-account-owner");
    let mut store = SqliteStore::open(&path).expect("store should open");
    assert!(
        store
            .save_code_host_account(projection("account-one", "missing-principal", "Profile"))
            .is_err()
    );
    let _ = std::fs::remove_file(path);
}
