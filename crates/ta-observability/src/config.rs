use std::{env, path::PathBuf};

use thiserror::Error;

pub const LOG_FORMAT_ENV_VAR: &str = "TAUGENTIC_LOG_FORMAT";
pub const LOG_DIR_ENV_VAR: &str = "TAUGENTIC_LOG_DIR";
pub const LOG_STDERR_ENV_VAR: &str = "TAUGENTIC_LOG_STDERR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl LogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ObservabilityConfigError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(ObservabilityConfigError::InvalidLogFormat(
                other.to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLogOutput {
    pub directory: PathBuf,
    pub file_name: String,
}

impl FileLogOutput {
    pub fn path(&self) -> PathBuf {
        self.directory.join(&self.file_name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservabilityEnvInputs {
    pub stderr_format: Option<String>,
    pub stderr_enabled: Option<String>,
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub default_level: String,
    pub stderr_enabled: bool,
    pub stderr_format: LogFormat,
    pub file_output: Option<FileLogOutput>,
}

impl ObservabilityConfig {
    pub fn cli(service_name: &str, default_level: &str) -> Result<Self, ObservabilityConfigError> {
        Self::cli_from_inputs(
            service_name,
            default_level,
            ObservabilityEnvInputs::from_env(),
        )
    }

    pub fn cli_from_inputs(
        service_name: &str,
        default_level: &str,
        inputs: ObservabilityEnvInputs,
    ) -> Result<Self, ObservabilityConfigError> {
        Self::from_inputs(service_name, default_level, None, inputs)
    }

    fn from_inputs(
        service_name: &str,
        default_level: &str,
        default_file_output: Option<FileLogOutput>,
        inputs: ObservabilityEnvInputs,
    ) -> Result<Self, ObservabilityConfigError> {
        let stderr_format = inputs
            .stderr_format
            .as_deref()
            .map(LogFormat::parse)
            .transpose()?
            .unwrap_or(LogFormat::Pretty);
        let stderr_enabled = inputs
            .stderr_enabled
            .as_deref()
            .map(|value| parse_bool_env(LOG_STDERR_ENV_VAR, value))
            .transpose()?
            .unwrap_or(true);
        let file_output = select_file_output(default_file_output, inputs.log_dir);

        Ok(Self {
            service_name: service_name.to_string(),
            default_level: default_level.to_string(),
            stderr_enabled,
            stderr_format,
            file_output,
        })
    }
}

impl ObservabilityEnvInputs {
    pub fn from_env() -> Self {
        Self {
            stderr_format: env::var(LOG_FORMAT_ENV_VAR).ok(),
            stderr_enabled: env::var(LOG_STDERR_ENV_VAR).ok(),
            log_dir: env::var_os(LOG_DIR_ENV_VAR)
                .map(|directory| directory.to_string_lossy().trim().to_string())
                .filter(|directory| !directory.is_empty()),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ObservabilityConfigError {
    #[error("invalid {LOG_FORMAT_ENV_VAR} value (expected one of: pretty, json)")]
    InvalidLogFormat(String),
    #[error("invalid {name} value (expected one of: true, false, 1, 0, yes, no, on, off)")]
    InvalidBool { name: &'static str, value: String },
}

pub fn select_file_output(
    default_file_output: Option<FileLogOutput>,
    override_directory: Option<String>,
) -> Option<FileLogOutput> {
    match (default_file_output, override_directory) {
        (Some(default_file_output), Some(directory)) => Some(FileLogOutput {
            directory: PathBuf::from(directory),
            file_name: default_file_output.file_name,
        }),
        (Some(default_file_output), None) => Some(default_file_output),
        (None, _) => None,
    }
}

pub fn parse_bool_env(name: &'static str, value: &str) -> Result<bool, ObservabilityConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ObservabilityConfigError::InvalidBool {
            name,
            value: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        FileLogOutput, LOG_STDERR_ENV_VAR, LogFormat, ObservabilityConfig,
        ObservabilityConfigError, ObservabilityEnvInputs, parse_bool_env, select_file_output,
    };

    #[test]
    fn parses_boolean_env_values() {
        assert!(parse_bool_env("TEST", "true").expect("true"));
        assert!(!parse_bool_env("TEST", "0").expect("false"));
        assert!(parse_bool_env("TEST", "On").expect("on"));
    }

    #[test]
    fn rejects_invalid_boolean_env_values() {
        let error = parse_bool_env("TEST", "sometimes").expect_err("invalid bool should fail");

        assert_eq!(
            error,
            ObservabilityConfigError::InvalidBool {
                name: "TEST",
                value: "sometimes".to_string(),
            }
        );
    }

    #[test]
    fn invalid_boolean_env_display_does_not_echo_raw_value() {
        let error =
            parse_bool_env(LOG_STDERR_ENV_VAR, "raw-secret-token").expect_err("invalid bool");
        let rendered = error.to_string();

        assert!(rendered.contains("invalid TAUGENTIC_LOG_STDERR value"));
        assert!(rendered.contains("expected one of: true, false, 1, 0, yes, no, on, off"));
        assert!(!rendered.contains("raw-secret-token"));
    }

    #[test]
    fn parses_log_format_values() {
        assert_eq!(
            LogFormat::parse("pretty").expect("pretty"),
            LogFormat::Pretty
        );
        assert_eq!(LogFormat::parse("JSON").expect("json"), LogFormat::Json);
    }

    #[test]
    fn invalid_log_format_display_does_not_echo_raw_value() {
        let error = LogFormat::parse("raw-secret-token").expect_err("invalid log format");
        let rendered = error.to_string();

        assert!(rendered.contains("invalid TAUGENTIC_LOG_FORMAT value"));
        assert!(rendered.contains("expected one of: pretty, json"));
        assert!(!rendered.contains("raw-secret-token"));
    }

    #[test]
    fn keeps_default_file_output_when_log_dir_override_is_missing() {
        let output = select_file_output(
            Some(FileLogOutput {
                directory: PathBuf::from("/tmp/taugentic-daemon/default"),
                file_name: "ta-daemon.log.jsonl".to_string(),
            }),
            None,
        );

        assert_eq!(
            output,
            Some(FileLogOutput {
                directory: PathBuf::from("/tmp/taugentic-daemon/default"),
                file_name: "ta-daemon.log.jsonl".to_string(),
            })
        );
    }

    #[test]
    fn override_directory_replaces_default_file_output_directory() {
        let output = select_file_output(
            Some(FileLogOutput {
                directory: PathBuf::from("/tmp/taugentic-daemon/default"),
                file_name: "ta-daemon.log.jsonl".to_string(),
            }),
            Some("/override/logs".to_string()),
        );

        assert_eq!(
            output,
            Some(FileLogOutput {
                directory: PathBuf::from("/override/logs"),
                file_name: "ta-daemon.log.jsonl".to_string(),
            })
        );
    }

    #[test]
    fn file_log_output_returns_full_path() {
        let output = FileLogOutput {
            directory: PathBuf::from("/tmp/taugentic-daemon/default"),
            file_name: "ta-daemon.log.jsonl".to_string(),
        };

        assert_eq!(
            output.path(),
            PathBuf::from("/tmp/taugentic-daemon/default/ta-daemon.log.jsonl")
        );
    }

    #[test]
    fn cli_from_inputs_uses_explicit_env_inputs() {
        let config = ObservabilityConfig::cli_from_inputs(
            "ta-cli",
            "warn",
            ObservabilityEnvInputs {
                stderr_format: Some("json".to_string()),
                stderr_enabled: Some("0".to_string()),
                log_dir: Some("/override/logs".to_string()),
            },
        )
        .expect("config should load");

        assert_eq!(config.stderr_format, LogFormat::Json);
        assert!(!config.stderr_enabled);
        assert_eq!(config.file_output, None);
    }
}
