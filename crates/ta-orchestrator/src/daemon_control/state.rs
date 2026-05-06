use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use super::process::process_is_running;
use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    DaemonActualRuntimeMode, DaemonControlAction, DaemonControlErrorCode,
    DaemonControlStatusResult, DaemonPendingTransitionKind, DaemonPendingTransitionView,
    DaemonRuntimeMode, DaemonStatusResult, DaemonTransitionStatus,
};

use crate::{
    BackgroundServiceControlError, BackgroundServiceState, daemon_runtime_mode_file_path,
    host::config::ControlToken, runtime_control_state_file_path,
};

const RUNTIME_CONTROL_LOCK_WAIT: Duration = Duration::from_secs(10);
const RUNTIME_CONTROL_LOCK_POLL: Duration = Duration::from_millis(50);
const RUNTIME_CONTROL_LOCK_STALE_AGE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeControlOwnershipRecord {
    pub runtime_mode: DaemonRuntimeMode,
    pub daemon_instance_id: String,
    pub control_token: ControlToken,
    pub process_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedRuntimeControlState {
    pub background_opt_in: bool,
    pub desired_mode: DaemonRuntimeMode,
    pub pending_transition: Option<PendingTransition>,
    pub last_error: Option<TransitionErrorRecord>,
    pub generation: u64,
}

impl Default for PersistedRuntimeControlState {
    fn default() -> Self {
        Self {
            background_opt_in: false,
            desired_mode: DaemonRuntimeMode::Local,
            pending_transition: None,
            last_error: None,
            generation: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingTransition {
    pub kind: PendingTransitionKind,
    pub op_id: u64,
    pub step: TransitionStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingTransitionKind {
    EnableBackground,
    DisableBackground,
    RecoverToLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransitionStep {
    Prepare,
    StopConflictingRuntime,
    EnsureBackgroundService,
    WaitForBackgroundRuntime,
    StopBackgroundService,
    WaitForBackgroundShutdown,
    ClearOwnership,
    StartLocalRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionErrorRecord {
    pub code: DaemonControlErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeControlObservedState {
    pub daemon_status: Option<DaemonStatusResult>,
    pub background_service: BackgroundServiceState,
    pub ownership: Option<RuntimeControlOwnershipRecord>,
    pub socket_path: String,
    pub log_path: String,
    pub daemon_version: Option<String>,
}

pub struct RuntimeControlMutationLock {
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeControlLockMetadata {
    process_id: Option<u32>,
}

impl Drop for RuntimeControlMutationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn acquire_runtime_control_lock()
-> Result<RuntimeControlMutationLock, BackgroundServiceControlError> {
    let path = runtime_control_lock_file_path();
    ensure_parent_dir(&path)?;
    let start = std::time::Instant::now();
    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "pid={}", std::process::id())
                    .and_then(|()| file.sync_all())
                    .map_err(|source| {
                        let _ = fs::remove_file(&path);
                        BackgroundServiceControlError::WriteFile {
                            path: path.clone(),
                            source,
                        }
                    })?;
                return Ok(RuntimeControlMutationLock { path });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                clear_stale_runtime_control_lock(&path)?;
                if start.elapsed() >= RUNTIME_CONTROL_LOCK_WAIT {
                    return Err(BackgroundServiceControlError::MutationLockTimeout { path });
                }
                thread::sleep(RUNTIME_CONTROL_LOCK_POLL);
            }
            Err(source) => {
                return Err(BackgroundServiceControlError::WriteFile { path, source });
            }
        }
    }
}

pub fn read_persisted_runtime_control_state()
-> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let path = runtime_control_state_file_path();
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PersistedRuntimeControlState::default());
        }
        Err(source) => return Err(BackgroundServiceControlError::ReadFile { path, source }),
    };
    serde_json::from_str(&contents).map_err(|source| {
        BackgroundServiceControlError::ParseControlPlaneFile {
            path: path.clone(),
            source,
        }
    })
}

pub fn start_runtime_control_transition(
    kind: PendingTransitionKind,
    desired_mode: DaemonRuntimeMode,
    background_opt_in: bool,
    step: TransitionStep,
) -> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let mut state = read_persisted_runtime_control_state()?;
    state.generation = state.generation.saturating_add(1);
    state.background_opt_in = background_opt_in;
    state.desired_mode = desired_mode;
    state.pending_transition = Some(PendingTransition {
        kind,
        op_id: state.generation,
        step,
    });
    state.last_error = None;
    write_persisted_runtime_control_state(&state)?;
    Ok(state)
}

pub fn advance_runtime_control_transition(
    step: TransitionStep,
) -> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let mut state = read_persisted_runtime_control_state()?;
    if let Some(pending) = state.pending_transition.as_mut() {
        pending.step = step;
    }
    write_persisted_runtime_control_state(&state)?;
    Ok(state)
}

pub fn fail_runtime_control_transition(
    code: DaemonControlErrorCode,
    message: impl Into<String>,
) -> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let mut state = read_persisted_runtime_control_state()?;
    state.last_error = Some(TransitionErrorRecord {
        code,
        message: message.into(),
    });
    write_persisted_runtime_control_state(&state)?;
    Ok(state)
}

pub fn complete_runtime_control_transition(
    desired_mode: DaemonRuntimeMode,
    background_opt_in: bool,
) -> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let mut state = read_persisted_runtime_control_state()?;
    state.generation = state.generation.saturating_add(1);
    state.desired_mode = desired_mode;
    state.background_opt_in = background_opt_in;
    state.pending_transition = None;
    state.last_error = None;
    write_persisted_runtime_control_state(&state)?;
    Ok(state)
}

pub fn clear_runtime_control_error()
-> Result<PersistedRuntimeControlState, BackgroundServiceControlError> {
    let mut state = read_persisted_runtime_control_state()?;
    state.last_error = None;
    write_persisted_runtime_control_state(&state)?;
    Ok(state)
}

pub fn daemon_control_status(
    control_plane: &PersistedRuntimeControlState,
    observed: &RuntimeControlObservedState,
) -> DaemonControlStatusResult {
    let actual_mode = derive_actual_runtime_mode(control_plane, observed);
    let reconcile_required =
        control_plane.pending_transition.is_some() || control_plane.last_error.is_some();
    let transition_status = derive_transition_status(control_plane);
    let error_code = control_plane
        .last_error
        .as_ref()
        .map(|error| error.code.clone())
        .or({
            if matches!(actual_mode, DaemonActualRuntimeMode::Foreign) {
                Some(DaemonControlErrorCode::ExternalRuntime)
            } else {
                None
            }
        });
    let message = derive_status_message(control_plane, observed, &actual_mode);
    DaemonControlStatusResult {
        background_opt_in: control_plane.background_opt_in,
        desired_mode: control_plane.desired_mode,
        actual_mode,
        transition_status,
        reconcile_required,
        allowed_actions: derive_allowed_actions(control_plane, observed),
        error_code,
        message,
        pending_transition: control_plane
            .pending_transition
            .as_ref()
            .map(public_pending_transition_view),
        socket_path: observed.socket_path.clone(),
        log_path: observed.log_path.clone(),
        daemon_version: observed.daemon_version.clone(),
        protocol_version: ta_protocol::wire::DAEMON_PROTOCOL_VERSION.to_string(),
    }
}

pub fn read_runtime_control_ownership()
-> Result<Option<RuntimeControlOwnershipRecord>, BackgroundServiceControlError> {
    let path = runtime_control_ownership_file_path();
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(BackgroundServiceControlError::ReadFile {
                path: path.clone(),
                source,
            });
        }
    };
    let record =
        serde_json::from_str::<RuntimeControlOwnershipRecord>(&contents).map_err(|source| {
            BackgroundServiceControlError::ParseOwnershipFile {
                path: path.clone(),
                source,
            }
        })?;
    Ok(Some(record))
}

pub fn write_runtime_control_ownership(
    record: &RuntimeControlOwnershipRecord,
) -> Result<(), BackgroundServiceControlError> {
    let path = runtime_control_ownership_file_path();
    write_runtime_control_ownership_file(&path, record)
}

pub fn clear_runtime_control_ownership_if_matches(
    expected: Option<(&str, &ControlToken)>,
) -> Result<bool, BackgroundServiceControlError> {
    let path = runtime_control_ownership_file_path();
    if let Some((expected_daemon_instance_id, expected_control_token)) = expected {
        let Some(current) = read_runtime_control_ownership()? else {
            return Ok(false);
        };
        if current.daemon_instance_id != expected_daemon_instance_id
            || &current.control_token != expected_control_token
        {
            return Ok(false);
        }
    }
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(BackgroundServiceControlError::RemoveFile { path, source }),
    }
}

pub fn runtime_control_ownership_file_path() -> PathBuf {
    daemon_runtime_mode_file_path()
        .parent()
        .expect("runtime mode file should have parent")
        .join("runtime-control-owner.json")
}

fn write_persisted_runtime_control_state(
    state: &PersistedRuntimeControlState,
) -> Result<(), BackgroundServiceControlError> {
    let path = runtime_control_state_file_path();
    ensure_parent_dir(&path)?;
    let contents = serde_json::to_string_pretty(state).map_err(|source| {
        BackgroundServiceControlError::SerializeControlPlaneFile {
            path: path.clone(),
            source,
        }
    })?;
    write_private_file(&path, &format!("{contents}\n"))
}

fn write_runtime_control_ownership_file(
    path: &Path,
    record: &RuntimeControlOwnershipRecord,
) -> Result<(), BackgroundServiceControlError> {
    ensure_parent_dir(path)?;
    let contents = serde_json::to_string_pretty(record).map_err(|source| {
        BackgroundServiceControlError::ParseOwnershipFile {
            path: path.to_path_buf(),
            source,
        }
    })?;
    write_private_file_atomically(path, &format!("{contents}\n"))
}

fn runtime_control_lock_file_path() -> PathBuf {
    runtime_control_state_file_path()
        .parent()
        .expect("runtime control state file should have parent")
        .join("runtime-control.lock")
}

fn clear_stale_runtime_control_lock(path: &Path) -> Result<(), BackgroundServiceControlError> {
    clear_stale_runtime_control_lock_at(path, SystemTime::now())
}

fn clear_stale_runtime_control_lock_at(
    path: &Path,
    now: SystemTime,
) -> Result<(), BackgroundServiceControlError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(BackgroundServiceControlError::ReadFile {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let lock_metadata = read_runtime_control_lock_metadata(path)?;
    if let Some(process_id) = lock_metadata.process_id {
        match process_is_running(process_id) {
            Some(false) => {
                return match fs::remove_file(path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                    Err(source) => Err(BackgroundServiceControlError::RemoveFile {
                        path: path.to_path_buf(),
                        source,
                    }),
                };
            }
            Some(true) | None => return Ok(()),
        }
    }
    let Ok(modified) = metadata.modified() else {
        return Ok(());
    };
    if now.duration_since(modified).unwrap_or_default() < RUNTIME_CONTROL_LOCK_STALE_AGE {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(BackgroundServiceControlError::RemoveFile {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn derive_actual_runtime_mode(
    control_plane: &PersistedRuntimeControlState,
    observed: &RuntimeControlObservedState,
) -> DaemonActualRuntimeMode {
    if let Some(status) = observed.daemon_status.as_ref() {
        if let Some(ownership) = observed.ownership.as_ref()
            && ownership.daemon_instance_id == status.daemon_instance_id
        {
            return match status.runtime_mode {
                DaemonRuntimeMode::Local => DaemonActualRuntimeMode::Local,
                DaemonRuntimeMode::Background => DaemonActualRuntimeMode::Background,
            };
        }
        if status.ready {
            return DaemonActualRuntimeMode::Foreign;
        }
    }
    if observed.background_service.running {
        return DaemonActualRuntimeMode::Background;
    }
    if control_plane.desired_mode == DaemonRuntimeMode::Local {
        return DaemonActualRuntimeMode::Stopped;
    }
    DaemonActualRuntimeMode::Stopped
}

fn derive_transition_status(
    control_plane: &PersistedRuntimeControlState,
) -> DaemonTransitionStatus {
    match (
        control_plane.pending_transition.is_some(),
        control_plane.last_error.is_some(),
    ) {
        (true, true) => DaemonTransitionStatus::DegradedReconcileRequired,
        (true, false) => DaemonTransitionStatus::Applying,
        (false, true) => DaemonTransitionStatus::FailedNoStateChange,
        (false, false) => DaemonTransitionStatus::Idle,
    }
}

fn derive_allowed_actions(
    control_plane: &PersistedRuntimeControlState,
    observed: &RuntimeControlObservedState,
) -> Vec<DaemonControlAction> {
    let mut actions = Vec::new();
    let actual_mode = derive_actual_runtime_mode(control_plane, observed);
    let transition_status = derive_transition_status(control_plane);
    if matches!(
        transition_status,
        DaemonTransitionStatus::Applying | DaemonTransitionStatus::DegradedReconcileRequired
    ) {
        actions.push(DaemonControlAction::Reconcile);
        return actions;
    }
    if !observed.background_service.available && !control_plane.background_opt_in {
        if matches!(actual_mode, DaemonActualRuntimeMode::Stopped) {
            actions.push(DaemonControlAction::Start);
        }
        if matches!(
            actual_mode,
            DaemonActualRuntimeMode::Local | DaemonActualRuntimeMode::Background
        ) {
            actions.push(DaemonControlAction::Stop);
        }
        return actions;
    }
    match control_plane.desired_mode {
        DaemonRuntimeMode::Local => {
            if matches!(actual_mode, DaemonActualRuntimeMode::Stopped) {
                actions.push(DaemonControlAction::Start);
            }
            if matches!(actual_mode, DaemonActualRuntimeMode::Local) {
                actions.push(DaemonControlAction::Stop);
            }
            if observed.background_service.available && !control_plane.background_opt_in {
                actions.push(DaemonControlAction::EnableBackground);
            }
        }
        DaemonRuntimeMode::Background => {
            if matches!(actual_mode, DaemonActualRuntimeMode::Stopped) {
                actions.push(DaemonControlAction::Start);
            }
            if matches!(actual_mode, DaemonActualRuntimeMode::Background) {
                actions.push(DaemonControlAction::Stop);
            }
            if control_plane.background_opt_in {
                actions.push(DaemonControlAction::DisableBackground);
            }
        }
    }
    actions
}

fn derive_status_message(
    control_plane: &PersistedRuntimeControlState,
    observed: &RuntimeControlObservedState,
    actual_mode: &DaemonActualRuntimeMode,
) -> String {
    if let Some(error) = control_plane.last_error.as_ref() {
        if let Some(pending) = control_plane.pending_transition.as_ref() {
            return format!(
                "{} transition stalled: {}",
                pending.kind.label(),
                error.message
            );
        }
        return error.message.clone();
    }
    if let Some(pending) = control_plane.pending_transition.as_ref() {
        return format!("{} transition applying.", pending.kind.label());
    }
    if matches!(actual_mode, DaemonActualRuntimeMode::Foreign) {
        return "Connected runtime is not owned by this control plane.".to_string();
    }
    if !observed.background_service.available && control_plane.background_opt_in {
        return "Background mode was requested, but this platform does not expose a supported background service.".to_string();
    }
    match control_plane.desired_mode {
        DaemonRuntimeMode::Local => "Local mode is the desired runtime.".to_string(),
        DaemonRuntimeMode::Background => "Background mode is the desired runtime.".to_string(),
    }
}

fn public_pending_transition_view(pending: &PendingTransition) -> DaemonPendingTransitionView {
    DaemonPendingTransitionView {
        kind: pending.kind.public_kind(),
        op_id: pending.op_id.to_string(),
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), BackgroundServiceControlError> {
    let parent = path
        .parent()
        .expect("service control paths should always have a parent")
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|source| BackgroundServiceControlError::CreateDirectory {
        path: parent,
        source,
    })
}

fn write_private_file(path: &Path, contents: &str) -> Result<(), BackgroundServiceControlError> {
    write_private_file_atomically(path, contents)
}

fn write_private_file_atomically(
    path: &Path,
    contents: &str,
) -> Result<(), BackgroundServiceControlError> {
    let temp_path = temp_private_file_path(path);
    #[cfg(unix)]
    {
        use std::{
            fs::OpenOptions,
            os::unix::fs::{OpenOptionsExt, PermissionsExt},
        };

        let mut file = OpenOptions::new()
            .create(true)
            .create_new(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&temp_path)
            .map_err(|source| BackgroundServiceControlError::WriteFile {
                path: temp_path.clone(),
                source,
            })?;
        let write_result = file
            .write_all(contents.as_bytes())
            .and_then(|()| file.set_permissions(fs::Permissions::from_mode(0o600)))
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(source) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(BackgroundServiceControlError::WriteFile {
                path: temp_path,
                source,
            });
        }
        fs::rename(&temp_path, path).map_err(|source| {
            BackgroundServiceControlError::WriteFile {
                path: path.to_path_buf(),
                source,
            }
        })?;
        sync_parent_dir(path)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| BackgroundServiceControlError::WriteFile {
                path: temp_path.clone(),
                source,
            })?;
        let write_result = file
            .write_all(contents.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(source) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(BackgroundServiceControlError::WriteFile {
                path: temp_path,
                source,
            });
        }
        fs::rename(&temp_path, path).map_err(|source| BackgroundServiceControlError::WriteFile {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn temp_private_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("runtime control file should have a file name");
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.parent()
        .expect("runtime control file should have a parent")
        .join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            unique
        ))
}

fn read_runtime_control_lock_metadata(
    path: &Path,
) -> Result<RuntimeControlLockMetadata, BackgroundServiceControlError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RuntimeControlLockMetadata { process_id: None });
        }
        Err(source) => {
            return Err(BackgroundServiceControlError::ReadFile {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    Ok(RuntimeControlLockMetadata {
        process_id: contents
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .and_then(|value| value.trim().parse::<u32>().ok()),
    })
}

#[cfg(unix)]
fn sync_parent_dir(path: &Path) -> Result<(), BackgroundServiceControlError> {
    let parent = path
        .parent()
        .expect("runtime control file should have a parent");
    fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|source| BackgroundServiceControlError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) -> Result<(), BackgroundServiceControlError> {
    Ok(())
}

impl PendingTransitionKind {
    pub fn public_kind(self) -> DaemonPendingTransitionKind {
        match self {
            Self::EnableBackground => DaemonPendingTransitionKind::EnableBackground,
            Self::DisableBackground => DaemonPendingTransitionKind::DisableBackground,
            Self::RecoverToLocal => DaemonPendingTransitionKind::RecoverToLocal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::EnableBackground => "Enable background",
            Self::DisableBackground => "Disable background",
            Self::RecoverToLocal => "Recover local runtime",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackgroundServiceControlError, RUNTIME_CONTROL_LOCK_STALE_AGE,
        RuntimeControlOwnershipRecord, clear_runtime_control_ownership_if_matches,
        clear_stale_runtime_control_lock_at, read_persisted_runtime_control_state,
        read_runtime_control_ownership, runtime_control_ownership_file_path,
        runtime_control_state_file_path, write_runtime_control_ownership,
        write_runtime_control_ownership_file,
    };
    use crate::host::config::ControlToken;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    #[cfg(unix)]
    fn runtime_control_ownership_file_is_written_with_private_permissions() {
        let path = temp_test_path("runtime-control-owner");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");

        write_runtime_control_ownership_file(
            &path,
            &RuntimeControlOwnershipRecord {
                runtime_mode: ta_protocol::wire::DaemonRuntimeMode::Local,
                daemon_instance_id: "daemon-1".to_string(),
                control_token: ControlToken::new("secret-token".to_string()),
                process_id: Some(1234),
            },
        )
        .expect("ownership file should write");

        let mode = fs::metadata(&path)
            .expect("ownership file should exist")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "ownership file should not be world-readable"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn runtime_control_ownership_compare_clear_preserves_mismatched_record() {
        crate::with_test_config_home("state-ownership-compare-clear", || {
            let current = RuntimeControlOwnershipRecord {
                runtime_mode: ta_protocol::wire::DaemonRuntimeMode::Local,
                daemon_instance_id: "daemon-current".to_string(),
                control_token: ControlToken::new("current-token".to_string()),
                process_id: Some(4321),
            };
            write_runtime_control_ownership(&current).expect("ownership should write");

            assert!(
                !clear_runtime_control_ownership_if_matches(Some((
                    "daemon-stale",
                    &ControlToken::new("stale-token".to_string()),
                )))
                .expect("mismatched clear should not fail")
            );
            assert_eq!(
                read_runtime_control_ownership().expect("ownership should read"),
                Some(current.clone())
            );

            assert!(
                clear_runtime_control_ownership_if_matches(Some((
                    current.daemon_instance_id.as_str(),
                    &current.control_token,
                )))
                .expect("matching clear should succeed")
            );
            assert_eq!(
                read_runtime_control_ownership().expect("ownership should read"),
                None
            );
        });
    }

    #[test]
    fn malformed_runtime_control_state_file_returns_parse_error() {
        crate::with_test_config_home("state-control-parse-error", || {
            let path = runtime_control_state_file_path();
            fs::create_dir_all(path.parent().expect("state path should have parent"))
                .expect("state parent dir should exist");
            fs::write(&path, "{\n  \"desiredMode\": \"local\"\n}\n169,\n")
                .expect("malformed state file should write");

            let error = read_persisted_runtime_control_state()
                .expect_err("malformed state file should fail to parse");
            assert!(matches!(
                error,
                BackgroundServiceControlError::ParseControlPlaneFile { path: error_path, .. }
                    if error_path == path
            ));
        });
    }

    #[test]
    fn malformed_runtime_control_ownership_file_returns_parse_error() {
        crate::with_test_config_home("state-ownership-parse-error", || {
            let path = runtime_control_ownership_file_path();
            fs::create_dir_all(path.parent().expect("ownership path should have parent"))
                .expect("ownership parent dir should exist");
            fs::write(
                &path,
                "{\n  \"daemonInstanceId\": \"daemon-1\"\n}\n\"token\":\n",
            )
            .expect("malformed ownership file should write");

            let error = read_runtime_control_ownership()
                .expect_err("malformed ownership file should fail to parse");
            assert!(matches!(
                error,
                BackgroundServiceControlError::ParseOwnershipFile { path: error_path, .. }
                    if error_path == path
            ));
        });
    }

    #[test]
    fn runtime_control_ownership_atomic_rewrite_replaces_cleanly() {
        let path = temp_test_path("runtime-control-owner-atomic");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");

        write_runtime_control_ownership_file(
            &path,
            &RuntimeControlOwnershipRecord {
                runtime_mode: ta_protocol::wire::DaemonRuntimeMode::Local,
                daemon_instance_id: "daemon-1".to_string(),
                control_token: ControlToken::new("token-1".to_string()),
                process_id: Some(11),
            },
        )
        .expect("initial ownership should write");
        std::thread::sleep(std::time::Duration::from_millis(1));
        write_runtime_control_ownership_file(
            &path,
            &RuntimeControlOwnershipRecord {
                runtime_mode: ta_protocol::wire::DaemonRuntimeMode::Background,
                daemon_instance_id: "daemon-2".to_string(),
                control_token: ControlToken::new("token-2".to_string()),
                process_id: Some(22),
            },
        )
        .expect("replacement ownership should write");

        let contents = fs::read_to_string(&path).expect("ownership file should exist");
        let parsed: RuntimeControlOwnershipRecord =
            serde_json::from_str(&contents).expect("ownership file should stay valid json");
        assert_eq!(parsed.daemon_instance_id, "daemon-2");
        assert_eq!(parsed.control_token.as_str(), "token-2");
        let tmp_prefix = format!(
            ".{}.",
            path.file_name()
                .expect("ownership path should have file name")
                .to_string_lossy()
        );
        assert!(
            fs::read_dir(parent)
                .expect("parent dir should be readable")
                .filter_map(Result::ok)
                .all(|entry| {
                    let file_name = entry.file_name();
                    let file_name = file_name.to_string_lossy();
                    !file_name.starts_with(&tmp_prefix) || !file_name.ends_with(".tmp")
                }),
            "atomic ownership rewrite should not leave temp files behind"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn stale_runtime_control_lock_keeps_live_pid() {
        let path = temp_test_path("runtime-control-lock-live");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");
        fs::write(&path, format!("pid={}\n", std::process::id())).expect("lock file should write");
        let modified = fs::metadata(&path)
            .expect("lock file should exist")
            .modified()
            .expect("lock file should have modified time");

        clear_stale_runtime_control_lock_at(&path, modified + RUNTIME_CONTROL_LOCK_STALE_AGE)
            .expect("stale lock sweep should succeed");

        assert!(path.exists(), "live pid lock must not be evicted");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn stale_runtime_control_lock_evicts_missing_pid_metadata() {
        let path = temp_test_path("runtime-control-lock-missing-pid");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");
        fs::write(&path, "invalid-lock\n").expect("lock file should write");
        let modified = fs::metadata(&path)
            .expect("lock file should exist")
            .modified()
            .expect("lock file should have modified time");

        clear_stale_runtime_control_lock_at(
            &path,
            modified + RUNTIME_CONTROL_LOCK_STALE_AGE + std::time::Duration::from_secs(1),
        )
        .expect("stale lock sweep should succeed");

        assert!(!path.exists(), "missing-pid stale lock should be evicted");
    }

    #[test]
    #[cfg(unix)]
    fn fresh_runtime_control_lock_evicts_dead_pid_immediately() {
        let path = temp_test_path("runtime-control-lock-dead-pid-fresh");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");

        let mut child = Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("child should spawn");
        let child_pid = child.id();
        child.wait().expect("child should exit");

        fs::write(&path, format!("pid={child_pid}\n")).expect("lock file should write");
        let modified = fs::metadata(&path)
            .expect("lock file should exist")
            .modified()
            .expect("lock file should have modified time");

        clear_stale_runtime_control_lock_at(&path, modified + std::time::Duration::from_secs(1))
            .expect("dead pid lock should evict immediately");

        assert!(!path.exists(), "dead-pid fresh lock should be evicted");
    }

    #[test]
    #[cfg(unix)]
    fn stale_runtime_control_lock_evicts_dead_pid() {
        let path = temp_test_path("runtime-control-lock-dead-pid");
        let parent = path.parent().expect("test path should have parent");
        fs::create_dir_all(parent).expect("test parent dir should exist");

        let mut child = Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("child should spawn");
        let child_pid = child.id();
        child.wait().expect("child should exit");

        fs::write(&path, format!("pid={child_pid}\n")).expect("lock file should write");
        let modified = fs::metadata(&path)
            .expect("lock file should exist")
            .modified()
            .expect("lock file should have modified time");

        clear_stale_runtime_control_lock_at(
            &path,
            modified + RUNTIME_CONTROL_LOCK_STALE_AGE + std::time::Duration::from_secs(1),
        )
        .expect("stale lock sweep should succeed");

        assert!(!path.exists(), "dead-pid stale lock should be evicted");
    }

    fn temp_test_path(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join("taugentic-runtime-control-tests")
            .join(format!("{prefix}-{unique}.json"))
    }
}
