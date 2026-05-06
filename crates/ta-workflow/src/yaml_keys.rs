use std::collections::{BTreeMap, BTreeSet};

use ta_protocol::wire::WorkflowValidationError;

pub(crate) fn duplicate_key_errors(contents: &str) -> Vec<WorkflowValidationError> {
    let mut stack = Vec::<PathFrame>::new();
    let mut seen = BTreeMap::<String, BTreeSet<String>>::new();
    let mut errors = Vec::new();

    for (line_index, line) in contents.lines().enumerate() {
        let Some((indent, key)) = key_at_line(line) else {
            continue;
        };
        while stack.last().is_some_and(|frame| frame.indent >= indent) {
            stack.pop();
        }
        let parent_path = if stack.is_empty() {
            "$".to_string()
        } else {
            format!(
                "$.{}",
                stack
                    .iter()
                    .map(|frame| frame.key.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            )
        };
        let entry = seen.entry(parent_path.clone()).or_default();
        if !entry.insert(key.clone()) {
            errors.push(WorkflowValidationError {
                path: format!("{parent_path}.{key}"),
                message: format!("duplicate key on line {}", line_index + 1),
            });
        }
        stack.push(PathFrame { indent, key });
    }

    errors
}

#[derive(Debug)]
struct PathFrame {
    indent: usize,
    key: String,
}

fn key_at_line(line: &str) -> Option<(usize, String)> {
    let without_comment = line.split('#').next().unwrap_or("").trim_end();
    if without_comment.trim().is_empty() {
        return None;
    }
    let indent = without_comment.len() - without_comment.trim_start().len();
    let trimmed = without_comment.trim_start();
    if trimmed.starts_with('-') {
        return None;
    }
    let colon = trimmed.find(':')?;
    let key = trimmed[..colon].trim();
    if key.is_empty()
        || key.contains(' ')
        || key.contains('[')
        || key.contains(']')
        || key.contains('{')
        || key.contains('}')
    {
        return None;
    }
    Some((indent, key.trim_matches('"').trim_matches('\'').to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_duplicate_mapping_keys_at_same_parent() {
        let errors = duplicate_key_errors(
            r#"
kind: taugentic.workflow/v1
source:
  kind: github_issues
  kind: linear
"#,
        );

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "$.source.kind");
    }
}
