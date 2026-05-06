use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use taugentic_agent::patch::{FileChangeKind, apply_patch, parse_patch};
use tempfile::tempdir;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn codex_style_patch_scenarios() -> TestResult {
    let cases = [
        Scenario::success(
            "001_add_file",
            [],
            "*** Begin Patch\n*** Add File: bar.md\n+This is a new file\n*** End Patch\n",
            [("bar.md", "This is a new file\n")],
        ),
        Scenario::success(
            "002_multiple_operations",
            [
                ("delete.txt", "obsolete\n"),
                ("modify.txt", "line1\nline2\n"),
            ],
            "*** Begin Patch\n*** Add File: nested/new.txt\n+created\n*** Delete File: delete.txt\n*** Update File: modify.txt\n@@\n-line2\n+changed\n*** End Patch\n",
            [
                ("modify.txt", "line1\nchanged\n"),
                ("nested/new.txt", "created\n"),
            ],
        ),
        Scenario::success(
            "004_move_to_new_directory",
            [
                ("old/name.txt", "old content\n"),
                ("old/other.txt", "unrelated file\n"),
            ],
            "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch\n",
            [
                ("old/other.txt", "unrelated file\n"),
                ("renamed/dir/name.txt", "new content\n"),
            ],
        ),
        Scenario::apply_error(
            "005_rejects_empty_patch",
            [("foo.txt", "stable\n")],
            "*** Begin Patch\n*** End Patch\n",
        ),
        Scenario::apply_error(
            "006_rejects_missing_context",
            [("modify.txt", "line1\nline2\n")],
            "*** Begin Patch\n*** Update File: modify.txt\n@@\n-missing\n+changed\n*** End Patch\n",
        ),
        Scenario::apply_error(
            "007_rejects_missing_file_delete",
            [("foo.txt", "stable\n")],
            "*** Begin Patch\n*** Delete File: missing.txt\n*** End Patch\n",
        ),
        Scenario::parse_error(
            "013_rejects_invalid_hunk_header",
            [("foo.txt", "stable\n")],
            "*** Begin Patch\n*** Frobnicate File: foo\n*** End Patch\n",
        ),
        Scenario::success(
            "014_update_file_appends_trailing_newline",
            [("no_newline.txt", "no newline at end\n")],
            "*** Begin Patch\n*** Update File: no_newline.txt\n@@\n-no newline at end\n+first line\n+second line\n*** End Patch\n",
            [("no_newline.txt", "first line\nsecond line\n")],
        ),
        Scenario::success(
            "016_pure_addition_update_chunk",
            [("input.txt", "line1\nline2\n")],
            "*** Begin Patch\n*** Update File: input.txt\n@@\n+added line 1\n+added line 2\n*** End Patch\n",
            [("input.txt", "line1\nline2\nadded line 1\nadded line 2\n")],
        ),
        Scenario::success(
            "017_whitespace_padded_hunk_header",
            [("foo.txt", "old\n")],
            "*** Begin Patch\n  *** Update File: foo.txt\n@@\n-old\n+new\n*** End Patch\n",
            [("foo.txt", "new\n")],
        ),
        Scenario::success(
            "018_whitespace_padded_patch_markers",
            [("file.txt", "one\n")],
            " *** Begin Patch\n*** Update File: file.txt\n@@\n-one\n+two\n*** End Patch \n",
            [("file.txt", "two\n")],
        ),
        Scenario::success(
            "019_unicode_simple",
            [("foo.txt", "line1\nnaïve café\nline3\n")],
            "*** Begin Patch\n*** Update File: foo.txt\n@@\n line1\n-naïve café\n+naïve café ✅\n*** End Patch\n",
            [("foo.txt", "line1\nnaïve café ✅\nline3\n")],
        ),
        Scenario::success(
            "020_delete_file_success",
            [("keep.txt", "keep\n"), ("obsolete.txt", "obsolete\n")],
            "*** Begin Patch\n*** Delete File: obsolete.txt\n*** End Patch\n",
            [("keep.txt", "keep\n")],
        ),
        Scenario::success(
            "022_update_file_end_of_file_marker",
            [("tail.txt", "first\nsecond\n")],
            "*** Begin Patch\n*** Update File: tail.txt\n@@\n first\n-second\n+second updated\n*** End of File\n*** End Patch\n",
            [("tail.txt", "first\nsecond updated\n")],
        ),
    ];

    for case in cases {
        run_scenario(case)?;
    }
    Ok(())
}

#[test]
fn pure_apply_does_not_mutate_disk() -> TestResult {
    let dir = tempdir()?;
    let patch =
        parse_patch("*** Begin Patch\n*** Add File: created.txt\n+created\n*** End Patch\n")?;
    let applied = apply_patch(&patch, dir.path())?;
    assert_eq!(applied.changed_files.len(), 1);
    assert!(!dir.path().join("created.txt").exists());
    Ok(())
}

struct Scenario {
    name: &'static str,
    input: Vec<(&'static str, &'static str)>,
    patch: &'static str,
    expected: Option<Vec<(&'static str, &'static str)>>,
    parse_error: bool,
}

impl Scenario {
    fn success<const I: usize, const E: usize>(
        name: &'static str,
        input: [(&'static str, &'static str); I],
        patch: &'static str,
        expected: [(&'static str, &'static str); E],
    ) -> Self {
        Self {
            name,
            input: input.to_vec(),
            patch,
            expected: Some(expected.to_vec()),
            parse_error: false,
        }
    }

    fn apply_error<const I: usize>(
        name: &'static str,
        input: [(&'static str, &'static str); I],
        patch: &'static str,
    ) -> Self {
        Self {
            name,
            input: input.to_vec(),
            patch,
            expected: None,
            parse_error: false,
        }
    }

    fn parse_error<const I: usize>(
        name: &'static str,
        input: [(&'static str, &'static str); I],
        patch: &'static str,
    ) -> Self {
        Self {
            name,
            input: input.to_vec(),
            patch,
            expected: None,
            parse_error: true,
        }
    }
}

fn run_scenario(case: Scenario) -> TestResult {
    let dir = tempdir()?;
    write_input(dir.path(), &case.input)?;
    let parsed = parse_patch(case.patch);
    if case.parse_error {
        assert!(
            parsed.is_err(),
            "{} should reject during parsing",
            case.name
        );
        return Ok(());
    }

    let parsed = parsed?;
    let result = apply_patch(&parsed, dir.path());
    let Some(expected) = case.expected else {
        assert!(result.is_err(), "{} should reject during apply", case.name);
        return Ok(());
    };
    let applied = result?;
    let actual = materialize_virtual_state(case.input, &applied.changed_files);
    assert_eq!(
        actual,
        tuples_to_map(&expected),
        "{} final state",
        case.name
    );
    assert!(!applied.diff.is_empty(), "{} diff is empty", case.name);
    Ok(())
}

fn write_input(root: &Path, files: &[(&str, &str)]) -> TestResult {
    for (path, content) in files {
        let absolute = root.join(path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(absolute, content)?;
    }
    Ok(())
}

fn materialize_virtual_state(
    input: Vec<(&'static str, &'static str)>,
    changes: &[taugentic_agent::patch::FileChange],
) -> BTreeMap<PathBuf, String> {
    let mut state = input
        .into_iter()
        .map(|(path, content)| (PathBuf::from(path), content.to_string()))
        .collect::<BTreeMap<_, _>>();
    for change in changes {
        match change.kind {
            FileChangeKind::Added => {
                if let Some(content) = &change.new_content {
                    state.insert(change.path.clone(), content.clone());
                }
            }
            FileChangeKind::Deleted => {
                state.remove(&change.path);
            }
            FileChangeKind::Updated => {
                state.remove(&change.path);
                if let Some(content) = &change.new_content {
                    state.insert(
                        change
                            .move_to
                            .clone()
                            .unwrap_or_else(|| change.path.clone()),
                        content.clone(),
                    );
                }
            }
        }
    }
    state
}

fn tuples_to_map(files: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
    files
        .iter()
        .map(|(path, content)| (PathBuf::from(path), (*content).to_string()))
        .collect()
}
