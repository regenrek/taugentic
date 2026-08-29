use std::{fmt, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ta_host_platform::{HostSecretKey, HostSecretStore, HostSecretValue};
use ta_protocol::wire::{CodeHostAccountId, CodeHostProviderKind};

use crate::CodeHostError;

const CODE_HOST_SECRET_SERVICE_NAME: &str = "taugentic.code-host.credentials";

#[derive(Clone, PartialEq, Eq)]
pub struct CodeHostAccessToken(HostSecretValue);

impl CodeHostAccessToken {
    pub fn new(value: impl Into<String>) -> Result<Self, CodeHostError> {
        HostSecretValue::new(value)
            .map(Self)
            .map_err(|_| CodeHostError::CredentialsMissing)
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for CodeHostAccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CodeHostAccessToken([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GitHttpsAuthorization {
    origin: String,
    extra_header: HostSecretValue,
}

impl GitHttpsAuthorization {
    pub fn github(
        origin: &str,
        account_login: &str,
        token: &CodeHostAccessToken,
    ) -> Result<Self, CodeHostError> {
        let origin = normalize_origin(origin)?;
        let account_login = normalize_account_login(account_login)?;
        let encoded = STANDARD.encode(format!("{account_login}:{}", token.expose_secret()));
        let extra_header = HostSecretValue::new(format!("AUTHORIZATION: basic {encoded}"))
            .map_err(|_| CodeHostError::CredentialsMissing)?;
        Ok(Self {
            origin,
            extra_header,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn expose_extra_header(&self) -> &str {
        self.extra_header.expose_secret()
    }
}

fn normalize_account_login(value: &str) -> Result<&str, CodeHostError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(CodeHostError::InvalidInput);
    }
    Ok(value)
}

impl fmt::Debug for GitHttpsAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitHttpsAuthorization")
            .field("origin", &self.origin)
            .field("extra_header", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct CodeHostCredentialStore {
    store: Arc<dyn HostSecretStore>,
}

impl CodeHostCredentialStore {
    pub fn new(store: Arc<dyn HostSecretStore>) -> Self {
        Self { store }
    }

    pub fn from_default_store() -> Result<Self, CodeHostError> {
        ta_host_platform::default_host_secret_store(CODE_HOST_SECRET_SERVICE_NAME)
            .map(Self::new)
            .map_err(|_| CodeHostError::CredentialsBackend)
    }

    pub fn store(
        &self,
        provider: CodeHostProviderKind,
        account_id: &CodeHostAccountId,
        token: &CodeHostAccessToken,
    ) -> Result<(), CodeHostError> {
        self.store
            .store_secret(&secret_key(provider, account_id)?, &token.0)
            .map_err(|_| CodeHostError::CredentialsBackend)
    }

    pub fn load(
        &self,
        provider: CodeHostProviderKind,
        account_id: &CodeHostAccountId,
    ) -> Result<CodeHostAccessToken, CodeHostError> {
        self.store
            .load_secret(&secret_key(provider, account_id)?)
            .map_err(|_| CodeHostError::CredentialsBackend)?
            .map(CodeHostAccessToken)
            .ok_or(CodeHostError::CredentialsMissing)
    }

    pub fn delete(
        &self,
        provider: CodeHostProviderKind,
        account_id: &CodeHostAccountId,
    ) -> Result<(), CodeHostError> {
        self.store
            .delete_secret(&secret_key(provider, account_id)?)
            .map_err(|_| CodeHostError::CredentialsBackend)
    }
}

fn secret_key(
    provider: CodeHostProviderKind,
    account_id: &CodeHostAccountId,
) -> Result<HostSecretKey, CodeHostError> {
    let provider = match provider {
        CodeHostProviderKind::GitHub => "github",
    };
    HostSecretKey::new(format!("{provider}/{}/access_token", account_id.as_str()))
        .map_err(|_| CodeHostError::InvalidInput)
}

fn normalize_origin(value: &str) -> Result<String, CodeHostError> {
    let parsed = url::Url::parse(value).map_err(|_| CodeHostError::InvalidConfig)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CodeHostError::InvalidConfig);
    }
    let host = parsed.host_str().ok_or(CodeHostError::InvalidConfig)?;
    let port = parsed
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    Ok(format!("https://{host}{port}"))
}
