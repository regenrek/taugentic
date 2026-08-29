use ta_protocol::wire::{CodeHostAccount, CodeHostAccountId, CodeHostProviderKind};

use super::*;
use crate::{CodeHostAccountProjection, CodeHostAccountRepository};

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
fn code_host_accounts_are_sorted_by_owner_provider_and_name() {
    let mut store = InMemoryStore::current();
    for account in [
        projection("account-3", "principal-b", "Alpha"),
        projection("account-2", "principal-a", "Zulu"),
        projection("account-1", "principal-a", "Alpha"),
    ] {
        store
            .save_code_host_account(account)
            .expect("account should save");
    }
    let ordered = store.code_host_accounts().expect("accounts should load");
    assert_eq!(
        ordered
            .iter()
            .map(|account| account.account.id.as_str())
            .collect::<Vec<_>>(),
        ["account-1", "account-2", "account-3"]
    );
}

#[test]
fn code_host_account_save_replaces_exact_id_and_remove_is_explicit() {
    let mut store = InMemoryStore::current();
    let id = CodeHostAccountId::new("account-one").expect("account id");
    store
        .save_code_host_account(projection("account-one", "principal-a", "First"))
        .expect("account should save");
    store
        .save_code_host_account(projection("account-one", "principal-a", "Renamed"))
        .expect("account should replace");
    assert_eq!(
        store
            .code_host_account(&id)
            .expect("account should load")
            .expect("account should exist")
            .account
            .display_name,
        "Renamed"
    );
    assert!(
        store
            .remove_code_host_account(&id)
            .expect("remove should work")
    );
    assert!(
        !store
            .remove_code_host_account(&id)
            .expect("second remove should work")
    );
}
