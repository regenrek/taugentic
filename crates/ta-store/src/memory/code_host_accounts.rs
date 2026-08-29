use ta_protocol::wire::CodeHostAccountId;

use crate::{CodeHostAccountProjection, CodeHostAccountRepository, InMemoryStore, StoreError};

impl CodeHostAccountRepository for InMemoryStore {
    fn code_host_account(
        &self,
        account_id: &CodeHostAccountId,
    ) -> Result<Option<CodeHostAccountProjection>, StoreError> {
        Ok(self.code_host_accounts.get(account_id).cloned())
    }

    fn code_host_accounts(&self) -> Result<Vec<CodeHostAccountProjection>, StoreError> {
        let mut accounts = self
            .code_host_accounts
            .values()
            .cloned()
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| {
            left.owner_principal_id
                .cmp(&right.owner_principal_id)
                .then_with(|| left.account.provider.cmp(&right.account.provider))
                .then_with(|| left.account.display_name.cmp(&right.account.display_name))
                .then_with(|| left.account.id.cmp(&right.account.id))
        });
        Ok(accounts)
    }

    fn save_code_host_account(
        &mut self,
        account: CodeHostAccountProjection,
    ) -> Result<(), StoreError> {
        self.code_host_accounts
            .insert(account.id().clone(), account);
        Ok(())
    }

    fn remove_code_host_account(
        &mut self,
        account_id: &CodeHostAccountId,
    ) -> Result<bool, StoreError> {
        Ok(self.code_host_accounts.remove(account_id).is_some())
    }
}
