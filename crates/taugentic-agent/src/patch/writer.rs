use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use super::{AppliedPatch, ApplyError, FileChangeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReport {
    pub bytes_written: u64,
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("path escapes workdir: {path}")]
    PathEscape { path: PathBuf },
    #[error("failed to create parent directory for {path}: {source}")]
    CreateParent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to delete {path}: {source}")]
    DeleteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to rename {from} to {to}: {source}")]
    RenameFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl WriteError {
    pub fn to_apply_error(&self) -> Option<ApplyError> {
        match self {
            Self::PathEscape { path } => Some(ApplyError::PathEscape { path: path.clone() }),
            _ => None,
        }
    }
}

pub fn write_applied_patch(
    applied: &AppliedPatch,
    workdir: &Path,
) -> Result<WriteReport, WriteError> {
    let workdir = canonicalize_workdir(workdir)?;
    validate_changes(applied, &workdir)?;

    let mut bytes_written = 0;
    for change in &applied.changed_files {
        match change.kind {
            FileChangeKind::Added => {
                let path = workdir.join(&change.path);
                let content = change.new_content.as_deref().unwrap_or_default();
                write_file(&path, content)?;
                bytes_written += content.len() as u64;
            }
            FileChangeKind::Deleted => {
                let path = workdir.join(&change.path);
                fs::remove_file(&path).map_err(|source| WriteError::DeleteFile {
                    path: change.path.clone(),
                    source,
                })?;
            }
            FileChangeKind::Updated => {
                let source = workdir.join(&change.path);
                let target_relative = change.move_to.as_ref().unwrap_or(&change.path);
                let target = workdir.join(target_relative);
                let content = change.new_content.as_deref().unwrap_or_default();
                write_file(&target, content)?;
                bytes_written += content.len() as u64;
                if change.move_to.is_some() {
                    fs::remove_file(&source).map_err(|source_error| WriteError::RenameFile {
                        from: change.path.clone(),
                        to: target_relative.clone(),
                        source: source_error,
                    })?;
                }
            }
        }
    }

    Ok(WriteReport { bytes_written })
}

pub fn validate_changes(applied: &AppliedPatch, workdir: &Path) -> Result<(), WriteError> {
    let workdir = canonicalize_workdir(workdir)?;
    for change in &applied.changed_files {
        ensure_contained(&workdir, &change.path)?;
        if let Some(move_to) = &change.move_to {
            ensure_contained(&workdir, move_to)?;
        }
    }
    Ok(())
}

fn write_file(path: &Path, content: &str) -> Result<(), WriteError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| WriteError::CreateParent {
            path: path.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| WriteError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn canonicalize_workdir(workdir: &Path) -> Result<PathBuf, WriteError> {
    workdir.canonicalize().map_err(|_| WriteError::PathEscape {
        path: workdir.to_path_buf(),
    })
}

pub fn ensure_contained(workdir: &Path, relative_path: &Path) -> Result<(), WriteError> {
    if relative_path.as_os_str().is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(WriteError::PathEscape {
            path: relative_path.to_path_buf(),
        });
    }

    let absolute = workdir.join(relative_path);
    let canonical = match absolute.canonicalize() {
        Ok(path) => path,
        Err(_) => canonicalize_missing_path(workdir, relative_path)?,
    };
    if canonical.starts_with(workdir) {
        Ok(())
    } else {
        Err(WriteError::PathEscape {
            path: relative_path.to_path_buf(),
        })
    }
}

fn canonicalize_missing_path(workdir: &Path, relative_path: &Path) -> Result<PathBuf, WriteError> {
    let mut existing_parent = workdir.to_path_buf();
    let mut missing = Vec::new();
    for component in relative_path.components() {
        let Component::Normal(part) = component else {
            return Err(WriteError::PathEscape {
                path: relative_path.to_path_buf(),
            });
        };
        let candidate = existing_parent.join(part);
        if candidate.exists() {
            existing_parent = candidate;
        } else {
            missing.push(PathBuf::from(part));
        }
    }

    let mut canonical = existing_parent
        .canonicalize()
        .map_err(|_| WriteError::PathEscape {
            path: relative_path.to_path_buf(),
        })?;
    for component in missing {
        canonical.push(component);
    }
    Ok(canonical)
}
