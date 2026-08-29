use std::fs;

use ta_protocol::wire::{
    InspectPluginPackageRequest, InstallPluginPackageRequest, InstallPluginPackageResult,
    ListPluginInstallationsResult, METHOD_DAEMON_PLUGIN_INSPECT, METHOD_DAEMON_PLUGIN_INSTALL,
    METHOD_DAEMON_PLUGIN_LIST, METHOD_DAEMON_PLUGIN_UNINSTALL, PluginCapability, PluginInspection,
    UninstallPluginRequest,
};

use super::*;

fn request(id: i64, method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: crate::RequestId::Integer(id),
        method: method.to_string(),
        params: Some(params),
    }
}

fn write_package(root: &std::path::Path) {
    fs::create_dir_all(root).expect("package directory");
    fs::write(
        root.join("manifest.json"),
        r#"{"id":"example.plugin","version":"1.2.3","entrypoint":"plugin.js","capabilities":["workspaceRead"]}"#,
    )
    .expect("manifest");
    fs::write(root.join("plugin.js"), "export default 1;").expect("entrypoint");
}

#[test]
fn plugin_rpc_is_principal_scoped_and_supports_install_list_uninstall() {
    with_test_config_home("plugin-rpc", || {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        write_package(&source);
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let first_session = test_session();
        let first_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        initialize_client(
            &state,
            &shutdown_requested,
            &first_session,
            &first_state,
            TEST_CLIENT_NAME,
        );
        let source_path = source.display().to_string();
        let inspection: PluginInspection = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &first_session,
                &first_state,
                request(
                    901,
                    METHOD_DAEMON_PLUGIN_INSPECT,
                    serde_json::to_value(InspectPluginPackageRequest {
                        source_path: source_path.clone(),
                    })
                    .expect("inspect params"),
                ),
            )
            .expect("inspect response"),
        )
        .expect("typed inspection");
        let installed: InstallPluginPackageResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &first_session,
                &first_state,
                request(
                    902,
                    METHOD_DAEMON_PLUGIN_INSTALL,
                    serde_json::to_value(InstallPluginPackageRequest {
                        source_path: source_path.clone(),
                        inspection: inspection.clone(),
                        granted_capabilities: vec![PluginCapability::WorkspaceRead],
                    })
                    .expect("install params"),
                ),
            )
            .expect("install response"),
        )
        .expect("typed installation");
        assert!(
            !serde_json::to_string(&installed)
                .expect("serialize installation")
                .contains(&source_path)
        );

        let first_list: ListPluginInstallationsResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &first_session,
                &first_state,
                request(903, METHOD_DAEMON_PLUGIN_LIST, serde_json::json!({})),
            )
            .expect("first list response"),
        )
        .expect("typed first list");
        assert_eq!(
            first_list.installations,
            vec![installed.installation.clone()]
        );

        let second_session = test_session();
        let second_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        initialize_client(
            &state,
            &shutdown_requested,
            &second_session,
            &second_state,
            "other-plugin-client",
        );
        let second_list: ListPluginInstallationsResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &second_session,
                &second_state,
                request(904, METHOD_DAEMON_PLUGIN_LIST, serde_json::json!({})),
            )
            .expect("second list response"),
        )
        .expect("typed second list");
        assert!(second_list.installations.is_empty());

        handle_request(
            &state,
            &shutdown_requested,
            &first_session,
            &first_state,
            request(
                905,
                METHOD_DAEMON_PLUGIN_UNINSTALL,
                serde_json::to_value(UninstallPluginRequest {
                    plugin_id: installed.installation.plugin_id,
                    version: installed.installation.version,
                    digest_sha256: installed.installation.digest_sha256,
                })
                .expect("uninstall params"),
            ),
        )
        .expect("uninstall response");
        let after_uninstall: ListPluginInstallationsResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &first_session,
                &first_state,
                request(906, METHOD_DAEMON_PLUGIN_LIST, serde_json::json!({})),
            )
            .expect("post uninstall list response"),
        )
        .expect("typed post uninstall list");
        assert!(after_uninstall.installations.is_empty());
    });
}
