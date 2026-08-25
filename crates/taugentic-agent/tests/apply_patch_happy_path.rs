use std::error::Error;
use std::fs;
use std::time::Duration;

use serde_json::json;
use taugentic_agent::tools::{ApplyPatchTool, Tool, ToolContext};
use tempfile::tempdir;

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn apply_patch_adds_updates_and_deletes_files() -> TestResult {
    let dir = tempdir()?;
    fs::write(dir.path().join("delete.txt"), "obsolete\n")?;
    fs::write(dir.path().join("modify.txt"), "line1\nline2\n")?;

    let patch = "\
*** Begin Patch
*** Add File: nested/new.txt
+created
*** Delete File: delete.txt
*** Update File: modify.txt
@@
-line2
+changed
*** End Patch
";

    let output = ApplyPatchTool
        .run(json!({ "input": patch }), context(dir.path()))
        .await?;

    assert_eq!(
        fs::read_to_string(dir.path().join("nested/new.txt"))?,
        "created\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("modify.txt"))?,
        "line1\nchanged\n"
    );
    assert!(!dir.path().join("delete.txt").exists());
    assert_eq!(output.content["files_added"], json!(["nested/new.txt"]));
    assert_eq!(output.content["files_modified"], json!(["modify.txt"]));
    assert_eq!(output.content["files_deleted"], json!(["delete.txt"]));
    assert!(
        output.content["bytes_written"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
    Ok(())
}

fn context(path: &std::path::Path) -> ToolContext {
    tool_support::context(path, Duration::from_secs(5))
}
mod tool_support;
