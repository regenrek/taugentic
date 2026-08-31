use ta_protocol::wire::{
    BrowserActionDecision, BrowserActionKind, BrowserActionRequest, BrowserProfileRequest,
    BrowserProfileResult, METHOD_DAEMON_BROWSER_ACTION, METHOD_DAEMON_BROWSER_PROFILE,
};

use super::*;

fn request(id: i64, method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: crate::RequestId::Integer(id),
        method: method.into(),
        params: Some(params),
    }
}

#[test]
fn browser_rpc_issues_public_profile_and_denies_download_for_the_authenticated_principal() {
    with_test_config_home("browser-rpc", || {
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let session = test_session();
        let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        initialize_client(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            TEST_CLIENT_NAME,
        );
        let profile: BrowserProfileResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request(
                    1,
                    METHOD_DAEMON_BROWSER_PROFILE,
                    serde_json::to_value(BrowserProfileRequest {}).expect("profile params"),
                ),
            )
            .expect("profile response"),
        )
        .expect("typed profile");
        let serialized = serde_json::to_string(&profile).expect("public profile should serialize");
        assert!(!serialized.contains("principal"));
        let action = BrowserActionRequest {
            request_id: "download".into(),
            profile_id: profile.profile.id,
            kind: BrowserActionKind::DownloadDestination,
            navigation: None,
            should_perform_download: None,
            can_show_mime_type: None,
        };
        let result: ta_protocol::wire::BrowserActionResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request(
                    2,
                    METHOD_DAEMON_BROWSER_ACTION,
                    serde_json::to_value(action).expect("action params"),
                ),
            )
            .expect("action response"),
        )
        .expect("typed action");
        assert_eq!(result.decision, BrowserActionDecision::Cancel);
        assert_eq!(
            result.reason.as_deref(),
            Some("Downloads are not available yet.")
        );
    });
}

#[test]
fn browser_rpc_cancels_a_stale_profile_action_instead_of_leaving_it_unresolved() {
    with_test_config_home("browser-rpc-stale-profile", || {
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let session = test_session();
        let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        initialize_client(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            TEST_CLIENT_NAME,
        );
        let _: BrowserProfileResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request(
                    1,
                    METHOD_DAEMON_BROWSER_PROFILE,
                    serde_json::to_value(BrowserProfileRequest {}).expect("profile params"),
                ),
            )
            .expect("profile response"),
        )
        .expect("typed profile");
        let action = BrowserActionRequest {
            request_id: "stale-action".into(),
            profile_id: ta_protocol::wire::BrowserProfileId::new("stale-profile")
                .expect("valid stale profile id"),
            kind: BrowserActionKind::NavigationAction,
            navigation: Some(ta_protocol::wire::BrowserNavigationRequest {
                kind: ta_protocol::wire::BrowserNavigationKind::Navigate,
                url: Some("https://example.com".into()),
            }),
            should_perform_download: Some(false),
            can_show_mime_type: None,
        };
        let result: ta_protocol::wire::BrowserActionResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request(
                    2,
                    METHOD_DAEMON_BROWSER_ACTION,
                    serde_json::to_value(action).expect("action params"),
                ),
            )
            .expect("action response"),
        )
        .expect("typed cancellation");
        assert_eq!(result.request_id, "stale-action");
        assert_eq!(result.decision, BrowserActionDecision::Cancel);
        assert_eq!(
            result.reason.as_deref(),
            Some("Browser action is not authorized.")
        );
    });
}

#[test]
fn browser_rpc_delivers_native_download_decisions_for_authorized_actions_and_responses() {
    with_test_config_home("browser-rpc-native-download", || {
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let session = test_session();
        let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        initialize_client(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            TEST_CLIENT_NAME,
        );
        let profile: BrowserProfileResult = serde_json::from_value(
            handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request(
                    1,
                    METHOD_DAEMON_BROWSER_PROFILE,
                    serde_json::to_value(BrowserProfileRequest {}).expect("profile params"),
                ),
            )
            .expect("profile response"),
        )
        .expect("typed profile");

        for (id, request_id, kind, should_perform_download, can_show_mime_type) in [
            (
                2,
                "action-download",
                BrowserActionKind::NavigationAction,
                Some(true),
                None,
            ),
            (
                3,
                "response-download",
                BrowserActionKind::NavigationResponse,
                None,
                Some(false),
            ),
        ] {
            let action = BrowserActionRequest {
                request_id: request_id.into(),
                profile_id: profile.profile.id.clone(),
                kind,
                navigation: Some(ta_protocol::wire::BrowserNavigationRequest {
                    kind: ta_protocol::wire::BrowserNavigationKind::Navigate,
                    url: Some("https://example.com/file".into()),
                }),
                should_perform_download,
                can_show_mime_type,
            };
            let result: ta_protocol::wire::BrowserActionResult = serde_json::from_value(
                handle_request(
                    &state,
                    &shutdown_requested,
                    &session,
                    &session_state,
                    request(
                        id,
                        METHOD_DAEMON_BROWSER_ACTION,
                        serde_json::to_value(action).expect("action params"),
                    ),
                )
                .expect("action response"),
            )
            .expect("typed action");
            assert_eq!(result.request_id, request_id);
            assert_eq!(result.decision, BrowserActionDecision::Download);
            assert_eq!(result.reason, None);
        }
    });
}
