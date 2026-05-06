use std::path::{Component, PathBuf};

use thiserror::Error;

use super::model::{FileOp, Hunk, HunkLine, HunkLineKind, Patch};

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error("invalid hunk at line {line_number}: {message}")]
    InvalidHunk { line_number: usize, message: String },
}

pub fn parse_patch(input: &str) -> Result<Patch, ParseError> {
    let lines: Vec<&str> = input.trim().lines().collect();
    check_patch_boundaries(&lines)?;

    let mut operations = Vec::new();
    let mut remaining = &lines[1..lines.len() - 1];
    let mut line_number = 2;
    while !remaining.is_empty() {
        let (operation, parsed) = parse_file_op(remaining, line_number)?;
        operations.push(operation);
        remaining = &remaining[parsed..];
        line_number += parsed;
    }

    Ok(Patch { operations })
}

fn check_patch_boundaries(lines: &[&str]) -> Result<(), ParseError> {
    match lines {
        [] => Err(ParseError::InvalidPatch(
            "The first line of the patch must be '*** Begin Patch'".to_string(),
        )),
        [first, .., last]
            if first.trim() == BEGIN_PATCH_MARKER && last.trim() == END_PATCH_MARKER =>
        {
            Ok(())
        }
        [first, ..] if first.trim() != BEGIN_PATCH_MARKER => Err(ParseError::InvalidPatch(
            "The first line of the patch must be '*** Begin Patch'".to_string(),
        )),
        _ => Err(ParseError::InvalidPatch(
            "The last line of the patch must be '*** End Patch'".to_string(),
        )),
    }
}

fn parse_file_op(lines: &[&str], line_number: usize) -> Result<(FileOp, usize), ParseError> {
    let first = lines[0].trim();
    if let Some(path) = first.strip_prefix(ADD_FILE_MARKER) {
        let mut contents = String::new();
        let mut parsed = 1;
        for line in &lines[1..] {
            if let Some(content) = line.strip_prefix('+') {
                contents.push_str(content);
                contents.push('\n');
                parsed += 1;
            } else {
                break;
            }
        }
        return Ok((
            FileOp::AddFile {
                path: parse_relative_path(path, line_number)?,
                contents,
            },
            parsed,
        ));
    }

    if let Some(path) = first.strip_prefix(DELETE_FILE_MARKER) {
        return Ok((
            FileOp::DeleteFile {
                path: parse_relative_path(path, line_number)?,
            },
            1,
        ));
    }

    if let Some(path) = first.strip_prefix(UPDATE_FILE_MARKER) {
        return parse_update_file(path, &lines[1..], line_number);
    }

    Err(ParseError::InvalidHunk {
        line_number,
        message: format!(
            "'{first}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
        ),
    })
}

fn parse_update_file(
    path: &str,
    lines: &[&str],
    line_number: usize,
) -> Result<(FileOp, usize), ParseError> {
    let mut parsed = 1;
    let mut remaining = lines;
    let mut move_to = None;
    if let Some(first) = remaining.first()
        && let Some(path) = first.trim().strip_prefix(MOVE_TO_MARKER)
    {
        move_to = Some(parse_relative_path(path, line_number + 1)?);
        remaining = &remaining[1..];
        parsed += 1;
    }

    let mut hunks = Vec::new();
    while !remaining.is_empty() {
        if remaining[0].trim().is_empty() {
            remaining = &remaining[1..];
            parsed += 1;
            continue;
        }
        if remaining[0].trim().starts_with("*** ") {
            break;
        }

        let (hunk, consumed) = parse_hunk(remaining, line_number + parsed)?;
        hunks.push(hunk);
        remaining = &remaining[consumed..];
        parsed += consumed;
    }

    if hunks.is_empty() {
        return Err(ParseError::InvalidHunk {
            line_number,
            message: format!("Update file hunk for path '{path}' is empty"),
        });
    }

    Ok((
        FileOp::UpdateFile {
            path: parse_relative_path(path, line_number)?,
            move_to,
            hunks,
        },
        parsed,
    ))
}

fn parse_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), ParseError> {
    let first = lines[0].trim();
    let header = if first == "@@" {
        None
    } else if let Some(header) = first.strip_prefix("@@ ") {
        Some(header.to_string())
    } else {
        return Err(ParseError::InvalidHunk {
            line_number,
            message: format!(
                "Expected update hunk to start with a @@ context marker, got: '{}'",
                lines[0]
            ),
        });
    };

    let mut hunk = Hunk {
        header,
        lines: Vec::new(),
        end_of_file: false,
    };
    let mut parsed = 1;
    for line in &lines[1..] {
        if line.trim() == EOF_MARKER {
            if hunk.lines.is_empty() {
                return Err(ParseError::InvalidHunk {
                    line_number: line_number + parsed,
                    message: "Update hunk does not contain any lines".to_string(),
                });
            }
            hunk.end_of_file = true;
            parsed += 1;
            break;
        }

        let Some(prefix) = line.chars().next() else {
            hunk.lines.push(HunkLine {
                kind: HunkLineKind::Context,
                text: String::new(),
            });
            parsed += 1;
            continue;
        };

        let kind = match prefix {
            ' ' => HunkLineKind::Context,
            '+' => HunkLineKind::Added,
            '-' => HunkLineKind::Removed,
            _ if !hunk.lines.is_empty() => break,
            _ => {
                return Err(ParseError::InvalidHunk {
                    line_number: line_number + parsed,
                    message: format!(
                        "Unexpected line found in update hunk: '{line}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)"
                    ),
                });
            }
        };
        hunk.lines.push(HunkLine {
            kind,
            text: line[1..].to_string(),
        });
        parsed += 1;
    }

    if hunk.lines.is_empty() {
        return Err(ParseError::InvalidHunk {
            line_number: line_number + 1,
            message: "Update hunk does not contain any lines".to_string(),
        });
    }

    Ok((hunk, parsed))
}

fn parse_relative_path(path: &str, line_number: usize) -> Result<PathBuf, ParseError> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ParseError::InvalidHunk {
            line_number,
            message: "File references must be non-empty relative paths".to_string(),
        });
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ParseError::InvalidHunk {
            line_number,
            message: "File references must stay inside the workdir".to_string(),
        });
    }
    Ok(path)
}
