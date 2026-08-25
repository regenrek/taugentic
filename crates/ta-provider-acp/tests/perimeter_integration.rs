#![cfg(target_os = "macos")]

mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ta_provider_acp::{
    descriptor::{AcpLaunchKind, AcpProviderSpec},
    launch::build_perimeter_profile,
};

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/ta-provider-acp")
        .join(format!("{prefix}-{nanos}"))
}

fn test_perimeter_profile(work_dir: &Path, command: &Path) -> ta_exec::SandboxProfile {
    let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
    build_perimeter_profile(&provider, &support::execution_context(work_dir), command)
        .expect("test ACP perimeter profile")
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn perimeter_profile_spawns_mock_program_with_ta_exec() {
    use ta_exec::{ExecEngine, LocalExecEngine, SpawnRequest, StdioPolicy};

    let dir = unique_dir("perimeter-echo");
    fs::create_dir_all(&dir).expect("temp dir");
    let profile = test_perimeter_profile(&dir, Path::new("/bin/echo"));
    let child = LocalExecEngine
        .spawn(
            SpawnRequest::new("/bin/echo")
                .arg("ok")
                .cwd(dir.clone())
                .stdin(StdioPolicy::Null)
                .stdout(StdioPolicy::Piped)
                .stderr(StdioPolicy::Null)
                .sandbox_profile(profile),
        )
        .expect("spawn sandboxed echo");

    let output = child.wait_with_output().await.expect("wait echo");
    let _ = fs::remove_dir_all(&dir);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");
}
