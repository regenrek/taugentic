mod support;

use support::*;
use taugentic_agent::ExecutionError;
use taugentic_agent::session::Session;
use taugentic_agent::tools::Registry;

#[test]
fn tool_list_lock() {
    let mut registry = Registry::with_read_only_builtins();
    let session = Session::new(&request());

    let lock_result = session.lock_tool_list_if_unlocked(&mut registry);
    assert!(lock_result.is_ok(), "{lock_result:?}");
    let add_result = registry.add(EchoTool);

    assert!(matches!(add_result, Err(ExecutionError::ToolListLocked(_))));
}
