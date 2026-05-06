use std::env;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthProfileConnectionState, AuthProfileId, AuthProfileLoginResult,
    AuthProfileLogoutResult, AuthProfileManagementMode, AuthProfileMethodInfo, AuthProfileRef,
    AuthProfileState,
};

use crate::families::codex_app_server::client::CodexCli;
use crate::families::codex_app_server::{
    CODEX_API_KEY_AUTH_PROFILE_ID, CODEX_CHATGPT_AUTH_PROFILE_ID, CODEX_PROVIDER_ID, CodexAuthMode,
    CodexLlmClientError, OPENAI_API_KEY_ENV_VAR, matches_auth_profile_id,
};

const LOGIN_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const LOGIN_CONFIRM_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn auth_mode(cli: &CodexCli) -> CodexAuthMode {
    auth_mode_with_timeout(cli, LOGIN_STATUS_TIMEOUT)
}

fn auth_mode_with_timeout(cli: &CodexCli, timeout: Duration) -> CodexAuthMode {
    match cli.run_with_timeout(&["login", "status"], None, Some(timeout)) {
        Ok(output) => parse_auth_mode_output(&output.stdout, &output.stderr),
        Err(CodexLlmClientError::CliUnavailable(error)) => CodexAuthMode::Unavailable(error),
        Err(CodexLlmClientError::CommandTimedOut(error)) => CodexAuthMode::Unavailable(error),
        Err(CodexLlmClientError::CommandFailed(error)) => CodexAuthMode::Unknown(error),
        Err(error) => CodexAuthMode::Unknown(error.to_string()),
    }
}

pub(crate) fn auth_profiles_for_mode(mode: &CodexAuthMode) -> Vec<AuthProfileState> {
    vec![
        auth_profile_state(
            CODEX_CHATGPT_AUTH_PROFILE_ID,
            "Codex ChatGPT",
            mode,
            CodexAuthMode::Chatgpt,
        ),
        auth_profile_state(
            CODEX_API_KEY_AUTH_PROFILE_ID,
            "Codex API Key",
            mode,
            CodexAuthMode::ApiKey,
        ),
    ]
}

pub(crate) fn login(
    cli: &CodexCli,
    auth_profile_id: &AuthProfileId,
) -> crate::families::codex_app_server::CodexLoginResult {
    match auth_profile_id.as_str() {
        CODEX_CHATGPT_AUTH_PROFILE_ID => {
            cli.run(&["login"], None)?;
            refreshed_auth_profile(cli, auth_profile_id)
        }
        CODEX_API_KEY_AUTH_PROFILE_ID => {
            let api_key = env::var(OPENAI_API_KEY_ENV_VAR)
                .map_err(|_| CodexLlmClientError::MissingApiKeyEnv)?;
            cli.run(&["login", "--with-api-key"], Some(&api_key))?;
            refreshed_auth_profile(cli, auth_profile_id)
        }
        other => Err(CodexLlmClientError::UnknownAuthProfile(other.to_string())),
    }
}

pub(crate) fn logout(
    cli: &CodexCli,
    auth_profile_id: &AuthProfileId,
) -> crate::families::codex_app_server::CodexLogoutResult {
    if !matches_auth_profile_id(auth_profile_id) {
        return Err(CodexLlmClientError::UnknownAuthProfile(
            auth_profile_id.as_str().to_string(),
        ));
    }
    cli.run(&["logout"], None)?;
    Ok(AuthProfileLogoutResult {
        auth_profile_id: auth_profile_id.clone(),
        disconnected: true,
    })
}

fn refreshed_auth_profile(
    cli: &CodexCli,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, CodexLlmClientError> {
    let profile = auth_profiles_for_mode(&auth_mode_with_timeout(cli, LOGIN_CONFIRM_TIMEOUT))
        .into_iter()
        .find(|profile| profile.profile.id == *auth_profile_id)
        .ok_or_else(|| {
            CodexLlmClientError::UnknownAuthProfile(auth_profile_id.as_str().to_string())
        })?;
    if profile.connection_state != AuthProfileConnectionState::Connected {
        return Err(CodexLlmClientError::LoginDidNotAuthenticate);
    }
    Ok(AuthProfileLoginResult {
        auth_profile: profile,
        challenge: None,
    })
}

fn auth_profile_state(
    auth_profile_id: &str,
    display_name: &str,
    mode: &CodexAuthMode,
    expected_mode: CodexAuthMode,
) -> AuthProfileState {
    let management_mode = if auth_profile_id == CODEX_API_KEY_AUTH_PROFILE_ID {
        AuthProfileManagementMode::Environment
    } else {
        AuthProfileManagementMode::Interactive
    };
    let (connection_state, last_error) = match mode {
        CodexAuthMode::Unavailable(error) => {
            (AuthProfileConnectionState::Error, Some(error.clone()))
        }
        CodexAuthMode::Unknown(message) => {
            (AuthProfileConnectionState::Error, Some(message.clone()))
        }
        current if *current == expected_mode => (AuthProfileConnectionState::Connected, None),
        _ => (AuthProfileConnectionState::LoggedOut, None),
    };
    AuthProfileState {
        profile: AuthProfileRef {
            id: AuthProfileId::new(auth_profile_id).expect("auth profile id"),
            provider_id: AgentRuntimeStrategyId::new(CODEX_PROVIDER_ID).expect("provider id"),
            display_name: display_name.to_string(),
        },
        connection_state,
        last_error,
        management_mode: management_mode.clone(),
        can_login: true,
        can_logout: true,
        platform_org_linked: None,
        setup_steps: if auth_profile_id == CODEX_API_KEY_AUTH_PROFILE_ID {
            vec![format!(
                "Set {OPENAI_API_KEY_ENV_VAR} in the daemon environment"
            )]
        } else {
            Vec::new()
        },
        action: None,
        methods: vec![AuthProfileMethodInfo {
            id: auth_profile_id.to_string(),
            display_name: display_name.to_string(),
            management_mode,
        }],
    }
}

fn parse_auth_mode_output(stdout: &str, stderr: &str) -> CodexAuthMode {
    let output = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let normalized = output.trim();
    if normalized.contains("Logged in using ChatGPT") {
        CodexAuthMode::Chatgpt
    } else if normalized.contains("Logged in using API key") {
        CodexAuthMode::ApiKey
    } else if normalized.contains("Not logged in") {
        CodexAuthMode::LoggedOut
    } else {
        CodexAuthMode::Unknown(normalized.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-artifacts/ta-provider-llm-oauth")
            .join(format!("{name}-{suffix}"))
    }

    #[cfg(unix)]
    fn write_script(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let dir = unique_dir(name);
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("codex");
        fs::write(&path, body).expect("script");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }

    #[test]
    fn parses_chatgpt_login_status() {
        assert_eq!(
            parse_auth_mode_output("Logged in using ChatGPT", ""),
            CodexAuthMode::Chatgpt
        );
    }

    #[test]
    fn parses_api_key_login_status() {
        assert_eq!(
            parse_auth_mode_output("Logged in using API key", ""),
            CodexAuthMode::ApiKey
        );
    }

    #[test]
    fn parses_logged_out_status() {
        assert_eq!(
            parse_auth_mode_output("Not logged in", ""),
            CodexAuthMode::LoggedOut
        );
    }

    #[test]
    fn parses_chatgpt_login_status_from_stderr_when_stdout_is_empty() {
        assert_eq!(
            parse_auth_mode_output("", "Logged in using ChatGPT"),
            CodexAuthMode::Chatgpt
        );
    }

    #[test]
    #[cfg(unix)]
    fn login_confirmation_uses_longer_timeout_than_background_probe() {
        let binary = write_script(
            "login-confirm-timeout",
            "#!/bin/sh
if [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then
  sleep 3
  printf 'Logged in using ChatGPT\\n'
  exit 0
fi
if [ \"$1\" = \"login\" ]; then
  exit 0
fi
printf 'unexpected\\n' 1>&2
exit 1
",
        );
        let cli = CodexCli::with_binary(binary);
        let auth_profile_id =
            AuthProfileId::new(CODEX_CHATGPT_AUTH_PROFILE_ID).expect("auth profile id");

        let login_result = login(&cli, &auth_profile_id).expect("login should succeed");
        assert_eq!(
            login_result.auth_profile.connection_state,
            AuthProfileConnectionState::Connected
        );

        assert!(matches!(
            auth_mode(&cli),
            CodexAuthMode::Unavailable(message) if message.contains("exceeded 2000ms")
        ));
    }
}
