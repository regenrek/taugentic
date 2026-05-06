# OpenAI ChatGPT Login

Use this runbook for the native ChatGPT subscription auth profile. API-key auth remains environment-owned by `OPENAI_API_KEY`; this page covers `auth-openai-chatgpt`.

## Flow

1. Open the desktop app.
2. Open **Agent Runtime**.
3. Select **OpenAI ChatGPT Subscription**.
4. Click **Login**.
5. The daemon starts a localhost PKCE callback server and returns a browser challenge to the renderer.
6. Complete the OpenAI browser flow.
7. The callback returns to `http://localhost:<port>/auth/callback`, the daemon exchanges the code, then stores credentials.

Callback ports are fixed by `crates/ta-auth-openai/src/oauth/endpoints.rs`:

```text
http://localhost:1455/auth/callback
http://localhost:1457/auth/callback
```

The authorization request includes:

```text
response_type=code
client_id=app_EMoamEEZ73f0CkXaXp7hrann
scope=openid profile email offline_access api.connectors.read api.connectors.invoke
code_challenge_method=S256
id_token_add_organizations=true
codex_cli_simplified_flow=true
originator=codex_cli_rs
```

If browser launch fails, the UI shows a manual URL. Copy it into a browser on the same host so the localhost callback can complete.

## Credential Storage

OpenAI OAuth storage is owned by `crates/ta-auth-openai/src/credential_store/`. Backend selection comes from `crates/ta-host-platform` via `secrets_backend_capability()`.

| OS | Backend | Service / account |
| --- | --- | --- |
| macOS | Keychain | `taugentic.openai.oauth/openai_chatgpt` |
| Linux | Secret Service when available; otherwise non-durable in-memory fallback | `taugentic.openai.oauth/openai_chatgpt` |
| Windows | Credential Manager | `taugentic.openai.oauth/openai_chatgpt` |

Taugentic does not read or write Codex credential paths.

## Logout

Use the desktop app:

1. Open **Agent Runtime**.
2. Select **OpenAI ChatGPT Subscription**.
3. Click **Logout**.

Logout calls the daemon auth-profile mutation `daemon.agent.runtime.auth.logout`, attempts token revocation at `https://auth.openai.com/oauth/revoke`, and clears local credentials even if revoke fails. There is no `ta` CLI logout command for auth profiles.

## Verify The Originator Invariant

The invariant from PR #74 is code-owned in `crates/ta-auth-openai`:

```sh
rg 'OPENAI_CHATGPT_ORIGINATOR|authorize_originator|codex_cli_rs' crates/ta-auth-openai/src
cargo test -p ta-auth-openai start_pins_codex_client_id_to_codex_originator
```

Expected result: `OPENAI_CHATGPT_ORIGINATOR` is `codex_cli_rs`, and the shared ChatGPT client id always authorizes with that originator. Do not override it to `taugentic` while Taugentic uses the Codex-owned OAuth registration.

## Troubleshooting

`Invalid ID token: missing organization_id` during API-token exchange:

```sh
RUST_LOG=info,ta_auth_openai=trace,ta_provider_llm=debug just daemon
```

Confirm the authorization URL uses `originator=codex_cli_rs`. This exchange can still fail for valid ChatGPT subscription accounts that are not linked to a Platform organization; login should remain connected as subscription-only, and Platform-API features should ask the user to link an organization.

Callback timeout:

```sh
lsof -nP -iTCP:1455 -sTCP:LISTEN
lsof -nP -iTCP:1457 -sTCP:LISTEN
```

Close stale login flows, click **Logout** to cancel any pending completion, then retry **Login**. The daemon waits five minutes for the callback.

Browser does not open:

Use the manual URL shown by **Agent Runtime**. On Linux, ensure the desktop environment can open URLs through the system browser; headless hosts need manual URL handling from a browser on the same host.

Credential backend unavailable:

```sh
RUST_LOG=info,ta_host_platform=debug,ta_auth_openai=debug just daemon
```

On Linux, Secret Service must be reachable for durable credentials. If unavailable, the daemon logs that it is using non-durable in-memory credentials; sign-in will not persist across daemon restarts.
