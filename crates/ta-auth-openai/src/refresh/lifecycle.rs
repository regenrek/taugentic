use tokio::sync::broadcast;

use crate::{AccountInfo, CredentialKey};

use super::TokenRefreshError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenLifecycleEvent {
    Refreshed {
        key: CredentialKey,
        account: AccountInfo,
    },
    RefreshFailed {
        key: CredentialKey,
        error: TokenRefreshError,
    },
    LoginFailed {
        key: CredentialKey,
        reason: String,
    },
    NeedsReauth {
        key: CredentialKey,
        reason: TokenRefreshError,
    },
    LoggedOut {
        key: CredentialKey,
    },
}

#[derive(Clone)]
pub(crate) struct TokenLifecycleBroadcaster {
    sender: broadcast::Sender<TokenLifecycleEvent>,
}

impl TokenLifecycleBroadcaster {
    pub(crate) fn new() -> Self {
        let (sender, _receiver) = broadcast::channel(32);
        Self { sender }
    }

    pub(crate) fn emit(&self, event: TokenLifecycleEvent) {
        let _ = self.sender.send(event);
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<TokenLifecycleEvent> {
        self.sender.subscribe()
    }
}
