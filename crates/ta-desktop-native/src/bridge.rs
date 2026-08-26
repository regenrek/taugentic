use napi::{
    bindgen_prelude::{AsyncTask, Error, Result, Task, Unknown},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};
use ta_daemon_client::{
    DaemonClient, DaemonLifecycleSubscription, DaemonLifecycleSubscriptionState,
    DaemonLifecycleUpdate, PersistentDaemonClient, RunEventSubscription,
};
use ta_orchestrator::{DesktopRuntimeHandle, DesktopRuntimeStartStage, start_desktop_runtime};
use ta_protocol::wire::{
    DaemonActualRuntimeMode, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonApprovalDecideParams, DaemonNavigationIntent, DaemonProjectOpenParams,
    DaemonRunCancelParams, DaemonSessionOpenParams, DesktopDaemonLifecycleProjection,
    DesktopDaemonLifecycleStatus, ListApprovalsQuery, METHOD_DAEMON_RUN_CANCEL, RunId, SessionId,
    StartRunCommand, SubscribeRunEventsRequest, WorkspacePath,
};

const DESKTOP_CLIENT_NAME: &str = "taugentic-desktop";
const EVENT_CLOSED: &str = "native daemon event stream closed";
const EVENT_BACKPRESSURE: &str = "native daemon event stream backpressure";
const STARTED_RESULT: &str = "{\"started\":true}";
type NativeJsonCallback = ThreadsafeFunction<String, Unknown<'static>, String, napi::Status, false>;

fn stream_terminal(error: &ta_jsonrpc::JsonRpcClientError) -> &'static str {
    match error {
        ta_jsonrpc::JsonRpcClientError::Backpressure => EVENT_BACKPRESSURE,
        _ => EVENT_CLOSED,
    }
}

struct Lifecycle {
    generation: u64,
    starting_generation: Option<u64>,
    closing: bool,
    lifecycle_subscription_generation: Option<u64>,
    run_subscription_generation: u64,
    run_subscription_client: Option<PersistentDaemonClient>,
    client: Option<StartedClient>,
}

struct StartedClient {
    client: PersistentDaemonClient,
    runtime: DesktopRuntimeHandle,
    foreign_runtime_restricted: bool,
}

/// Rust-only classification for the one native post-claim construction
/// operation. It intentionally retains no underlying error or value and is
/// always reduced through `fail()` before a production N-API result exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeStartStage {
    Runtime(DesktopRuntimeStartStage),
    ControlStatus,
    PersistentClient,
}

/// The bridge owns only resource lifetime. Runtime-control decides whether a
/// release stops anything; the bridge never receives runtime identity or mode.
trait BridgeResource {
    fn close_client(&self);
    fn release_runtime(
        &mut self,
    ) -> std::result::Result<(), ta_orchestrator::DaemonControlOperationError>;
}

impl BridgeResource for StartedClient {
    fn close_client(&self) {
        self.client.close();
    }

    fn release_runtime(
        &mut self,
    ) -> std::result::Result<(), ta_orchestrator::DaemonControlOperationError> {
        self.runtime.release()
    }
}
enum StartClaim {
    Existing,
    Launch(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartClaimError {
    Superseded,
    PriorStartFailed,
}
struct BridgeState {
    lifecycle: Mutex<Lifecycle>,
    changed: Condvar,
}

enum StartFinish<T, E> {
    Publish(T),
    CloseStale(T),
    Failed(E),
}

impl BridgeState {
    /// The only start claim transition. Both StartTask and concurrency tests
    /// use it so wait/claim semantics cannot drift.
    fn claim_start(
        &self,
        requested_generation: u64,
    ) -> std::result::Result<StartClaim, StartClaimError> {
        self.claim_start_with_wait_observer(requested_generation, || {})
    }

    fn claim_start_with_wait_observer(
        &self,
        requested_generation: u64,
        on_wait: impl FnOnce(),
    ) -> std::result::Result<StartClaim, StartClaimError> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        let mut waited_for_in_flight_start = false;
        let mut on_wait = Some(on_wait);
        while lifecycle.starting_generation.is_some() || lifecycle.closing {
            waited_for_in_flight_start = true;
            on_wait.take().expect("wait observer called once")();
            lifecycle = self
                .changed
                .wait(lifecycle)
                .expect("bridge lifecycle lock poisoned");
        }
        claim_start(
            &mut lifecycle,
            requested_generation,
            waited_for_in_flight_start,
        )
    }

    /// The only close transition. It invalidates an in-flight generation and
    /// hands its established client to the caller for closing outside the lock.
    fn begin_close(&self) -> Option<StartedClient> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        lifecycle.generation = lifecycle.generation.wrapping_add(1);
        lifecycle.starting_generation = None;
        lifecycle.lifecycle_subscription_generation = None;
        lifecycle.closing = lifecycle.client.is_some();
        self.changed.notify_all();
        lifecycle.client.take()
    }

    fn finish_close(&self, restore: Option<StartedClient>) {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        lifecycle.client = restore;
        lifecycle.closing = false;
        self.changed.notify_all();
    }
}

/// The sole bridge close operation. It is shared by normal close, stale start
/// completion, and failed post-bootstrap construction cleanup.
fn release_started_resource<R: BridgeResource>(
    started: &mut R,
) -> std::result::Result<(), ta_orchestrator::DaemonControlOperationError> {
    started.close_client();
    started.release_runtime()
}

/// One close transition for both explicit N-API close and Drop. A failed
/// explicit release keeps the opaque resource so the failure is not silently
/// converted into successful shutdown; Drop runs the same transition best
/// effort because it cannot report an error.
fn release_bridge_state(
    state: &Arc<BridgeState>,
) -> std::result::Result<(), ta_orchestrator::DaemonControlOperationError> {
    release_run_subscription(state);
    let Some(mut started) = state.begin_close() else {
        return Ok(());
    };
    match release_started_resource(&mut started) {
        Ok(()) => {
            state.finish_close(None);
            Ok(())
        }
        Err(error) => {
            state.finish_close(Some(started));
            Err(error)
        }
    }
}

/// Build the bridge resource after runtime control completed. If the client
/// cannot be constructed or connected, the opaque lease is released before
/// the error is returned. This is the production transition, not a test shim.
fn construct_after_bootstrap<C, R, E>(
    runtime: R,
    connect: impl FnOnce() -> std::result::Result<C, E>,
) -> std::result::Result<(C, R), E>
where
    R: IntoBridgeRuntime,
{
    match connect() {
        Ok(client) => Ok((client, runtime)),
        Err(error) => {
            runtime.release_after_failed_connect();
            Err(error)
        }
    }
}

trait IntoBridgeRuntime {
    fn release_after_failed_connect(self);
}

impl IntoBridgeRuntime for DesktopRuntimeHandle {
    fn release_after_failed_connect(mut self) {
        let _ = self.release();
    }
}

/// Construct the native bridge resource after a lifecycle claim. This is the
/// sole post-claim startup operation used by production and the isolated Rust
/// worker contract; it never projects its typed provenance across N-API.
fn start_after_claim() -> std::result::Result<StartedClient, NativeStartStage> {
    let started =
        start_desktop_runtime().map_err(|error| NativeStartStage::Runtime(error.stage()))?;
    let control_status = started.control_status().clone();
    let foreign_runtime_restricted =
        matches!(control_status.actual_mode, DaemonActualRuntimeMode::Foreign);
    let daemon_client = DaemonClient::from_control_status(&control_status, DESKTOP_CLIENT_NAME)
        .map_err(|_| NativeStartStage::ControlStatus)?;
    let (client, runtime) = construct_after_bootstrap(started.into_handle(), || {
        daemon_client
            .connect_persistent(DESKTOP_CLIENT_NAME, env!("CARGO_PKG_VERSION"))
            .map_err(|_| NativeStartStage::PersistentClient)
    })?;
    Ok(StartedClient {
        client,
        runtime,
        foreign_runtime_restricted,
    })
}

/// The canonical completion transition. It only changes the marker belonging
/// to this start generation; a stale completion is returned to its caller for
/// closing and can never publish into a newer lifecycle.
fn finish_start<T, E>(
    lifecycle: &mut Lifecycle,
    generation: u64,
    result: std::result::Result<T, E>,
) -> StartFinish<T, E> {
    if lifecycle.starting_generation == Some(generation) {
        lifecycle.starting_generation = None;
    }
    match result {
        Ok(client) if lifecycle.generation == generation => StartFinish::Publish(client),
        Ok(client) => StartFinish::CloseStale(client),
        Err(error) => StartFinish::Failed(error),
    }
}

/// Rust-only state: credentials, authorities, socket paths, and cursors never cross N-API.
#[napi]
pub struct NativeDaemonBridge {
    state: Arc<BridgeState>,
}

#[napi]
impl NativeDaemonBridge {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            state: Arc::new(BridgeState {
                lifecycle: Mutex::new(Lifecycle {
                    generation: 0,
                    starting_generation: None,
                    closing: false,
                    lifecycle_subscription_generation: None,
                    run_subscription_generation: 0,
                    run_subscription_client: None,
                    client: None,
                }),
                changed: Condvar::new(),
            }),
        }
    }
    #[napi]
    pub fn start(&self) -> AsyncTask<StartTask> {
        AsyncTask::new(StartTask {
            state: Arc::clone(&self.state),
            requested_generation: self
                .state
                .lifecycle
                .lock()
                .expect("bridge lifecycle lock poisoned")
                .generation,
        })
    }
    #[napi]
    pub fn list_sessions(&self) -> AsyncTask<ListTask> {
        AsyncTask::new(ListTask {
            client: get(&self.state),
        })
    }
    #[napi]
    pub fn open_session(&self, params_json: String) -> AsyncTask<OpenSessionTask> {
        AsyncTask::new(OpenSessionTask {
            client: get(&self.state),
            params_json,
        })
    }
    #[napi]
    pub fn attach_session(&self, session_id: String) -> AsyncTask<AttachTask> {
        AsyncTask::new(AttachTask {
            client: get(&self.state),
            session_id,
        })
    }
    #[napi]
    pub fn navigation_snapshot(&self, search: Option<String>) -> AsyncTask<NavigationSnapshotTask> {
        AsyncTask::new(NavigationSnapshotTask {
            client: get(&self.state),
            search,
        })
    }
    #[napi]
    pub fn navigation_intent(&self, intent_json: String) -> AsyncTask<NavigationIntentTask> {
        AsyncTask::new(NavigationIntentTask {
            client: get(&self.state),
            intent_json,
        })
    }
    #[napi]
    pub fn open_project(
        &self,
        path: String,
        trust_acknowledged: bool,
    ) -> AsyncTask<OpenProjectTask> {
        AsyncTask::new(OpenProjectTask {
            client: get(&self.state),
            path,
            trust_acknowledged,
        })
    }
    #[napi]
    pub fn get_agent_runtime(&self) -> AsyncTask<GetAgentRuntimeTask> {
        AsyncTask::new(GetAgentRuntimeTask {
            client: get(&self.state),
        })
    }
    #[napi]
    pub fn login_auth_profile(&self, params_json: String) -> AsyncTask<LoginAuthProfileTask> {
        AsyncTask::new(LoginAuthProfileTask {
            client: get(&self.state),
            params_json,
        })
    }
    #[napi]
    pub fn complete_auth_profile_login(
        &self,
        params_json: String,
    ) -> AsyncTask<CompleteAuthProfileLoginTask> {
        AsyncTask::new(CompleteAuthProfileLoginTask {
            client: get(&self.state),
            params_json,
        })
    }
    #[napi]
    pub fn logout_auth_profile(&self, params_json: String) -> AsyncTask<LogoutAuthProfileTask> {
        AsyncTask::new(LogoutAuthProfileTask {
            client: get(&self.state),
            params_json,
        })
    }
    #[napi]
    pub fn list_approvals(&self, query_json: String) -> AsyncTask<ListApprovalsTask> {
        AsyncTask::new(ListApprovalsTask {
            client: get(&self.state),
            query_json,
        })
    }
    #[napi]
    pub fn decide_approval(&self, params_json: String) -> AsyncTask<DecideApprovalTask> {
        AsyncTask::new(DecideApprovalTask {
            client: get(&self.state),
            params_json,
        })
    }
    #[napi]
    pub fn start_run(&self, command_json: String) -> AsyncTask<StartRunTask> {
        AsyncTask::new(StartRunTask {
            client: get(&self.state),
            command_json,
        })
    }
    #[napi]
    pub fn cancel_run(&self, run_id: String) -> AsyncTask<CancelTask> {
        AsyncTask::new(CancelTask {
            client: get(&self.state),
            run_id,
            state: Arc::clone(&self.state),
        })
    }
    #[napi]
    pub fn release_run_event_subscription(&self) -> String {
        release_run_subscription(&self.state);
        "{}".to_string()
    }
    #[napi]
    pub fn subscribe_run_events(
        &self,
        session_id: String,
        run_id: String,
        callback: NativeJsonCallback,
    ) -> AsyncTask<SubscribeTask> {
        AsyncTask::new(SubscribeTask {
            state: Arc::clone(&self.state),
            session_id,
            run_id,
            callback: Arc::new(callback),
        })
    }
    /// Establishes the one redacted global lifecycle projection. Rust keeps
    /// daemon identity, cursors, transport state, and all recovery mechanics.
    #[napi]
    pub fn subscribe_lifecycle(
        &self,
        callback: NativeJsonCallback,
    ) -> AsyncTask<SubscribeLifecycleTask> {
        AsyncTask::new(SubscribeLifecycleTask {
            source: claim_lifecycle_source(&self.state),
            callback: Arc::new(callback),
        })
    }
    #[napi]
    pub fn close(&self) -> AsyncTask<CloseTask> {
        AsyncTask::new(CloseTask {
            state: Arc::clone(&self.state),
        })
    }
}

fn get(state: &Arc<BridgeState>) -> Result<PersistentDaemonClient> {
    state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned")
        .client
        .as_ref()
        .map(|started| started.client.clone())
        .ok_or_else(|| Error::from_reason("native daemon bridge is not started"))
}

struct LifecycleSource {
    state: Arc<BridgeState>,
    generation: u64,
    client: PersistentDaemonClient,
    foreign_runtime_restricted: bool,
}

fn claim_lifecycle_source(state: &Arc<BridgeState>) -> Result<LifecycleSource> {
    let mut lifecycle = state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned");
    let generation = lifecycle.generation;
    if lifecycle.lifecycle_subscription_generation.is_some() {
        return Err(Error::from_reason(
            "native daemon lifecycle subscription is already active",
        ));
    }
    let source = lifecycle
        .client
        .as_ref()
        .map(|started| LifecycleSource {
            state: Arc::clone(state),
            generation,
            client: started.client.clone(),
            foreign_runtime_restricted: started.foreign_runtime_restricted,
        })
        .ok_or_else(|| Error::from_reason("native daemon bridge is not started"))?;
    lifecycle.lifecycle_subscription_generation = Some(generation);
    Ok(source)
}

fn release_lifecycle_subscription(state: &Arc<BridgeState>, generation: u64) {
    let mut lifecycle = state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned");
    if lifecycle.lifecycle_subscription_generation == Some(generation) {
        lifecycle.lifecycle_subscription_generation = None;
    }
}

fn claim_run_subscription(
    state: &Arc<BridgeState>,
    session_id: SessionId,
) -> Result<(u64, PersistentDaemonClient)> {
    let mut lifecycle = state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned");
    if lifecycle.run_subscription_client.is_some() {
        return Err(Error::from_reason(
            "native daemon run event subscription is already active",
        ));
    }
    let client = lifecycle
        .client
        .as_ref()
        .ok_or_else(|| Error::from_reason("native daemon bridge is not started"))?
        .client
        .fork_session_connection(session_id)
        .map_err(fail)?;
    lifecycle.run_subscription_generation = lifecycle.run_subscription_generation.wrapping_add(1);
    let generation = lifecycle.run_subscription_generation;
    lifecycle.run_subscription_client = Some(client.clone());
    Ok((generation, client))
}

fn release_run_subscription_generation(state: &Arc<BridgeState>, generation: u64) {
    let client = {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        if lifecycle.run_subscription_generation != generation {
            return;
        }
        lifecycle.run_subscription_client.take()
    };
    if let Some(client) = client {
        client.close();
    }
}

fn release_run_subscription(state: &Arc<BridgeState>) {
    let client = {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        lifecycle.run_subscription_generation =
            lifecycle.run_subscription_generation.wrapping_add(1);
        lifecycle.run_subscription_client.take()
    };
    if let Some(client) = client {
        client.close();
    }
}
fn fail<E>(_: E) -> Error {
    Error::from_reason("native daemon operation failed")
}
fn json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(fail)
}
macro_rules! task {
    ($name:ident, $body:expr) => {
        impl Task for $name {
            type Output = String;
            type JsValue = String;
            fn compute(&mut self) -> Result<String> {
                $body(self)
            }
            fn resolve(&mut self, _: napi::Env, output: String) -> Result<String> {
                Ok(output)
            }
        }
    };
}

struct StartTask {
    state: Arc<BridgeState>,
    requested_generation: u64,
}
task!(StartTask, |this: &mut StartTask| {
    let generation = match this
        .state
        .claim_start(this.requested_generation)
        .map_err(|error| match error {
            StartClaimError::Superseded => {
                Error::from_reason("native daemon bridge start was superseded")
            }
            StartClaimError::PriorStartFailed => {
                Error::from_reason("native daemon operation failed")
            }
        })? {
        StartClaim::Existing => return Ok(STARTED_RESULT.into()),
        StartClaim::Launch(generation) => generation,
    };
    let result = start_after_claim().map_err(fail);
    let mut lifecycle = this
        .state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned");
    let completion = finish_start(&mut lifecycle, generation, result);
    this.state.changed.notify_all();
    match completion {
        StartFinish::Publish(client) => {
            lifecycle.client = Some(client);
            Ok(STARTED_RESULT.into())
        }
        StartFinish::CloseStale(client) => {
            drop(lifecycle);
            let mut client = client;
            let _ = release_started_resource(&mut client);
            Err(Error::from_reason(
                "native daemon bridge start was superseded",
            ))
        }
        StartFinish::Failed(error) => Err(error),
    }
});

fn claim_start(
    lifecycle: &mut Lifecycle,
    requested_generation: u64,
    waited: bool,
) -> std::result::Result<StartClaim, StartClaimError> {
    if lifecycle.generation != requested_generation {
        return Err(StartClaimError::Superseded);
    }
    if lifecycle.client.is_some() {
        return Ok(StartClaim::Existing);
    }
    if waited {
        return Err(StartClaimError::PriorStartFailed);
    }
    lifecycle.starting_generation = Some(lifecycle.generation);
    Ok(StartClaim::Launch(lifecycle.generation))
}

struct CloseTask {
    state: Arc<BridgeState>,
}
task!(CloseTask, |this: &mut CloseTask| {
    release_bridge_state(&this.state).map_err(fail)?;
    Ok("{}".into())
});

impl Drop for NativeDaemonBridge {
    fn drop(&mut self) {
        let _ = release_bridge_state(&self.state);
    }
}
struct ListTask {
    client: Result<PersistentDaemonClient>,
}
task!(ListTask, |this: &mut ListTask| {
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.list_sessions().map_err(fail)?)
});
struct OpenSessionTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}
task!(OpenSessionTask, |this: &mut OpenSessionTask| {
    let params: DaemonSessionOpenParams = serde_json::from_str(&this.params_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    // The Rust client writes the returned authority before this method
    // projects the public SessionSummary to JavaScript.
    json(&client.open_session(params).map_err(fail)?.session)
});
struct AttachTask {
    client: Result<PersistentDaemonClient>,
    session_id: String,
}
task!(AttachTask, |this: &mut AttachTask| {
    let id = SessionId::new(this.session_id.clone()).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.attach_session(id).map_err(fail)?.session)
});
struct NavigationSnapshotTask {
    client: Result<PersistentDaemonClient>,
    search: Option<String>,
}
task!(
    NavigationSnapshotTask,
    |this: &mut NavigationSnapshotTask| {
        let mut client = this
            .client
            .as_ref()
            .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
            .clone();
        json(
            &client
                .navigation_snapshot(this.search.clone())
                .map_err(fail)?
                .snapshot,
        )
    }
);
struct NavigationIntentTask {
    client: Result<PersistentDaemonClient>,
    intent_json: String,
}
task!(NavigationIntentTask, |this: &mut NavigationIntentTask| {
    let intent: DaemonNavigationIntent = serde_json::from_str(&this.intent_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.navigation_intent(intent).map_err(fail)?.snapshot)
});
struct OpenProjectTask {
    client: Result<PersistentDaemonClient>,
    path: String,
    trust_acknowledged: bool,
}
task!(OpenProjectTask, |this: &mut OpenProjectTask| {
    let path = WorkspacePath::from_canonical_wire_value(this.path.clone()).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(
        &client
            .open_project(DaemonProjectOpenParams {
                path,
                trust_acknowledged: this.trust_acknowledged,
            })
            .map_err(fail)?,
    )
});
struct StartRunTask {
    client: Result<PersistentDaemonClient>,
    command_json: String,
}
task!(StartRunTask, |this: &mut StartRunTask| {
    let command: StartRunCommand = serde_json::from_str(&this.command_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.start_run(command).map_err(fail)?)
});
struct ListApprovalsTask {
    client: Result<PersistentDaemonClient>,
    query_json: String,
}
task!(ListApprovalsTask, |this: &mut ListApprovalsTask| {
    let query: ListApprovalsQuery = serde_json::from_str(&this.query_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.list_approvals(query).map_err(fail)?)
});
struct DecideApprovalTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}
task!(DecideApprovalTask, |this: &mut DecideApprovalTask| {
    let params: DaemonApprovalDecideParams =
        serde_json::from_str(&this.params_json).map_err(fail)?;
    let client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.decide_approval(params).map_err(fail)?)
});
struct GetAgentRuntimeTask {
    client: Result<PersistentDaemonClient>,
}
task!(GetAgentRuntimeTask, |this: &mut GetAgentRuntimeTask| {
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.get_agent_runtime().map_err(fail)?)
});
struct LoginAuthProfileTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}
task!(LoginAuthProfileTask, |this: &mut LoginAuthProfileTask| {
    let params: DaemonAgentRuntimeAuthLoginParams =
        serde_json::from_str(&this.params_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(
        &client
            .login_agent_runtime_auth_profile(params)
            .map_err(fail)?,
    )
});
struct CompleteAuthProfileLoginTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}
task!(
    CompleteAuthProfileLoginTask,
    |this: &mut CompleteAuthProfileLoginTask| {
        let params: DaemonAgentRuntimeAuthLoginCompleteParams =
            serde_json::from_str(&this.params_json).map_err(fail)?;
        let mut client = this
            .client
            .as_ref()
            .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
            .clone();
        json(
            &client
                .complete_agent_runtime_auth_profile_login(params)
                .map_err(fail)?,
        )
    }
);
struct LogoutAuthProfileTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}
task!(LogoutAuthProfileTask, |this: &mut LogoutAuthProfileTask| {
    let params: DaemonAgentRuntimeAuthLogoutParams =
        serde_json::from_str(&this.params_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(
        &client
            .logout_agent_runtime_auth_profile(params)
            .map_err(fail)?,
    )
});
struct CancelTask {
    client: Result<PersistentDaemonClient>,
    run_id: String,
    state: Arc<BridgeState>,
}
task!(CancelTask, |this: &mut CancelTask| {
    let run_id = RunId::new(this.run_id.clone()).map_err(fail)?;
    let _: serde_json::Value = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .call_public(
            METHOD_DAEMON_RUN_CANCEL,
            &DaemonRunCancelParams {
                run_id,
                reason: None,
            },
        )
        .map_err(fail)?;
    release_run_subscription(&this.state);
    Ok("{}".into())
});
struct SubscribeTask {
    state: Arc<BridgeState>,
    session_id: String,
    run_id: String,
    callback: Arc<NativeJsonCallback>,
}
task!(SubscribeTask, |this: &mut SubscribeTask| {
    let sid = SessionId::new(this.session_id.clone()).map_err(fail)?;
    let rid = RunId::new(this.run_id.clone()).map_err(fail)?;
    let (generation, client) = claim_run_subscription(&this.state, sid.clone())?;
    let subscription = match client.subscribe_run_events(SubscribeRunEventsRequest {
        session_id: sid,
        run_id: rid,
        after_seq: None,
    }) {
        Ok(subscription) => subscription,
        Err(error) => {
            release_run_subscription_generation(&this.state, generation);
            return Err(fail(error));
        }
    };
    let replay = subscription.replay().clone();
    spawn_event_delivery(
        subscription,
        Arc::clone(&this.callback),
        Arc::clone(&this.state),
        generation,
    );
    json(&replay)
});
fn spawn_event_delivery(
    subscription: RunEventSubscription,
    callback: Arc<NativeJsonCallback>,
    state: Arc<BridgeState>,
    generation: u64,
) {
    thread::spawn(move || {
        loop {
            let (value, terminal) = match subscription.recv() {
                Ok(event) => match serde_json::to_string(&event) {
                    Ok(value) => (value, run_event_is_terminal(&event)),
                    Err(_) => (EVENT_CLOSED.to_owned(), true),
                },
                Err(error) => (stream_terminal(&error).to_owned(), true),
            };
            if callback.call(value, ThreadsafeFunctionCallMode::NonBlocking) != napi::Status::Ok {
                // JS has rejected the terminal delivery. Ending this one
                // subscription thread drops its sole owned callback handle;
                // no background delivery is retained or retried.
                break;
            }
            if terminal {
                break;
            }
        }
        release_run_subscription_generation(&state, generation);
    });
}

fn run_event_is_terminal(event: &ta_protocol::wire::RunEventStreamItem) -> bool {
    matches!(
        &event.payload,
        ta_protocol::wire::RunEventStreamPayload::Delta { delta }
            if matches!(
                &delta.event,
                ta_protocol::wire::PublicDaemonEvent::Run(run)
                    if matches!(
                        run.status,
                        ta_protocol::wire::RunStatus::Completed
                            | ta_protocol::wire::RunStatus::Failed
                            | ta_protocol::wire::RunStatus::BudgetExceeded
                            | ta_protocol::wire::RunStatus::Cancelled
                    )
            )
    )
}

struct SubscribeLifecycleTask {
    source: Result<LifecycleSource>,
    callback: Arc<NativeJsonCallback>,
}

task!(
    SubscribeLifecycleTask,
    |this: &mut SubscribeLifecycleTask| {
        let source = this
            .source
            .as_ref()
            .map_err(|_| Error::from_reason("native daemon bridge is not started"))?;
        let (subscription, state) = match source.client.subscribe_lifecycle() {
            Ok(subscription) => subscription,
            Err(error) => {
                release_lifecycle_subscription(&source.state, source.generation);
                return Err(fail(error));
            }
        };
        let projection = lifecycle_projection(state, source.foreign_runtime_restricted);
        spawn_lifecycle_delivery(
            subscription,
            Arc::clone(&this.callback),
            source.foreign_runtime_restricted,
            Arc::clone(&source.state),
            source.generation,
        );
        json(&projection)
    }
);

fn lifecycle_projection(
    _state: DaemonLifecycleSubscriptionState,
    foreign_runtime_restricted: bool,
) -> DesktopDaemonLifecycleProjection {
    DesktopDaemonLifecycleProjection {
        status: DesktopDaemonLifecycleStatus::Ready,
        invalidated: false,
        foreign_runtime_restricted,
    }
}

fn lifecycle_update_projection(
    update: DaemonLifecycleUpdate,
    foreign_runtime_restricted: bool,
) -> DesktopDaemonLifecycleProjection {
    match update {
        DaemonLifecycleUpdate::Invalidated => DesktopDaemonLifecycleProjection {
            status: DesktopDaemonLifecycleStatus::Ready,
            invalidated: true,
            foreign_runtime_restricted,
        },
    }
}

fn disconnected_lifecycle_projection(
    foreign_runtime_restricted: bool,
) -> DesktopDaemonLifecycleProjection {
    DesktopDaemonLifecycleProjection {
        status: DesktopDaemonLifecycleStatus::Disconnected,
        invalidated: true,
        foreign_runtime_restricted,
    }
}

fn spawn_lifecycle_delivery(
    subscription: DaemonLifecycleSubscription,
    callback: Arc<NativeJsonCallback>,
    foreign_runtime_restricted: bool,
    state: Arc<BridgeState>,
    generation: u64,
) {
    thread::spawn(move || {
        loop {
            let (projection, terminal) = match subscription.recv() {
                Ok(update) => (
                    lifecycle_update_projection(update, foreign_runtime_restricted),
                    false,
                ),
                Err(_) => (
                    disconnected_lifecycle_projection(foreign_runtime_restricted),
                    true,
                ),
            };
            let generation_is_current = state
                .lifecycle
                .lock()
                .expect("bridge lifecycle lock poisoned")
                .lifecycle_subscription_generation
                == Some(generation);
            if !generation_is_current {
                break;
            }
            let value = match json(&projection) {
                Ok(value) => value,
                Err(_) => break,
            };
            if callback.call(value, ThreadsafeFunctionCallMode::NonBlocking) != napi::Status::Ok
                || terminal
            {
                break;
            }
        }
        release_lifecycle_subscription(&state, generation);
    });
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };
    use ta_observability::{LOG_DIR_ENV_VAR, LOG_STDERR_ENV_VAR};

    const NATIVE_START_WORKER_CHILD_ENV: &str = "TAUGENTIC_NATIVE_START_WORKER_CHILD";

    // `Task::compute` is pure Rust, but the native crate also contains the
    // production ThreadsafeFunction types. Cargo's standalone Rust test host
    // has no N-API runtime to provide these otherwise-unused linker symbols.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn napi_call_threadsafe_function(
        _: napi::sys::napi_threadsafe_function,
        _: *mut std::ffi::c_void,
        _: napi::sys::napi_threadsafe_function_call_mode,
    ) -> napi::sys::napi_status {
        0
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn napi_delete_reference(
        _: napi::sys::napi_env,
        _: napi::sys::napi_ref,
    ) -> napi::sys::napi_status {
        0
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn napi_reference_unref(
        _: napi::sys::napi_env,
        _: napi::sys::napi_ref,
        result: *mut u32,
    ) -> napi::sys::napi_status {
        if !result.is_null() {
            // SAFETY: this test-only linker shim receives the out-pointer
            // from napi's own drop implementation.
            unsafe { *result = 0 };
        }
        0
    }

    struct IsolatedTestRoot(PathBuf);

    impl IsolatedTestRoot {
        fn create() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after the Unix epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "taugentic-native-start-worker-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir(&root).expect("isolated native worker root should be created");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for IsolatedTestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn state(generation: u64) -> Arc<BridgeState> {
        Arc::new(BridgeState {
            lifecycle: Mutex::new(Lifecycle {
                generation,
                starting_generation: None,
                closing: false,
                lifecycle_subscription_generation: None,
                run_subscription_generation: 0,
                run_subscription_client: None,
                client: None,
            }),
            changed: Condvar::new(),
        })
    }

    #[derive(Clone)]
    struct TrackedRuntime(Arc<AtomicUsize>);

    impl IntoBridgeRuntime for TrackedRuntime {
        fn release_after_failed_connect(self) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }

    struct TrackedStarted {
        client_closes: Arc<AtomicUsize>,
        runtime_releases: Arc<AtomicUsize>,
    }

    impl BridgeResource for TrackedStarted {
        fn close_client(&self) {
            self.client_closes.fetch_add(1, Ordering::AcqRel);
        }

        fn release_runtime(
            &mut self,
        ) -> std::result::Result<(), ta_orchestrator::DaemonControlOperationError> {
            self.runtime_releases.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }
    }

    #[test]
    fn post_bootstrap_connect_failure_releases_the_opaque_runtime_before_returning() {
        let releases = Arc::new(AtomicUsize::new(0));
        let runtime = TrackedRuntime(Arc::clone(&releases));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (continue_sender, continue_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            construct_after_bootstrap(runtime, || {
                entered_sender.send(()).expect("connect entered");
                continue_receiver.recv().expect("connect released");
                Err::<(), _>("connect failed")
            })
        });

        entered_receiver.recv().expect("connect is in flight");
        assert_eq!(releases.load(Ordering::Acquire), 0);
        continue_sender.send(()).expect("permit failure");
        assert!(matches!(
            worker.join().expect("worker join"),
            Err("connect failed")
        ));
        assert_eq!(releases.load(Ordering::Acquire), 1);
    }

    #[test]
    fn native_start_compute_worker_contract() {
        if std::env::var_os(NATIVE_START_WORKER_CHILD_ENV).is_some() {
            let state = state(0);
            let mut start = StartTask {
                state: Arc::clone(&state),
                requested_generation: 0,
            };
            assert_eq!(
                start.compute().expect("native worker start must succeed"),
                STARTED_RESULT
            );

            let mut close = CloseTask { state };
            assert_eq!(
                close.compute().expect("native worker close must succeed"),
                "{}"
            );
            return;
        }

        let root = IsolatedTestRoot::create();
        let runtime_dir = root.path().join("runtime");
        let log_dir = root.path().join("logs");
        let config_home = root.path().join("home");
        fs::create_dir_all(&runtime_dir).expect("isolated runtime directory should exist");
        fs::create_dir_all(&log_dir).expect("isolated log directory should exist");
        fs::create_dir_all(config_home.join(".config"))
            .expect("isolated configuration directory should exist");

        let test_binary = std::env::current_exe().expect("test binary should resolve");
        let release_daemon = test_binary
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("test binary should be below the Cargo target directory")
            .join("release")
            .join(if cfg!(windows) {
                "ta-daemon.exe"
            } else {
                "ta-daemon"
            });
        assert!(
            release_daemon.is_file(),
            "the release daemon must be built before the native worker contract"
        );

        let status = Command::new(&test_binary)
            .arg("--exact")
            .arg("bridge::lifecycle_tests::native_start_compute_worker_contract")
            .env(NATIVE_START_WORKER_CHILD_ENV, "1")
            .env("TAUGENTIC_DAEMON_BINARY", release_daemon)
            .env("TAUGENTIC_DAEMON_SOCKET_NAME", "tg-native-start")
            .env("TAUGENTIC_DAEMON_RUNTIME_MODE", "local")
            .env("XDG_RUNTIME_DIR", runtime_dir)
            .env(LOG_DIR_ENV_VAR, log_dir)
            .env(LOG_STDERR_ENV_VAR, "0")
            .env("HOME", &config_home)
            .env("USERPROFILE", &config_home)
            .env("XDG_CONFIG_HOME", config_home.join(".config"))
            .env("APPDATA", config_home.join("AppData").join("Roaming"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("isolated native worker child should spawn");

        assert!(
            status.success(),
            "native worker contract must complete its typed start operation"
        );
    }

    #[test]
    fn concurrent_starts_dedupe_and_waiters_do_not_retry_a_failed_start() {
        let state = state(4);
        assert!(matches!(state.claim_start(4), Ok(StartClaim::Launch(4))));
        let barrier = Arc::new(Barrier::new(2));
        let waiting_state = Arc::clone(&state);
        let waiting_barrier = Arc::clone(&barrier);
        let (result_sender, result_receiver) = mpsc::channel();
        let (waiting_sender, waiting_receiver) = mpsc::channel();
        let waiter = thread::spawn(move || {
            waiting_barrier.wait();
            result_sender
                .send(waiting_state.claim_start_with_wait_observer(4, || {
                    waiting_sender.send(()).expect("waiting signal")
                }))
                .expect("result");
        });
        barrier.wait();
        waiting_receiver
            .recv()
            .expect("waiter is blocked in production transition");
        let mut lifecycle = state.lifecycle.lock().expect("lifecycle");
        assert!(matches!(
            finish_start(
                &mut lifecycle,
                4,
                Err::<(), _>(StartClaimError::PriorStartFailed)
            ),
            StartFinish::Failed(StartClaimError::PriorStartFailed)
        ));
        state.changed.notify_all();
        drop(lifecycle);
        assert!(matches!(
            result_receiver.recv().expect("waiter result"),
            Err(StartClaimError::PriorStartFailed)
        ));
        waiter.join().expect("waiter join");
    }

    #[test]
    fn close_during_inflight_start_rejects_stale_publication_and_closes_client() {
        let state = state(7);
        assert!(matches!(state.claim_start(7), Ok(StartClaim::Launch(7))));
        assert!(state.begin_close().is_none());
        let client_closes = Arc::new(AtomicUsize::new(0));
        let runtime_releases = Arc::new(AtomicUsize::new(0));
        let stale = TrackedStarted {
            client_closes: Arc::clone(&client_closes),
            runtime_releases: Arc::clone(&runtime_releases),
        };
        let mut lifecycle = state.lifecycle.lock().expect("lifecycle");
        let stale = match finish_start(&mut lifecycle, 7, Ok::<_, ()>(stale)) {
            StartFinish::CloseStale(client) => client,
            _ => panic!("stale completion must not publish"),
        };
        assert!(lifecycle.client.is_none());
        drop(lifecycle);
        let mut stale = stale;
        release_started_resource(&mut stale).expect("stale resource release");
        assert_eq!(client_closes.load(Ordering::Acquire), 1);
        assert_eq!(runtime_releases.load(Ordering::Acquire), 1);
        assert!(matches!(
            state.claim_start(7),
            Err(StartClaimError::Superseded)
        ));
    }

    #[test]
    fn stale_completion_cannot_clear_a_newer_start_marker() {
        let state = state(9);
        assert!(matches!(state.claim_start(9), Ok(StartClaim::Launch(9))));
        assert!(state.begin_close().is_none());
        assert!(matches!(state.claim_start(10), Ok(StartClaim::Launch(10))));
        let mut lifecycle = state.lifecycle.lock().expect("lifecycle");
        assert!(matches!(
            finish_start(&mut lifecycle, 9, Ok::<_, ()>(())),
            StartFinish::CloseStale(())
        ));
        assert_eq!(lifecycle.starting_generation, Some(10));
    }

    #[test]
    fn native_safe_outputs_are_redacted_and_backpressure_stays_distinct() {
        for forbidden in ["socket", "log", "credential", "authority"] {
            assert!(!STARTED_RESULT.contains(forbidden));
        }
        assert_eq!(
            stream_terminal(&ta_jsonrpc::JsonRpcClientError::Backpressure),
            EVENT_BACKPRESSURE
        );
        assert_eq!(
            stream_terminal(&ta_jsonrpc::JsonRpcClientError::ConnectionClosed),
            EVENT_CLOSED
        );
    }

    #[test]
    fn lifecycle_projection_exposes_only_safe_invalidation_and_restriction() {
        let ready = lifecycle_projection(DaemonLifecycleSubscriptionState::Ready, false);
        assert_eq!(ready.status, DesktopDaemonLifecycleStatus::Ready);
        assert!(!ready.invalidated);
        assert!(!ready.foreign_runtime_restricted);

        let invalidated = lifecycle_update_projection(DaemonLifecycleUpdate::Invalidated, true);
        assert_eq!(invalidated.status, DesktopDaemonLifecycleStatus::Ready);
        assert!(invalidated.invalidated);
        assert!(invalidated.foreign_runtime_restricted);

        let disconnected = disconnected_lifecycle_projection(true);
        assert_eq!(
            disconnected.status,
            DesktopDaemonLifecycleStatus::Disconnected
        );
        assert!(disconnected.invalidated);
        let value = serde_json::to_string(&disconnected).expect("projection serializes");
        for forbidden in [
            "daemonInstanceId",
            "cursor",
            "socket",
            "log",
            "credential",
            "authority",
            "process",
            "pid",
        ] {
            assert!(!value.contains(forbidden));
        }
    }
}
