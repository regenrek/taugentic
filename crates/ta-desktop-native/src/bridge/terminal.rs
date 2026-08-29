use std::sync::Arc;

use napi::{
    Error, Result, bindgen_prelude::AsyncTask, threadsafe_function::ThreadsafeFunctionCallMode,
};
use napi_derive::napi;
use ta_daemon_client::{PersistentDaemonClient, TerminalEventSubscription};
use ta_protocol::wire::{
    TerminalAttachParams, TerminalCloseParams, TerminalInputParams, TerminalListParams,
    TerminalResizeParams, TerminalSessionId, TerminalSpawnParams, TerminalStreamEvent,
};

use super::{
    BridgeState, NativeDaemonBridge, NativeJsonCallback, fail, get, json, stream_terminal,
};

pub(super) struct ActiveTerminalSubscription {
    client: PersistentDaemonClient,
}

#[napi]
impl NativeDaemonBridge {
    #[napi]
    pub fn spawn_terminal(&self, params_json: String) -> AsyncTask<SpawnTerminalTask> {
        AsyncTask::new(SpawnTerminalTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn list_terminals(&self, params_json: String) -> AsyncTask<ListTerminalsTask> {
        AsyncTask::new(ListTerminalsTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn terminal_input(&self, params_json: String) -> AsyncTask<TerminalInputTask> {
        AsyncTask::new(TerminalInputTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn resize_terminal(&self, params_json: String) -> AsyncTask<ResizeTerminalTask> {
        AsyncTask::new(ResizeTerminalTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn close_terminal(&self, params_json: String) -> AsyncTask<CloseTerminalTask> {
        AsyncTask::new(CloseTerminalTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn subscribe_terminal_events(
        &self,
        terminal_id: String,
        callback: NativeJsonCallback,
    ) -> AsyncTask<SubscribeTerminalTask> {
        AsyncTask::new(SubscribeTerminalTask {
            state: Arc::clone(&self.state),
            terminal_id,
            callback: Arc::new(callback),
        })
    }

    #[napi]
    pub fn release_terminal_event_subscription(&self) -> String {
        release_terminal_subscription(&self.state);
        "{}".to_string()
    }
}

pub(super) fn release_terminal_subscription(state: &Arc<BridgeState>) {
    let active = {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        lifecycle.terminal_subscription_generation =
            lifecycle.terminal_subscription_generation.wrapping_add(1);
        lifecycle.terminal_subscription.take()
    };
    if let Some(active) = active {
        active.client.close();
    }
}

fn claim_terminal_subscription(state: &Arc<BridgeState>) -> Result<(u64, PersistentDaemonClient)> {
    let mut lifecycle = state
        .lifecycle
        .lock()
        .expect("bridge lifecycle lock poisoned");
    if lifecycle.terminal_subscription.is_some() {
        return Err(Error::from_reason(
            "native daemon terminal subscription is already active",
        ));
    }
    let client = lifecycle
        .client
        .as_ref()
        .ok_or_else(|| Error::from_reason("native daemon bridge is not started"))?
        .client
        .fork_connection()
        .map_err(fail)?;
    lifecycle.terminal_subscription_generation =
        lifecycle.terminal_subscription_generation.wrapping_add(1);
    let generation = lifecycle.terminal_subscription_generation;
    lifecycle.terminal_subscription = Some(ActiveTerminalSubscription {
        client: client.clone(),
    });
    Ok((generation, client))
}

fn release_terminal_subscription_generation(state: &Arc<BridgeState>, generation: u64) {
    let active = {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .expect("bridge lifecycle lock poisoned");
        if lifecycle.terminal_subscription_generation != generation {
            return;
        }
        lifecycle.terminal_subscription.take()
    };
    if let Some(active) = active {
        active.client.close();
    }
}

macro_rules! terminal_task {
    ($name:ident, $params:ty, $method:ident) => {
        pub struct $name {
            client: Result<PersistentDaemonClient>,
            params_json: String,
        }

        impl napi::Task for $name {
            type Output = String;
            type JsValue = String;

            fn compute(&mut self) -> Result<String> {
                let params: $params = serde_json::from_str(&self.params_json).map_err(fail)?;
                let mut client = self
                    .client
                    .as_ref()
                    .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
                    .clone();
                json(&client.$method(params).map_err(fail)?)
            }

            fn resolve(&mut self, _: napi::Env, output: String) -> Result<String> {
                Ok(output)
            }
        }
    };
}

terminal_task!(SpawnTerminalTask, TerminalSpawnParams, spawn_terminal);
terminal_task!(ListTerminalsTask, TerminalListParams, list_terminals);
terminal_task!(TerminalInputTask, TerminalInputParams, terminal_input);
terminal_task!(ResizeTerminalTask, TerminalResizeParams, resize_terminal);
terminal_task!(CloseTerminalTask, TerminalCloseParams, close_terminal);

pub struct SubscribeTerminalTask {
    state: Arc<BridgeState>,
    terminal_id: String,
    callback: Arc<NativeJsonCallback>,
}

impl napi::Task for SubscribeTerminalTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<String> {
        let terminal_id = TerminalSessionId::new(self.terminal_id.clone()).map_err(fail)?;
        let (generation, client) = claim_terminal_subscription(&self.state)?;
        let subscription = match client.subscribe_terminal(TerminalAttachParams { terminal_id }) {
            Ok(subscription) => subscription,
            Err(error) => {
                release_terminal_subscription_generation(&self.state, generation);
                return Err(fail(error));
            }
        };
        let initial = subscription.initial().clone();
        spawn_terminal_delivery(
            subscription,
            Arc::clone(&self.callback),
            Arc::clone(&self.state),
            generation,
        );
        json(&initial)
    }

    fn resolve(&mut self, _: napi::Env, output: String) -> Result<String> {
        Ok(output)
    }
}

fn spawn_terminal_delivery(
    subscription: TerminalEventSubscription,
    callback: Arc<NativeJsonCallback>,
    state: Arc<BridgeState>,
    generation: u64,
) {
    std::thread::spawn(move || {
        loop {
            let (value, terminal) = match subscription.recv() {
                Ok(event) => match serde_json::to_string(&event) {
                    Ok(value) => (value, matches!(event.event, TerminalStreamEvent::Exited)),
                    Err(_) => (super::EVENT_CLOSED.to_owned(), true),
                },
                Err(error) => (stream_terminal(&error).to_owned(), true),
            };
            if callback.call(value, ThreadsafeFunctionCallMode::NonBlocking) != napi::Status::Ok {
                break;
            }
            if terminal {
                break;
            }
        }
        release_terminal_subscription_generation(&state, generation);
    });
}
