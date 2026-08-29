mod support;

use std::os::unix::fs::PermissionsExt;

use ta_protocol::wire::{WorkspaceFileAttachment, WorkspaceFileKind};
use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
#[cfg(unix)]
fn image_generation_emits_one_typed_image_publication() {
    let binary_dir = support::sandbox_safe_temp_dir("codex-app-server-image");
    let script = binary_dir.path().join("codex");
    std::fs::write(
        &script,
        r##"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        print(json.dumps({"id": message["id"], "result": {}}), flush=True)
    elif method == "thread/start":
        print(json.dumps({"id": message["id"], "result": {"thread": {"id": "thread-1"}}}), flush=True)
    elif method == "turn/start":
        input = message["params"]["input"]
        if len(input) != 2 or input[1].get("type") != "localImage":
            sys.exit(2)
        print(json.dumps({"id": message["id"], "result": {"turn": {"id": "turn-1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"turnId": "turn-1", "item": {"type": "imageGeneration", "id": "image-1", "status": "completed", "result": {"text": "iVBORw0KGgo=", "truncated": False}}}}), flush=True)
        print(json.dumps({"method": "turn/completed", "params": {"turn": {"id": "turn-1", "error": None}}}), flush=True)
"##,
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).expect("permissions");

    let mut request = support::request();
    support::configure_codex_app_server_request(&mut request);
    support::set_request_cwd(&mut request, binary_dir.path());
    request.attachments = vec![WorkspaceFileAttachment {
        path: "input.png".to_string(),
        revision: "sha256:fixture".to_string(),
        kind: WorkspaceFileKind::Image,
        byte_len: 8,
    }];
    let sink = support::TestSink::new();
    let handle = dispatch_with_client(
        request,
        sink.clone(),
        CodexAppServerClient::with_binary(script),
    )
    .expect("dispatch");
    sink.wait_for_completion();
    drop(handle);
    assert_eq!(
        sink.artifacts(),
        vec![(
            ta_protocol::wire::ArtifactKind::Image,
            "iVBORw0KGgo=".to_string()
        )]
    );
}
