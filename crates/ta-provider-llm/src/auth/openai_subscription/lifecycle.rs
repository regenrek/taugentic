use std::sync::{Arc, Weak};

use ta_auth_openai::{TokenLifecycleEvent, TokenManager, TokenRefreshError};
use tokio_util::sync::CancellationToken;

use super::{OpenAiSubscriptionAuth, OpenAiSubscriptionAuthInner, profile};

pub(crate) async fn listen(
    manager: Arc<TokenManager>,
    auth: Weak<OpenAiSubscriptionAuthInner>,
    shutdown_token: CancellationToken,
) {
    let mut receiver = manager.subscribe();
    loop {
        let event = tokio::select! {
            () = shutdown_token.cancelled() => return,
            event = receiver.recv() => event,
        };
        match event {
            Ok(event) => {
                let Some(inner) = auth.upgrade() else {
                    tracing::debug!("auth dropped, exiting lifecycle listener");
                    return;
                };
                apply_event(&OpenAiSubscriptionAuth { inner }, event);
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

pub(crate) fn record_refresh_error(auth: &OpenAiSubscriptionAuth, error: &TokenRefreshError) {
    match error {
        TokenRefreshError::AuthRevoked | TokenRefreshError::MaxRetriesExceeded { .. } => {
            profile::record_needs_reauth(auth, error.to_string());
        }
        TokenRefreshError::NoCredentials => {
            profile::record_logged_out(auth);
        }
        other => profile::record_refresh_failed(auth, other.to_string()),
    }
}

fn apply_event(auth: &OpenAiSubscriptionAuth, event: TokenLifecycleEvent) {
    match event {
        TokenLifecycleEvent::Refreshed { key, account } if key == *auth.key() => {
            profile::record_connected(auth, account);
        }
        TokenLifecycleEvent::RefreshFailed { key, error } if key == *auth.key() => {
            profile::record_refresh_failed(auth, error.to_string());
        }
        TokenLifecycleEvent::LoginFailed { key, reason } if key == *auth.key() => {
            profile::record_login_failed(auth, reason);
        }
        TokenLifecycleEvent::NeedsReauth { key, reason } if key == *auth.key() => {
            profile::record_needs_reauth(auth, reason.to_string());
        }
        TokenLifecycleEvent::LoggedOut { key } if key == *auth.key() => {
            profile::record_logged_out(auth);
        }
        _ => {}
    }
}
