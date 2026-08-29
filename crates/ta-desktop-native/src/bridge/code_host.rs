use napi::{Error, Result, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use ta_daemon_client::PersistentDaemonClient;
use ta_jsonrpc::JsonRpcClientError;
use ta_protocol::wire::{
    CodeHostAccountConnectParams, CodeHostAccountDisconnectParams,
    CodeHostPullRequestActivityParams, CodeHostPullRequestChecksParams,
    CodeHostPullRequestCommentCreateParams, CodeHostPullRequestDetailParams,
    CodeHostPullRequestEnsureParams, CodeHostPullRequestListParams, CodeHostPushApplyParams,
    CodeHostPushPrepareParams, CodeHostRepositoryContextParams,
};

use super::{NativeDaemonBridge, fail, get, json};

const NATIVE_CODE_HOST_AUTHENTICATION_REQUIRED: &str =
    "native code-host authentication is required";
const NATIVE_CODE_HOST_PERMISSION_DENIED: &str = "native code-host permission was denied";
const NATIVE_CODE_HOST_RESOURCE_NOT_FOUND: &str = "native code-host resource was not found";
const NATIVE_CODE_HOST_CONFLICT: &str = "native code-host operation conflicts with current state";
const NATIVE_CODE_HOST_RATE_LIMITED: &str = "native code-host request was rate limited";
const NATIVE_CODE_HOST_OUTCOME_UNKNOWN: &str =
    "native code-host outcome is unknown; refresh before retrying";
const NATIVE_CODE_HOST_RESPONSE_TOO_LARGE: &str =
    "native code-host response exceeded its production bound";
const NATIVE_CODE_HOST_ACCOUNT_NOT_FOUND: &str = "native code-host account was not found";
const NATIVE_CODE_HOST_WORKSPACE_RUN_ACTIVE: &str = "native code-host workspace run is active";
const NATIVE_CODE_HOST_PUSH_CONFLICT: &str = "native code-host push preview is no longer current";
const NATIVE_DAEMON_OPERATION_FAILED: &str = "native daemon operation failed";

/// The daemon emits only canonical error codes for code-host calls. Translate
/// those codes into static native messages; never relay a daemon message, error
/// data, URL, account identifier, or transport detail through N-API.
fn code_host_failure_reason(error: &JsonRpcClientError) -> &'static str {
    let Some(code) = (match error {
        JsonRpcClientError::Remote(remote) => remote.error.data.as_ref(),
        _ => None,
    })
    .and_then(|data| data.get("code"))
    .and_then(serde_json::Value::as_str) else {
        return NATIVE_DAEMON_OPERATION_FAILED;
    };

    match code {
        "CodeHostAuthenticationRequired" => NATIVE_CODE_HOST_AUTHENTICATION_REQUIRED,
        "CodeHostPermissionDenied" => NATIVE_CODE_HOST_PERMISSION_DENIED,
        "CodeHostResourceNotFound" => NATIVE_CODE_HOST_RESOURCE_NOT_FOUND,
        "CodeHostConflict" => NATIVE_CODE_HOST_CONFLICT,
        "CodeHostRateLimited" => NATIVE_CODE_HOST_RATE_LIMITED,
        "CodeHostOutcomeUnknown" => NATIVE_CODE_HOST_OUTCOME_UNKNOWN,
        "CodeHostResponseTooLarge" => NATIVE_CODE_HOST_RESPONSE_TOO_LARGE,
        "CodeHostAccountNotFound" => NATIVE_CODE_HOST_ACCOUNT_NOT_FOUND,
        "CodeHostWorkspaceRunActive" => NATIVE_CODE_HOST_WORKSPACE_RUN_ACTIVE,
        "CodeHostPushConflict" => NATIVE_CODE_HOST_PUSH_CONFLICT,
        _ => NATIVE_DAEMON_OPERATION_FAILED,
    }
}

fn code_host_fail(error: JsonRpcClientError) -> Error {
    Error::from_reason(code_host_failure_reason(&error))
}

#[napi]
impl NativeDaemonBridge {
    #[napi]
    pub fn code_host_accounts(&self) -> AsyncTask<CodeHostAccountsTask> {
        AsyncTask::new(CodeHostAccountsTask {
            client: get(&self.state),
        })
    }

    #[napi]
    pub fn connect_code_host_account(
        &self,
        params_json: String,
    ) -> AsyncTask<ConnectCodeHostAccountTask> {
        AsyncTask::new(ConnectCodeHostAccountTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn disconnect_code_host_account(
        &self,
        params_json: String,
    ) -> AsyncTask<DisconnectCodeHostAccountTask> {
        AsyncTask::new(DisconnectCodeHostAccountTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn code_host_repository_context(
        &self,
        params_json: String,
    ) -> AsyncTask<CodeHostRepositoryContextTask> {
        AsyncTask::new(CodeHostRepositoryContextTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn prepare_code_host_push(
        &self,
        params_json: String,
    ) -> AsyncTask<PrepareCodeHostPushTask> {
        AsyncTask::new(PrepareCodeHostPushTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn apply_code_host_push(&self, params_json: String) -> AsyncTask<ApplyCodeHostPushTask> {
        AsyncTask::new(ApplyCodeHostPushTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn code_host_pull_requests(
        &self,
        params_json: String,
    ) -> AsyncTask<CodeHostPullRequestsTask> {
        AsyncTask::new(CodeHostPullRequestsTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn code_host_pull_request_detail(
        &self,
        params_json: String,
    ) -> AsyncTask<CodeHostPullRequestDetailTask> {
        AsyncTask::new(CodeHostPullRequestDetailTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn ensure_code_host_pull_request(
        &self,
        params_json: String,
    ) -> AsyncTask<EnsureCodeHostPullRequestTask> {
        AsyncTask::new(EnsureCodeHostPullRequestTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn code_host_pull_request_checks(
        &self,
        params_json: String,
    ) -> AsyncTask<CodeHostPullRequestChecksTask> {
        AsyncTask::new(CodeHostPullRequestChecksTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn code_host_pull_request_activity(
        &self,
        params_json: String,
    ) -> AsyncTask<CodeHostPullRequestActivityTask> {
        AsyncTask::new(CodeHostPullRequestActivityTask {
            client: get(&self.state),
            params_json,
        })
    }

    #[napi]
    pub fn create_code_host_pull_request_comment(
        &self,
        params_json: String,
    ) -> AsyncTask<CreateCodeHostPullRequestCommentTask> {
        AsyncTask::new(CreateCodeHostPullRequestCommentTask {
            client: get(&self.state),
            params_json,
        })
    }
}

pub struct CodeHostAccountsTask {
    client: Result<PersistentDaemonClient>,
}

impl napi::Task for CodeHostAccountsTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .map_err(|_| Error::from_reason("native daemon bridge is not started"))?;
        let mut client = client.fork_connection().map_err(code_host_fail)?;
        json(&client.code_host_accounts().map_err(code_host_fail)?)
    }

    fn resolve(&mut self, _: napi::Env, output: String) -> Result<String> {
        Ok(output)
    }
}

macro_rules! code_host_task {
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
                let client = self
                    .client
                    .as_ref()
                    .map_err(|_| Error::from_reason("native daemon bridge is not started"))?;
                let mut client = client.fork_connection().map_err(code_host_fail)?;
                json(&client.$method(params).map_err(code_host_fail)?)
            }

            fn resolve(&mut self, _: napi::Env, output: String) -> Result<String> {
                Ok(output)
            }
        }
    };
}

code_host_task!(
    ConnectCodeHostAccountTask,
    CodeHostAccountConnectParams,
    connect_code_host_account
);
code_host_task!(
    DisconnectCodeHostAccountTask,
    CodeHostAccountDisconnectParams,
    disconnect_code_host_account
);
code_host_task!(
    CodeHostRepositoryContextTask,
    CodeHostRepositoryContextParams,
    code_host_repository_context
);
code_host_task!(
    PrepareCodeHostPushTask,
    CodeHostPushPrepareParams,
    prepare_code_host_push
);
code_host_task!(
    ApplyCodeHostPushTask,
    CodeHostPushApplyParams,
    apply_code_host_push
);
code_host_task!(
    CodeHostPullRequestsTask,
    CodeHostPullRequestListParams,
    code_host_pull_requests
);
code_host_task!(
    CodeHostPullRequestDetailTask,
    CodeHostPullRequestDetailParams,
    code_host_pull_request_detail
);
code_host_task!(
    EnsureCodeHostPullRequestTask,
    CodeHostPullRequestEnsureParams,
    ensure_code_host_pull_request
);
code_host_task!(
    CodeHostPullRequestChecksTask,
    CodeHostPullRequestChecksParams,
    code_host_pull_request_checks
);
code_host_task!(
    CodeHostPullRequestActivityTask,
    CodeHostPullRequestActivityParams,
    code_host_pull_request_activity
);
code_host_task!(
    CreateCodeHostPullRequestCommentTask,
    CodeHostPullRequestCommentCreateParams,
    create_code_host_pull_request_comment
);

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ta_jsonrpc::{JsonRpcClientError, JsonRpcError, JsonRpcErrorObject};

    use super::*;

    fn remote_code(code: &str, message: &str) -> JsonRpcClientError {
        JsonRpcClientError::Remote(JsonRpcError::new(
            None,
            JsonRpcErrorObject::new(-32_000, message).with_data(json!({ "code": code })),
        ))
    }

    #[test]
    fn code_host_native_errors_expose_only_static_canonical_classifications() {
        assert_eq!(
            code_host_failure_reason(&remote_code(
                "CodeHostAuthenticationRequired",
                "private token and endpoint detail",
            )),
            NATIVE_CODE_HOST_AUTHENTICATION_REQUIRED
        );
        assert_eq!(
            code_host_failure_reason(&remote_code(
                "CodeHostPushConflict",
                "private repository state detail",
            )),
            NATIVE_CODE_HOST_PUSH_CONFLICT
        );
    }

    #[test]
    fn code_host_native_errors_keep_unknown_or_unstructured_failures_redacted() {
        assert_eq!(
            code_host_failure_reason(&remote_code("UnexpectedCode", "private detail")),
            NATIVE_DAEMON_OPERATION_FAILED
        );
        assert_eq!(
            code_host_failure_reason(&JsonRpcClientError::ConnectionClosed),
            NATIVE_DAEMON_OPERATION_FAILED
        );
    }
}
