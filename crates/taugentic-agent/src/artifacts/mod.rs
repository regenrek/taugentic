use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use std::sync::Arc;

use ta_protocol::wire::{ArtifactKind, RunId};
use tracing::instrument;

use crate::tools::{ApplyPatchResult, ShellResult, ToolOutput};
use crate::{ExecutionError, ExecutionSink};

pub const SHELL_LOG_ARTIFACT_THRESHOLD: usize = 4 * 1024;

pub struct ArtifactWriter {
    root: PathBuf,
    run_id: RunId,
    run_segment: String,
    counter: AtomicU64,
}

impl ArtifactWriter {
    #[instrument(skip(root, run_id), fields(run_id = %run_id.as_str()))]
    pub fn new(root: impl AsRef<Path>, run_id: RunId) -> Result<Self, ExecutionError> {
        let run_segment = sanitize_path_segment("artifact run id", run_id.as_str())?;
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|error| {
            ExecutionError::ToolFailed(format!(
                "failed to create artifact root {}: {error}",
                root.display()
            ))
        })?;
        let root = root.canonicalize().map_err(|error| {
            ExecutionError::ToolFailed(format!(
                "failed to canonicalize artifact root {}: {error}",
                root.display()
            ))
        })?;
        Ok(Self {
            root,
            run_id,
            run_segment,
            counter: AtomicU64::new(0),
        })
    }

    #[instrument(skip(self, diff_text), fields(run_id = %self.run_id.as_str(), kind = kind_discriminant))]
    pub fn write_patch(
        &self,
        kind_discriminant: &str,
        diff_text: &str,
    ) -> Result<PathBuf, ExecutionError> {
        self.write_artifact(kind_discriminant, "diff", diff_text.as_bytes())
    }

    #[instrument(skip(self, stdout, stderr), fields(run_id = %self.run_id.as_str(), kind = kind_discriminant))]
    pub fn write_log(
        &self,
        kind_discriminant: &str,
        stdout: &str,
        stderr: &str,
    ) -> Result<PathBuf, ExecutionError> {
        let content = format!("stdout:\n{stdout}\nstderr:\n{stderr}");
        self.write_artifact(kind_discriminant, "log", content.as_bytes())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write_artifact(
        &self,
        kind_discriminant: &str,
        extension: &str,
        content: &[u8],
    ) -> Result<PathBuf, ExecutionError> {
        let kind = sanitize_path_segment("artifact kind discriminant", kind_discriminant)?;
        let run_dir = self.root.join(&self.run_segment);
        self.ensure_run_dir(&run_dir)?;

        loop {
            let sequence = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            let path = run_dir.join(format!("{kind}-{sequence}.{extension}"));
            self.ensure_candidate_parent(&path)?;
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(content)
                        .map_err(|error| artifact_error("write", &path, error))?;
                    return self.canonical_artifact_path(&path);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(artifact_error("create", &path, error)),
            }
        }
    }

    fn ensure_run_dir(&self, run_dir: &Path) -> Result<(), ExecutionError> {
        if !run_dir.starts_with(&self.root) {
            return Err(ExecutionError::ToolFailed(
                "artifact path must stay inside artifact root".to_string(),
            ));
        }
        fs::create_dir_all(run_dir).map_err(|error| artifact_error("create", run_dir, error))?;
        let canonical = run_dir
            .canonicalize()
            .map_err(|error| artifact_error("canonicalize", run_dir, error))?;
        if !canonical.starts_with(&self.root) {
            return Err(ExecutionError::ToolFailed(
                "artifact path must stay inside artifact root".to_string(),
            ));
        }
        Ok(())
    }

    fn ensure_candidate_parent(&self, path: &Path) -> Result<(), ExecutionError> {
        if !path.starts_with(&self.root) {
            return Err(ExecutionError::ToolFailed(
                "artifact path must stay inside artifact root".to_string(),
            ));
        }
        let Some(parent) = path.parent() else {
            return Err(ExecutionError::ToolFailed(
                "artifact path must have a parent".to_string(),
            ));
        };
        let parent = parent
            .canonicalize()
            .map_err(|error| artifact_error("canonicalize", parent, error))?;
        if !parent.starts_with(&self.root) {
            return Err(ExecutionError::ToolFailed(
                "artifact path must stay inside artifact root".to_string(),
            ));
        }
        Ok(())
    }

    fn canonical_artifact_path(&self, path: &Path) -> Result<PathBuf, ExecutionError> {
        let canonical = path
            .canonicalize()
            .map_err(|error| artifact_error("canonicalize", path, error))?;
        if !canonical.starts_with(&self.root) {
            return Err(ExecutionError::ToolFailed(
                "artifact path must stay inside artifact root".to_string(),
            ));
        }
        Ok(canonical)
    }
}

pub fn record_tool_artifact(
    writer: &ArtifactWriter,
    sink: &Arc<dyn ExecutionSink>,
    tool_name: &str,
    output: &ToolOutput,
) -> Result<(), ExecutionError> {
    match tool_name {
        "apply_patch" => {
            let result: ApplyPatchResult = serde_json::from_value(output.content.clone())
                .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
            if !result.diff_text.is_empty() {
                let path = writer.write_patch("patch", &result.diff_text)?;
                sink.record_artifact(ArtifactKind::Patch, &path.to_string_lossy())?;
            }
        }
        "shell" => {
            let result: ShellResult = serde_json::from_value(output.content.clone())
                .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
            if result.truncated
                || result.stdout.len().saturating_add(result.stderr.len())
                    >= SHELL_LOG_ARTIFACT_THRESHOLD
            {
                let path = writer.write_log("shell", &result.stdout, &result.stderr)?;
                sink.record_artifact(ArtifactKind::CommandLog, &path.to_string_lossy())?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sanitize_path_segment(label: &str, value: &str) -> Result<String, ExecutionError> {
    let value = value.trim();
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ExecutionError::InvalidToolInput(format!(
            "{label} must be non-empty ascii alphanumeric, dash, or underscore"
        )));
    }
    Ok(value.to_string())
}

fn artifact_error(action: &str, path: &Path, error: io::Error) -> ExecutionError {
    ExecutionError::ToolFailed(format!(
        "failed to {action} artifact path {}: {error}",
        path.display()
    ))
}
