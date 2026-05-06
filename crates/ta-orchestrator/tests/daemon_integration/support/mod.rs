pub use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub use serde_json::Value;
pub use serde_json::json;
pub use ta_jsonrpc::{
    ClientConfig, INVALID_PARAMS_ERROR_CODE, JsonLineCodec, JsonRpcClient, JsonRpcClientError,
    JsonRpcError, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, METHOD_NOT_FOUND_ERROR_CODE,
    RequestId, SocketConnection, configure_connection_timeouts, connect_socket,
};
pub use ta_observability::{LOG_DIR_ENV_VAR, LOG_FORMAT_ENV_VAR, LOG_STDERR_ENV_VAR};
pub use ta_protocol::wire::{
    ActivityPageQuery, ActivityPageResult, AgentRuntimeSnapshot, ApprovalDecision, ApprovalId,
    ApprovalSnapshotResult, ArtifactSnapshotResult, ArtifactSummary, DAEMON_PROTOCOL_VERSION,
    DAEMON_SOCKET_NAME_ENV_VAR, DaemonAgentRuntimeSelectProfileParams, DaemonApprovalDecideParams,
    DaemonApprovalDecideResult, DaemonClientCapabilities, DaemonControlStatusResult, DaemonEvent,
    DaemonEventCursor, DaemonEventEnvelope, DaemonEventKind, DaemonInitializeParams,
    DaemonInitializeResult, DaemonRuntimeMode, DaemonSessionAttachParams,
    DaemonSessionAttachResult, DaemonSessionOpenParams, DaemonSessionOpenResult,
    DaemonStatusParams, DaemonStatusResult, DaemonSubscribeResult, GetArtifactQuery, GetRunQuery,
    GetSessionQuery, ListApprovalsQuery, ListArtifactsQuery, ListRunsQuery, ListSessionsQuery,
    METHOD_DAEMON_ACTIVITY_PAGE, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_CONTROL_STATUS, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_RUN_GET, METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_REPLAY_EVENTS,
    METHOD_DAEMON_RUN_START, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_GET,
    METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_STATUS,
    METHOD_DAEMON_SUBSCRIBE, PublicActivityPageResult, PublicApprovalEvent, PublicDaemonEvent,
    PublicDaemonEventEnvelope, RunDetail, RunId, RunStatus, RunSummary, RuntimeProfileId,
    SessionAuthority, SessionId, SessionSummary, StartRunCommand, SubscribeRunEventsRequest,
    SubscribeRunEventsResult,
};
pub use ta_store::{
    ArtifactRecord, CheckpointRecord, CommitArtifactPublish, CommitCheckpointPersist,
    CommitRepository, CommitRunTransition, ProjectionRepository, SqliteStore,
};
pub use tungstenite::Message;

pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
pub const POLL_INTERVAL: Duration = Duration::from_millis(50);
pub const DAEMON_RUNTIME_MODE_ENV_VAR: &str = "TAUGENTIC_DAEMON_RUNTIME_MODE";
pub const DAEMON_REMOTE_WS_ENABLED_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_ENABLED";
pub const DAEMON_REMOTE_WS_BIND_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_BIND";
pub const DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_AUTH_TOKEN";

mod api;
mod process;
mod rpc;

pub use api::*;
pub use process::*;
pub use rpc::*;
