use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::model::{FileOp, Hunk, HunkLineKind, Patch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPatch {
    pub changed_files: Vec<FileChange>,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub move_to: Option<PathBuf>,
    pub kind: FileChangeKind,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub added_lines: usize,
    pub removed_lines: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Added,
    Deleted,
    Updated,
}

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("no files were modified")]
    EmptyPatch,
    #[error("failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect {path}: {source}")]
    InspectFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot delete directory {0}")]
    DeleteDirectory(PathBuf),
    #[error("failed to find context '{context}' in {path}")]
    MissingContext { path: PathBuf, context: String },
    #[error("failed to find expected lines in {path}:\n{expected}")]
    MissingLines { path: PathBuf, expected: String },
    #[error("path escapes workdir: {path}")]
    PathEscape { path: PathBuf },
}

pub fn apply_patch(patch: &Patch, workdir: &Path) -> Result<AppliedPatch, ApplyError> {
    if patch.operations.is_empty() {
        return Err(ApplyError::EmptyPatch);
    }

    let mut changed_files = Vec::new();
    for operation in &patch.operations {
        changed_files.push(apply_file_op(operation, workdir)?);
    }

    let added_lines = changed_files.iter().map(|change| change.added_lines).sum();
    let removed_lines = changed_files
        .iter()
        .map(|change| change.removed_lines)
        .sum();
    let diff = changed_files
        .iter()
        .map(render_change_diff)
        .collect::<Vec<_>>()
        .join("");

    Ok(AppliedPatch {
        changed_files,
        added_lines,
        removed_lines,
        diff,
    })
}

fn apply_file_op(operation: &FileOp, workdir: &Path) -> Result<FileChange, ApplyError> {
    match operation {
        FileOp::AddFile { path, contents } => Ok(FileChange {
            path: path.clone(),
            move_to: None,
            kind: FileChangeKind::Added,
            old_content: None,
            new_content: Some(contents.clone()),
            added_lines: count_lines(contents),
            removed_lines: 0,
        }),
        FileOp::DeleteFile { path } => {
            let absolute = workdir.join(path);
            let metadata = fs::metadata(&absolute).map_err(|source| ApplyError::InspectFile {
                path: path.clone(),
                source,
            })?;
            if metadata.is_dir() {
                return Err(ApplyError::DeleteDirectory(path.clone()));
            }
            let old_content =
                fs::read_to_string(&absolute).map_err(|source| ApplyError::ReadFile {
                    path: path.clone(),
                    source,
                })?;
            Ok(FileChange {
                path: path.clone(),
                move_to: None,
                kind: FileChangeKind::Deleted,
                old_content: Some(old_content.clone()),
                new_content: None,
                added_lines: 0,
                removed_lines: count_lines(&old_content),
            })
        }
        FileOp::UpdateFile {
            path,
            move_to,
            hunks,
        } => {
            let old_content =
                fs::read_to_string(workdir.join(path)).map_err(|source| ApplyError::ReadFile {
                    path: path.clone(),
                    source,
                })?;
            let new_content = derive_new_content(path, &old_content, hunks)?;
            let added_lines = count_added_lines(hunks);
            let removed_lines = count_removed_lines(hunks);
            Ok(FileChange {
                path: path.clone(),
                move_to: move_to.clone(),
                kind: FileChangeKind::Updated,
                old_content: Some(old_content),
                new_content: Some(new_content),
                added_lines,
                removed_lines,
            })
        }
    }
}

fn derive_new_content(
    path: &Path,
    old_content: &str,
    hunks: &[Hunk],
) -> Result<String, ApplyError> {
    let mut original_lines = split_lines(old_content);
    let replacements = compute_replacements(path, &original_lines, hunks)?;
    for (start, old_len, new_lines) in replacements.into_iter().rev() {
        original_lines.splice(start..start + old_len, new_lines);
    }
    if !original_lines.last().is_some_and(String::is_empty) {
        original_lines.push(String::new());
    }
    Ok(original_lines.join("\n"))
}

fn compute_replacements(
    path: &Path,
    original_lines: &[String],
    hunks: &[Hunk],
) -> Result<Vec<(usize, usize, Vec<String>)>, ApplyError> {
    let mut replacements = Vec::new();
    let mut line_index = 0;

    for hunk in hunks {
        if let Some(header) = &hunk.header {
            if let Some(index) = seek_sequence(
                original_lines,
                std::slice::from_ref(header),
                line_index,
                false,
            ) {
                line_index = index + 1;
            } else {
                return Err(ApplyError::MissingContext {
                    path: path.to_path_buf(),
                    context: header.clone(),
                });
            }
        }

        let old_lines = old_lines(hunk);
        let new_lines = new_lines(hunk);
        if old_lines.is_empty() {
            replacements.push((original_lines.len(), 0, new_lines));
            continue;
        }

        let mut pattern = old_lines.as_slice();
        let mut replacement = new_lines.as_slice();
        let mut found = seek_sequence(original_lines, pattern, line_index, hunk.end_of_file);
        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if replacement.last().is_some_and(String::is_empty) {
                replacement = &replacement[..replacement.len() - 1];
            }
            found = seek_sequence(original_lines, pattern, line_index, hunk.end_of_file);
        }

        let Some(start) = found else {
            return Err(ApplyError::MissingLines {
                path: path.to_path_buf(),
                expected: old_lines.join("\n"),
            });
        };
        replacements.push((start, pattern.len(), replacement.to_vec()));
        line_index = start + pattern.len();
    }

    replacements.sort_by_key(|(start, _, _)| *start);
    Ok(replacements)
}

fn old_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|line| match line.kind {
            HunkLineKind::Context | HunkLineKind::Removed => Some(line.text.clone()),
            HunkLineKind::Added => None,
        })
        .collect()
}

fn new_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|line| match line.kind {
            HunkLineKind::Context | HunkLineKind::Added => Some(line.text.clone()),
            HunkLineKind::Removed => None,
        })
        .collect()
}

fn split_lines(content: &str) -> Vec<String> {
    let mut lines: Vec<String> = content.split('\n').map(ToString::to_string).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

fn seek_sequence(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > lines.len() {
        return None;
    }
    let max_start = lines.len().saturating_sub(pattern.len());
    if eof {
        return (start..=max_start)
            .rev()
            .find(|candidate| lines[*candidate..*candidate + pattern.len()] == *pattern);
    }
    (start..=max_start).find(|candidate| lines[*candidate..*candidate + pattern.len()] == *pattern)
}

fn count_lines(content: &str) -> usize {
    content.lines().count()
}

fn count_added_lines(hunks: &[Hunk]) -> usize {
    hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == HunkLineKind::Added)
        .count()
}

fn count_removed_lines(hunks: &[Hunk]) -> usize {
    hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == HunkLineKind::Removed)
        .count()
}

fn render_change_diff(change: &FileChange) -> String {
    let old_path = change.path.display();
    let new_path = change.move_to.as_ref().unwrap_or(&change.path).display();
    let mut out = format!("--- {old_path}\n+++ {new_path}\n");
    match change.kind {
        FileChangeKind::Added => {
            for line in change.new_content.as_deref().unwrap_or_default().lines() {
                out.push('+');
                out.push_str(line);
                out.push('\n');
            }
        }
        FileChangeKind::Deleted => {
            for line in change.old_content.as_deref().unwrap_or_default().lines() {
                out.push('-');
                out.push_str(line);
                out.push('\n');
            }
        }
        FileChangeKind::Updated => {
            out.push_str("@@\n");
            for line in change.old_content.as_deref().unwrap_or_default().lines() {
                out.push('-');
                out.push_str(line);
                out.push('\n');
            }
            for line in change.new_content.as_deref().unwrap_or_default().lines() {
                out.push('+');
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}
