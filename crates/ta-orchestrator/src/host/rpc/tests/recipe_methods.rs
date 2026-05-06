use super::*;

#[test]
fn recipes_list_returns_builtin_registry_recipes() {
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

    let result = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(1),
            method: METHOD_DAEMON_RECIPES_LIST.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect("recipes list should succeed");
    let response: RecipeListResponse =
        serde_json::from_value(result).expect("response should decode");

    assert_eq!(response.recipes.len(), 5);
    assert_eq!(response.recipes[0].id, "debug-agent");
    assert!(
        response
            .recipes
            .iter()
            .all(|recipe| !recipe.name.is_empty())
    );
}
