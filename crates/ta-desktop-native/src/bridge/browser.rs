use super::*;
use ta_protocol::wire::{BrowserActionRequest, BrowserClearDataRequest};

#[napi]
impl NativeDaemonBridge {
    #[napi]
    pub fn browser_profile(&self) -> AsyncTask<BrowserProfileTask> {
        AsyncTask::new(BrowserProfileTask {
            client: get(&self.state),
        })
    }

    #[napi]
    pub fn browser_action(&self, params_json: String) -> AsyncTask<BrowserActionTask> {
        AsyncTask::new(BrowserActionTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn clear_browser_data(&self, params_json: String) -> AsyncTask<BrowserClearDataTask> {
        AsyncTask::new(BrowserClearDataTask {
            client: get(&self.state),
            params_json,
        })
    }
}

struct BrowserProfileTask {
    client: Result<PersistentDaemonClient>,
}

task!(BrowserProfileTask, |this: &mut BrowserProfileTask| {
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.browser_profile().map_err(fail)?)
});

struct BrowserActionTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}

task!(BrowserActionTask, |this: &mut BrowserActionTask| {
    let params: BrowserActionRequest = serde_json::from_str(&this.params_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.browser_action(params).map_err(fail)?)
});

struct BrowserClearDataTask {
    client: Result<PersistentDaemonClient>,
    params_json: String,
}

task!(BrowserClearDataTask, |this: &mut BrowserClearDataTask| {
    let params: BrowserClearDataRequest = serde_json::from_str(&this.params_json).map_err(fail)?;
    let mut client = this
        .client
        .as_ref()
        .map_err(|_| Error::from_reason("native daemon bridge is not started"))?
        .clone();
    json(&client.clear_browser_data(params).map_err(fail)?)
});
