use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

pub fn check_daemon_foundation(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let mut violations = Vec::new();

    let cli_src = repo_root.join("crates/ta-cli/src");
    collect_literal_matches(
        &cli_src,
        "ta_orchestrator::boundary",
        &mut violations,
        "CLI must consume public daemon contract types from ta-protocol, not ta-orchestrator::boundary",
    )?;

    let desktop_main_src = repo_root.join("apps/desktop/packages/main/src");
    collect_disallowed_create_connection_matches(
        &desktop_main_src,
        &[
            desktop_main_src.join("daemon-rpc-client.ts"),
            desktop_main_src.join("daemon-session.ts"),
        ],
        &mut violations,
    )?;

    let desktop_rpc = desktop_main_src.join("rpc.ts");
    require_literal(
        &desktop_rpc,
        "listSessions: listDaemonSessions,",
        &mut violations,
        "desktop listSessions IPC must route to daemon-session canonical client",
    )?;
    require_literal(
        &desktop_rpc,
        "listRuns: listDaemonRuns,",
        &mut violations,
        "desktop listRuns IPC must route to daemon-session canonical client",
    )?;

    if violations.is_empty() {
        println!("daemon foundation checks passed");
        return Ok(());
    }

    for violation in &violations {
        eprintln!("{violation}");
    }

    Err("daemon foundation drift detected".into())
}

fn collect_literal_matches(
    root: &Path,
    pattern: &str,
    violations: &mut Vec<String>,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    for path in source_files(root)? {
        let contents = fs::read_to_string(&path)?;
        if contents.contains(pattern) {
            violations.push(format!("{}: {} ({message})", path.display(), pattern));
        }
    }

    Ok(())
}

fn collect_disallowed_create_connection_matches(
    root: &Path,
    allowed_paths: &[PathBuf],
    violations: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let allowed_paths = allowed_paths.iter().collect::<BTreeSet<_>>();
    for path in source_files(root)? {
        let contents = fs::read_to_string(&path)?;
        if !contents.contains("createConnection(") {
            continue;
        }
        if allowed_paths.contains(&path) {
            continue;
        }

        violations.push(format!(
            "{}: createConnection( (desktop main must open daemon sockets only through daemon-rpc-client.ts or daemon-session.ts)",
            path.display()
        ));
    }

    Ok(())
}

fn require_literal(
    path: &Path,
    pattern: &str,
    violations: &mut Vec<String>,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    if !contents.contains(pattern) {
        violations.push(format!(
            "{}: missing `{pattern}` ({message})",
            path.display()
        ));
    }

    Ok(())
}

fn source_files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    collect_source_files(root, &mut files)?;
    Ok(files)
}

fn collect_source_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_source_files(&path, files)?;
            continue;
        }

        let extension = path.extension().and_then(|value| value.to_str());
        if matches!(extension, Some("rs" | "ts")) {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{collect_disallowed_create_connection_matches, require_literal};

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("xtask-foundation-{prefix}-{nanos}"))
    }

    #[test]
    fn require_literal_reports_missing_pattern() {
        let dir = temp_dir("require-literal");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("rpc.ts");
        fs::write(&file, "export function example() {}\n").expect("fixture should write");

        let mut violations = Vec::new();
        require_literal(
            &file,
            "listSessions: listDaemonSessions,",
            &mut violations,
            "message",
        )
        .expect("require literal should succeed");

        assert_eq!(violations.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn create_connection_guard_allows_only_canonical_desktop_files() {
        let dir = temp_dir("create-connection");
        fs::create_dir_all(&dir).expect("temp dir should exist");
        let allowed = dir.join("daemon-session.ts");
        let forbidden = dir.join("rpc.ts");
        fs::write(&allowed, "const socket = createConnection(path);\n")
            .expect("allowed fixture should write");
        fs::write(&forbidden, "const socket = createConnection(path);\n")
            .expect("forbidden fixture should write");

        let mut violations = Vec::new();
        collect_disallowed_create_connection_matches(&dir, &[allowed], &mut violations)
            .expect("guard should run");

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("rpc.ts"));
        let _ = fs::remove_dir_all(dir);
    }
}
