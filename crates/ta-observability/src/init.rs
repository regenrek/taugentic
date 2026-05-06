use std::{fs, io::IsTerminal};

use thiserror::Error;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan, writer::MakeWriterExt},
    prelude::*,
    util::TryInitError,
};

use crate::{LogFormat, ObservabilityConfig, ObservabilityConfigError};

#[derive(Debug)]
pub struct ObservabilityHandle {
    config: ObservabilityConfig,
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

impl ObservabilityHandle {
    pub fn config(&self) -> &ObservabilityConfig {
        &self.config
    }
}

#[derive(Debug, Error)]
pub enum ObservabilityInitError {
    #[error(transparent)]
    Config(#[from] ObservabilityConfigError),
    #[error("failed to create log directory {path}: {source}")]
    CreateLogDirectory {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to initialize tracing subscriber: {0}")]
    Subscriber(#[from] TryInitError),
}

pub fn init(config: ObservabilityConfig) -> Result<ObservabilityHandle, ObservabilityInitError> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(config.default_level.clone()))
        .unwrap_or_else(|_| EnvFilter::new(config.default_level.clone()));
    let effective_format = effective_format(&config);

    let file_writer = if let Some(output) = &config.file_output {
        fs::create_dir_all(&output.directory).map_err(|source| {
            ObservabilityInitError::CreateLogDirectory {
                path: output.directory.display().to_string(),
                source,
            }
        })?;

        let appender = tracing_appender::rolling::daily(&output.directory, &output.file_name);
        let (writer, guard) = tracing_appender::non_blocking(appender);
        Some((writer, guard))
    } else {
        None
    };

    match (config.stderr_enabled, file_writer) {
        (true, Some((file_writer, guard))) => {
            init_with_writer(
                env_filter,
                effective_format,
                std::io::stderr.and(file_writer),
            )?;
            log_initialized(&config, effective_format);
            Ok(ObservabilityHandle {
                config,
                _file_guard: Some(guard),
            })
        }
        (true, None) => {
            init_with_writer(env_filter, effective_format, std::io::stderr)?;
            log_initialized(&config, effective_format);
            Ok(ObservabilityHandle {
                config,
                _file_guard: None,
            })
        }
        (false, Some((file_writer, guard))) => {
            init_with_writer(env_filter, effective_format, file_writer)?;
            log_initialized(&config, effective_format);
            Ok(ObservabilityHandle {
                config,
                _file_guard: Some(guard),
            })
        }
        (false, None) => {
            tracing_subscriber::registry().try_init()?;
            Ok(ObservabilityHandle {
                config,
                _file_guard: None,
            })
        }
    }
}

fn init_with_writer<W>(
    env_filter: EnvFilter,
    format: LogFormat,
    writer: W,
) -> Result<(), TryInitError>
where
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
    let stderr_is_terminal = std::io::stderr().is_terminal();
    match format {
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .pretty()
                    .with_writer(writer)
                    .with_ansi(stderr_is_terminal)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(env_filter),
            )
            .try_init(),
        LogFormat::Json => tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .json()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(env_filter),
            )
            .try_init(),
    }
}

fn effective_format(config: &ObservabilityConfig) -> LogFormat {
    if config.file_output.is_some() {
        LogFormat::Json
    } else {
        config.stderr_format
    }
}

fn log_initialized(config: &ObservabilityConfig, effective_format: LogFormat) {
    tracing::info!(
        service.name = %config.service_name,
        log.stderr = config.stderr_enabled,
        log.configured_stderr_format = config.stderr_format.as_str(),
        log.effective_format = effective_format.as_str(),
        log.file = config.file_output.as_ref().map(|output| output.file_name.as_str()).unwrap_or("disabled"),
        "observability initialized"
    );
}
