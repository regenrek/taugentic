use ta_protocol::wire::RunStatus;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("duplicate {entity} record for key {key}")]
    DuplicateRecord { entity: &'static str, key: String },
    #[error("artifact kind {kind} is incompatible with metadata {metadata}")]
    ArtifactMetadataMismatch { kind: String, metadata: String },
    #[error("{entity} record does not exist for key {key}")]
    MissingRecord { entity: &'static str, key: String },
    #[error(
        "auth profile preference order {order} is outside its provider-method group of {group_len}"
    )]
    AuthProfilePreferenceOrderOutOfRange { order: u32, group_len: usize },
    #[error("committed {entity} session mismatch: expected {expected}, got {actual}")]
    CommitSessionMismatch {
        entity: &'static str,
        expected: String,
        actual: String,
    },
    #[error("committed {entity} requires run status {expected:?}, got {actual:?}")]
    CommitRunStatusMismatch {
        entity: &'static str,
        expected: RunStatus,
        actual: RunStatus,
    },
    #[error("committed run event run id mismatch: expected {expected}, got {actual}")]
    CommitRunEventMismatch { expected: String, actual: String },
    #[error("execution context is immutable after run creation: {run_id}")]
    ImmutableRunExecutionContext { run_id: String },
    #[error("run source route is immutable after run creation: {run_id}")]
    ImmutableRunSourceRoute { run_id: String },
    #[error("auth profile mutation does not match the immutable run route: {run_id}")]
    AuthProfileMutationRouteMismatch { run_id: String },
    #[error("auth profile mutation lacks its matching terminal exhausted status: {run_id}")]
    AuthProfileMutationMissingTerminalStatus { run_id: String },
    #[error("committed store transition requires at least one event")]
    EmptyCommitEvents,
    #[error("scheduled work validation failed: {detail}")]
    ScheduledWorkValidation { detail: String },
    #[error("scheduled work occurrence {occurrence_id} is not pending")]
    ScheduledWorkOccurrenceNotPending { occurrence_id: String },
    #[error("scheduled work occurrence {occurrence_id} is not claimed by run {run_id}")]
    ScheduledWorkOccurrenceClaimMismatch {
        occurrence_id: String,
        run_id: String,
    },
    #[error("scheduled work run source does not match occurrence {occurrence_id}")]
    ScheduledWorkRunSourceMismatch { occurrence_id: String },
    #[error("session insert requires existing workspace; workspace_id {workspace_id} unknown")]
    SessionWorkspaceMissing { workspace_id: String },
    #[error("invalid approval lifecycle for {approval_id}: {detail}")]
    ApprovalLifecycleViolation { approval_id: String, detail: String },
    #[error("invalid agent turn projection state: {detail}")]
    AgentTurnProjectionViolation { detail: String },
    #[error("invalid receipt provenance: {message}")]
    InvalidProvenance { message: String },
    #[error("invalid receipt transition for {receipt_id}: {detail}")]
    ReceiptTransitionViolation { receipt_id: String, detail: String },
    #[error("system clock is before unix epoch for receipt timestamp")]
    ReceiptClockBeforeUnixEpoch,
    #[error("receipt timestamp is out of range")]
    ReceiptTimestampOutOfRange,
    #[error("failed to create store directory at {path}")]
    CreateStoreParentDirectory {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to open sqlite store at {path}")]
    OpenStore {
        path: std::path::PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("failed to prepare sqlite store at {path}")]
    PrepareStore {
        path: std::path::PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("sqlite integrity check failed at {path}: {result}")]
    IntegrityCheckFailed {
        path: std::path::PathBuf,
        result: String,
    },
    #[error("sqlite foreign key check failed at {path}: {detail}")]
    ForeignKeyCheckFailed {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("sqlite store missing required schema object at {path}: {kind} {name}")]
    MissingSchemaObject {
        path: std::path::PathBuf,
        kind: &'static str,
        name: &'static str,
    },
    #[error("sqlite store schema shape mismatch at {path} for {table}: {detail}")]
    SchemaShapeMismatch {
        path: std::path::PathBuf,
        table: &'static str,
        detail: String,
    },
    #[error("failed to query {entity} from sqlite store")]
    QueryStore {
        entity: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("failed to encode {entity} for sqlite store")]
    EncodeRecord {
        entity: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to decode {entity} from sqlite store")]
    DecodeRecord {
        entity: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

impl PartialEq for StoreError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::DuplicateRecord {
                    entity: left_entity,
                    key: left_key,
                },
                Self::DuplicateRecord {
                    entity: right_entity,
                    key: right_key,
                },
            ) => left_entity == right_entity && left_key == right_key,
            (
                Self::ArtifactMetadataMismatch {
                    kind: left_kind,
                    metadata: left_metadata,
                },
                Self::ArtifactMetadataMismatch {
                    kind: right_kind,
                    metadata: right_metadata,
                },
            ) => left_kind == right_kind && left_metadata == right_metadata,
            (
                Self::MissingRecord {
                    entity: left_entity,
                    key: left_key,
                },
                Self::MissingRecord {
                    entity: right_entity,
                    key: right_key,
                },
            ) => left_entity == right_entity && left_key == right_key,
            (
                Self::AuthProfilePreferenceOrderOutOfRange {
                    order: left_order,
                    group_len: left_group_len,
                },
                Self::AuthProfilePreferenceOrderOutOfRange {
                    order: right_order,
                    group_len: right_group_len,
                },
            ) => left_order == right_order && left_group_len == right_group_len,
            (
                Self::CommitSessionMismatch {
                    entity: left_entity,
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::CommitSessionMismatch {
                    entity: right_entity,
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => {
                left_entity == right_entity
                    && left_expected == right_expected
                    && left_actual == right_actual
            }
            (
                Self::CommitRunStatusMismatch {
                    entity: left_entity,
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::CommitRunStatusMismatch {
                    entity: right_entity,
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => {
                left_entity == right_entity
                    && left_expected == right_expected
                    && left_actual == right_actual
            }
            (
                Self::CommitRunEventMismatch {
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::CommitRunEventMismatch {
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => left_expected == right_expected && left_actual == right_actual,
            (
                Self::ImmutableRunExecutionContext {
                    run_id: left_run_id,
                },
                Self::ImmutableRunExecutionContext {
                    run_id: right_run_id,
                },
            ) => left_run_id == right_run_id,
            (Self::EmptyCommitEvents, Self::EmptyCommitEvents) => true,
            (
                Self::SessionWorkspaceMissing {
                    workspace_id: left_workspace_id,
                },
                Self::SessionWorkspaceMissing {
                    workspace_id: right_workspace_id,
                },
            ) => left_workspace_id == right_workspace_id,
            (
                Self::ApprovalLifecycleViolation {
                    approval_id: left_approval_id,
                    detail: left_detail,
                },
                Self::ApprovalLifecycleViolation {
                    approval_id: right_approval_id,
                    detail: right_detail,
                },
            ) => left_approval_id == right_approval_id && left_detail == right_detail,
            (
                Self::AgentTurnProjectionViolation {
                    detail: left_detail,
                },
                Self::AgentTurnProjectionViolation {
                    detail: right_detail,
                },
            ) => left_detail == right_detail,
            (
                Self::InvalidProvenance {
                    message: left_message,
                },
                Self::InvalidProvenance {
                    message: right_message,
                },
            ) => left_message == right_message,
            (
                Self::ReceiptTransitionViolation {
                    receipt_id: left_receipt_id,
                    detail: left_detail,
                },
                Self::ReceiptTransitionViolation {
                    receipt_id: right_receipt_id,
                    detail: right_detail,
                },
            ) => left_receipt_id == right_receipt_id && left_detail == right_detail,
            (Self::ReceiptClockBeforeUnixEpoch, Self::ReceiptClockBeforeUnixEpoch) => true,
            (Self::ReceiptTimestampOutOfRange, Self::ReceiptTimestampOutOfRange) => true,
            (
                Self::CreateStoreParentDirectory {
                    path: left_path, ..
                },
                Self::CreateStoreParentDirectory {
                    path: right_path, ..
                },
            ) => left_path == right_path,
            (
                Self::OpenStore {
                    path: left_path, ..
                },
                Self::OpenStore {
                    path: right_path, ..
                },
            ) => left_path == right_path,
            (
                Self::PrepareStore {
                    path: left_path, ..
                },
                Self::PrepareStore {
                    path: right_path, ..
                },
            ) => left_path == right_path,
            (
                Self::IntegrityCheckFailed {
                    path: left_path,
                    result: left_result,
                },
                Self::IntegrityCheckFailed {
                    path: right_path,
                    result: right_result,
                },
            ) => left_path == right_path && left_result == right_result,
            (
                Self::ForeignKeyCheckFailed {
                    path: left_path,
                    detail: left_detail,
                },
                Self::ForeignKeyCheckFailed {
                    path: right_path,
                    detail: right_detail,
                },
            ) => left_path == right_path && left_detail == right_detail,
            (
                Self::MissingSchemaObject {
                    path: left_path,
                    kind: left_kind,
                    name: left_name,
                },
                Self::MissingSchemaObject {
                    path: right_path,
                    kind: right_kind,
                    name: right_name,
                },
            ) => left_path == right_path && left_kind == right_kind && left_name == right_name,
            (
                Self::SchemaShapeMismatch {
                    path: left_path,
                    table: left_table,
                    detail: left_detail,
                },
                Self::SchemaShapeMismatch {
                    path: right_path,
                    table: right_table,
                    detail: right_detail,
                },
            ) => {
                left_path == right_path && left_table == right_table && left_detail == right_detail
            }
            (
                Self::QueryStore {
                    entity: left_entity,
                    ..
                },
                Self::QueryStore {
                    entity: right_entity,
                    ..
                },
            ) => left_entity == right_entity,
            (
                Self::EncodeRecord {
                    entity: left_entity,
                    ..
                },
                Self::EncodeRecord {
                    entity: right_entity,
                    ..
                },
            ) => left_entity == right_entity,
            (
                Self::DecodeRecord {
                    entity: left_entity,
                    ..
                },
                Self::DecodeRecord {
                    entity: right_entity,
                    ..
                },
            ) => left_entity == right_entity,
            _ => false,
        }
    }
}

impl Eq for StoreError {}
