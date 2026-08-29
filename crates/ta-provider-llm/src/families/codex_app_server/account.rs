use serde_json::{Value, json};

use super::CodexLlmClientError;
use super::client::CodexAppServerSession;
use super::events::required_string;

impl CodexAppServerSession {
    pub(crate) fn start_chatgpt_login(&mut self) -> Result<(String, String), CodexLlmClientError> {
        let result = self.request("account/login/start", json!({"type": "chatgpt"}))?;
        if result.get("type").and_then(Value::as_str) != Some("chatgpt") {
            return Err(CodexLlmClientError::Protocol(
                "account/login/start returned an unexpected login type".to_string(),
            ));
        }
        Ok((
            required_string(&result, "loginId")?,
            required_string(&result, "authUrl")?,
        ))
    }

    pub(crate) fn wait_for_chatgpt_login(
        &mut self,
        expected_login_id: &str,
    ) -> Result<(), CodexLlmClientError> {
        let mut login_completed = false;
        loop {
            let Some(message) = self.recv_message_tick()? else {
                self.ensure_child_running()?;
                continue;
            };
            if message.get("id").is_some() && message.get("method").is_none() {
                return Err(CodexLlmClientError::Protocol(
                    "received an unexpected response while waiting for account login".to_string(),
                ));
            }
            let Some(method) = message.get("method").and_then(Value::as_str) else {
                self.respond_to_server_request(&message)?;
                continue;
            };
            if method == "account/updated" && login_completed {
                return Ok(());
            }
            if method != "account/login/completed" {
                self.respond_to_server_request(&message)?;
                continue;
            }
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            if params.get("loginId").and_then(Value::as_str) != Some(expected_login_id) {
                continue;
            }
            match params.get("success").and_then(Value::as_bool) {
                Some(true) => login_completed = true,
                Some(false) => Err(CodexLlmClientError::Auth(
                    "Codex ChatGPT login was not completed".to_string(),
                ))?,
                None => Err(CodexLlmClientError::Protocol(
                    "account/login/completed omitted success".to_string(),
                ))?,
            }
        }
    }

    pub(crate) fn read_chatgpt_account(
        &mut self,
    ) -> Result<Option<(Option<String>, Option<String>)>, CodexLlmClientError> {
        let result = self.request("account/read", json!({}))?;
        let Some(account) = result.get("account").filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        if account.get("type").and_then(Value::as_str) != Some("chatgpt") {
            return Ok(None);
        }
        Ok(Some((
            account
                .get("email")
                .and_then(Value::as_str)
                .map(str::to_string),
            account
                .get("planType")
                .and_then(Value::as_str)
                .map(str::to_string),
        )))
    }

    pub(crate) fn logout_account(&mut self) -> Result<(), CodexLlmClientError> {
        let _ = self.request_without_params("account/logout")?;
        Ok(())
    }
}
