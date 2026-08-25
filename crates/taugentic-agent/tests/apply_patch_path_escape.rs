use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use taugentic_agent::patch::{
    AppliedPatch, ApplyError, FileChange, FileChangeKind, WriteError, write_applied_patch,
};
use taugentic_agent::tools::{ApplyPatchTool, Tool, ToolContext};
use tempfile::tempdir;

type TestResult = Result<(), Box<dyn Error>>;

#[cfg(unix)]
#[tokio::test]
async fn model_facing_apply_patch_rejects_symlink_escape_without_partial_writes() -> TestResult {
    let dir = tempdir()?;
    let outside_dir = tempdir()?;
    std::os::unix::fs::symlink(outside_dir.path(), dir.path().join("link"))?;
    let patch = "\
*** Begin Patch
*** Add File: link/outside.txt
+escape
*** Add File: inside.txt
+inside
*** End Patch
";

    let error = ApplyPatchTool
        .run(json!({ "input": patch }), context(dir.path()))
        .await
        .expect_err("symlink escape must fail");

    assert!(error.to_string().contains("path escapes workdir"));
    assert!(!outside_dir.path().join("outside.txt").exists());
    assert!(!dir.path().join("inside.txt").exists());
    Ok(())
}

#[test]
fn apply_patch_writer_rejects_path_escape_without_partial_writes() -> TestResult {
    let dir = tempdir()?;
    let outside_name = format!(
        "{}-outside.txt",
        dir.path()
            .file_name()
            .ok_or("tempdir has no filename")?
            .to_string_lossy()
    );
    let outside = dir
        .path()
        .parent()
        .ok_or("tempdir has no parent")?
        .join(&outside_name);
    let applied = AppliedPatch {
        changed_files: vec![
            FileChange {
                path: PathBuf::from(format!("../{outside_name}")),
                move_to: None,
                kind: FileChangeKind::Added,
                old_content: None,
                new_content: Some("escape\n".to_string()),
                added_lines: 1,
                removed_lines: 0,
            },
            FileChange {
                path: PathBuf::from("inside.txt"),
                move_to: None,
                kind: FileChangeKind::Added,
                old_content: None,
                new_content: Some("inside\n".to_string()),
                added_lines: 1,
                removed_lines: 0,
            },
        ],
        added_lines: 2,
        removed_lines: 0,
        diff: String::new(),
    };

    let error = write_applied_patch(&applied, dir.path()).expect_err("escape must fail");
    assert!(matches!(&error, WriteError::PathEscape { .. }));
    assert!(matches!(
        error.to_apply_error(),
        Some(ApplyError::PathEscape { .. })
    ));
    assert!(!outside.exists());
    assert!(!dir.path().join("inside.txt").exists());
    Ok(())
}

fn context(path: &std::path::Path) -> ToolContext {
    tool_support::context(path, Duration::from_secs(5))
}
mod tool_support;
