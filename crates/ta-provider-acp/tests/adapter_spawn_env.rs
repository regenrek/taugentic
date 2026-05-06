use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ta_provider_acp::{
    adapter::{AcpClientTrace, AcpProcessAdapter, AcpProcessConfig},
    descriptor::{AcpLaunchKind, AcpProviderSpec},
    launch::build_perimeter_profile,
    mode_mapping::ModeMapping,
};

#[tokio::test]
async fn adapter_env_remove_hides_explicit_child_env() {
    let dir = unique_dir("adapter-env-remove");
    fs::create_dir_all(&dir).expect("temp dir");
    let env_file = dir.join("child-env.txt");
    let stub = write_stub(&dir, "stub-acp", env_probe_stub_script());
    let mut config = config(
        &dir,
        stub,
        vec![
            ("ENV_PROBE_FILE".to_string(), env_file.display().to_string()),
            ("TA_TEST_REMOVE".to_string(), "caller-secret".to_string()),
        ],
    );
    config.env_remove = vec!["TA_TEST_REMOVE".to_string()];

    let mut client = AcpProcessAdapter::new(config)
        .spawn(AcpClientTrace {
            run_id: "run".to_string(),
            session_id: "session".to_string(),
        })
        .expect("spawn");
    client.initialize().await.expect("initialize");
    client.shutdown().await.expect("shutdown");

    let child_env = fs::read_to_string(&env_file).expect("env probe file");
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(child_env.trim(), "<unset>");
}

fn config(work_dir: &Path, command: PathBuf, env: Vec<(String, String)>) -> AcpProcessConfig {
    let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
    let sandbox_profile =
        build_perimeter_profile(&provider, work_dir, &command).expect("test ACP perimeter profile");
    AcpProcessConfig {
        flavor_id: "test-acp".to_string(),
        command,
        sandbox_profile,
        args: Vec::new(),
        env,
        env_remove: Vec::new(),
        work_dir: work_dir.to_path_buf(),
        mcp_servers: Vec::new(),
        session_mode_id: None,
        session_model_id: None,
        mode_mapping: ModeMapping::new(),
        cancel_grace: Duration::from_millis(100),
    }
}

fn write_stub(dir: &Path, name: &str, source: String) -> PathBuf {
    let stub = dir.join(name);
    fs::write(&stub, source).expect("stub script");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod");
    stub
}

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/ta-provider-acp")
        .join(format!("{prefix}-{nanos}"))
}

fn env_probe_stub_script() -> String {
    r#"#!/bin/sh
printf '%s\n' "${TA_TEST_REMOVE-<unset>}" > "$ENV_PROBE_FILE"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
  esac
done
"#
    .to_string()
}
