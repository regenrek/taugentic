use std::error::Error;
use std::fs;
use std::time::Duration;

use serde_json::json;
use taugentic_agent::tools::{
    ListDirectoryTool, ReadFileTool, Registry, SearchTool, Tool, ToolContext,
};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn read_file_respects_offset_and_limit() -> TestResult {
    let dir = tempdir()?;
    fs::write(dir.path().join("sample.txt"), "alpha\nbeta\ngamma\n")?;

    let output = ReadFileTool
        .run(
            json!({ "path": "sample.txt", "offset": 1, "limit": 1 }),
            context(dir.path()),
        )
        .await?;

    let content = required_str(&output.content, "content")?;
    assert!(content.contains("2\tbeta"));
    assert!(!content.contains("alpha"));
    assert_eq!(output.content["truncated"], json!(true));
    Ok(())
}

#[tokio::test]
async fn list_directory_supports_recursive_entries() -> TestResult {
    let dir = tempdir()?;
    fs::create_dir_all(dir.path().join("nested"))?;
    fs::write(dir.path().join("nested/file.txt"), "body")?;

    let output = ListDirectoryTool
        .run(
            json!({ "path": ".", "recursive": true }),
            context(dir.path()),
        )
        .await?;
    let entries = output.content["entries"]
        .as_array()
        .ok_or("missing entries")?;
    let paths = entries
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();
    assert!(paths.contains(&"nested"));
    assert!(paths.contains(&"nested/file.txt"));
    Ok(())
}

#[tokio::test]
async fn search_supports_content_and_filename_modes() -> TestResult {
    let dir = tempdir()?;
    fs::write(dir.path().join(".gitignore"), "ignored.txt\n")?;
    fs::write(dir.path().join("needle.txt"), "find this needle\n")?;
    fs::write(dir.path().join("ignored.txt"), "needle ignored\n")?;

    let content = SearchTool
        .run(
            json!({ "query": "needle", "mode": "content", "path": ".", "limit": 10 }),
            context(dir.path()),
        )
        .await?;
    let content_results = content.content["results"]
        .as_array()
        .ok_or("missing content results")?;
    assert!(
        content_results
            .iter()
            .any(|entry| required_str(entry, "line").is_ok_and(|line| line.contains("needle.txt")))
    );
    assert!(
        !content_results
            .iter()
            .any(|entry| required_str(entry, "line").is_ok_and(|line| line.contains("ignored.txt")))
    );

    let filename = SearchTool
        .run(
            json!({ "query": "needle", "mode": "fileName", "path": ".", "limit": 10 }),
            context(dir.path()),
        )
        .await?;
    let filename_results = filename.content["results"]
        .as_array()
        .ok_or("missing filename results")?;
    assert_eq!(filename_results.len(), 1);
    assert_eq!(filename_results[0]["path"], json!("needle.txt"));
    Ok(())
}

#[test]
fn registry_iteration_order_is_deterministic() {
    let registry = Registry::with_read_only_builtins();
    let names = registry.iter().map(|(name, _)| name).collect::<Vec<_>>();
    assert_eq!(names, vec!["list_directory", "read_file", "search"]);
}

fn context(path: &std::path::Path) -> ToolContext {
    ToolContext {
        workdir: path.to_path_buf(),
        cancellation_token: CancellationToken::new(),
        timeout: Duration::from_secs(5),
        parent_turn_id: None,
    }
}

fn required_str<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, Box<dyn Error>> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("missing string field {key}").into())
}
