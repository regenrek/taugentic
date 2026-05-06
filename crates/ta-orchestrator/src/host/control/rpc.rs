use serde::Serialize;

use crate::{
    JsonRpcHandlerResult, daemon_control_status,
    host::{bootstrap::BootstrapState, control::runtime_control::observe_runtime_control_state},
    internal_error, read_persisted_runtime_control_state,
};

pub(crate) fn handle_control_status_request(state: &BootstrapState) -> JsonRpcHandlerResult {
    let control_plane =
        read_persisted_runtime_control_state().map_err(|error| internal_error(error.to_string()));
    let observed =
        observe_runtime_control_state(state).map_err(|error| internal_error(error.to_string()));
    match (control_plane, observed) {
        (Ok(control_plane), Ok(observed)) => {
            json_result(daemon_control_status(&control_plane, &observed))
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

fn json_result<T>(value: T) -> JsonRpcHandlerResult
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|error| internal_error(error.to_string()))
}
