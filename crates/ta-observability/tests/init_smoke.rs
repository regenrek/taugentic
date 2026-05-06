use std::{
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use ta_observability::{FileLogOutput, LogFormat, ObservabilityConfig, init};

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[test]
fn init_writes_structured_startup_record_to_daily_json_log_when_file_output_is_enabled() {
    let log_dir = temp_dir("ta-observability-init-smoke");
    let config = ObservabilityConfig {
        service_name: "ta-daemon".to_string(),
        default_level: "info".to_string(),
        stderr_enabled: false,
        stderr_format: LogFormat::Pretty,
        file_output: Some(FileLogOutput {
            directory: log_dir.clone(),
            file_name: "ta-daemon.log.jsonl".to_string(),
        }),
    };

    let handle = init(config.clone()).expect("observability init should succeed");
    assert_eq!(handle.config(), &config);
    assert!(log_dir.exists(), "init should create log directory");

    let log_path = wait_for_daily_log_file(&log_dir, "ta-daemon.log.jsonl")
        .expect("init should create a rolling log file");
    let entries = wait_for_log_entries(&log_path, &["observability initialized"])
        .expect("startup log should contain observability initialized");
    let init_entry = find_log_entry(&entries, "observability initialized")
        .expect("startup log should contain observability initialized entry");

    assert_eq!(
        init_entry["fields"]["message"],
        Value::String("observability initialized".into())
    );
    assert_eq!(
        init_entry["fields"]["service.name"],
        Value::String("ta-daemon".into())
    );
    assert_eq!(init_entry["fields"]["log.stderr"], Value::Bool(false));
    assert_eq!(
        init_entry["fields"]["log.configured_stderr_format"],
        Value::String("pretty".into())
    );
    assert_eq!(
        init_entry["fields"]["log.effective_format"],
        Value::String("json".into())
    );
    assert_eq!(
        init_entry["fields"]["log.file"],
        Value::String("ta-daemon.log.jsonl".into())
    );

    drop(handle);
    let _ = fs::remove_dir_all(log_dir);
}

fn temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn wait_for_daily_log_file(log_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    let deadline = std::time::Instant::now() + WAIT_TIMEOUT;
    while std::time::Instant::now() < deadline {
        match fs::read_dir(log_dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|error| {
                        format!(
                            "failed to read log directory {}: {error}",
                            log_dir.display()
                        )
                    })?;
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    if name == file_name || name.starts_with(&format!("{file_name}.")) {
                        return Ok(path);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect log directory {}: {error}",
                    log_dir.display()
                ));
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for rolling log file in {}",
        log_dir.display()
    ))
}

fn wait_for_log_entries(log_path: &Path, messages: &[&str]) -> Result<Vec<Value>, String> {
    let deadline = std::time::Instant::now() + WAIT_TIMEOUT;
    while std::time::Instant::now() < deadline {
        match fs::read_to_string(log_path) {
            Ok(contents) => {
                let parsed = parse_jsonl(&contents)?;
                if messages
                    .iter()
                    .all(|message| find_log_entry(&parsed, message).is_some())
                {
                    return Ok(parsed);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read log file {}: {error}",
                    log_path.display()
                ));
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for startup records in {}",
        log_path.display()
    ))
}

fn parse_jsonl(contents: &str) -> Result<Vec<Value>, String> {
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map_err(|error| format!("failed to parse json log line: {error}; line={line}"))
        })
        .collect()
}

fn find_log_entry<'a>(entries: &'a [Value], message: &str) -> Option<&'a Value> {
    entries.iter().find(|entry| {
        entry
            .get("fields")
            .and_then(|fields| fields.get("message"))
            .and_then(Value::as_str)
            == Some(message)
    })
}
