use serde::{Deserialize, Serialize};

pub(crate) const METHOD_DAEMON_INTERNAL_STOP: &str = "daemon.internal.stop";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InternalDaemonStopParams {
    pub control_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InternalDaemonStopResult {
    pub stopping: bool,
}
