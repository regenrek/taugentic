pub const OPENAI_CHATGPT_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const OPENAI_CHATGPT_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const OPENAI_CHATGPT_REVOKE_URL: &str = "https://auth.openai.com/oauth/revoke";
pub const OPENAI_CHATGPT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_CHATGPT_REDIRECT_URI_TEMPLATE: &str = "http://localhost:{port}/auth/callback";
pub const OPENAI_CHATGPT_CALLBACK_PATH: &str = "/auth/callback";
pub const OPENAI_CHATGPT_CALLBACK_PORTS: [u16; 2] = [1455, 1457];
pub const OPENAI_CHATGPT_SCOPES: [&str; 6] = [
    "openid",
    "profile",
    "email",
    "offline_access",
    "api.connectors.read",
    "api.connectors.invoke",
];
// Keep the Codex-owned OAuth app in lockstep with codex-rs while using its client id.
pub const OPENAI_CHATGPT_ORIGINATOR: &str = "codex_cli_rs";
