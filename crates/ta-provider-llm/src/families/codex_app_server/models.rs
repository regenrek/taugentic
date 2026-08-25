use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ta_protocol::wire::{AgentRuntimeModelId, AgentRuntimeModelRef};

use super::{CodexAppServerClient, CodexLlmClientError};

const MODEL_CATALOG_TTL: Duration = Duration::from_secs(300);
const MODEL_CATALOG_ERROR_TTL: Duration = Duration::from_secs(15);

static MODEL_CATALOG_CACHE: OnceLock<Mutex<Option<CachedCatalog>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexModelCatalog {
    pub models: Vec<AgentRuntimeModelRef>,
    pub default_model_id: Option<AgentRuntimeModelId>,
}

#[derive(Debug, Clone)]
struct CachedCatalog {
    observed_at: Instant,
    value: Result<CodexModelCatalog, CodexLlmClientError>,
}

pub fn model_catalog() -> Result<CodexModelCatalog, CodexLlmClientError> {
    let cache = MODEL_CATALOG_CACHE.get_or_init(|| Mutex::new(None));
    let mut cached = cache
        .lock()
        .expect("Codex model catalog cache should not be poisoned");
    if let Some(entry) = cached.as_ref() {
        let ttl = if entry.value.is_ok() {
            MODEL_CATALOG_TTL
        } else {
            MODEL_CATALOG_ERROR_TTL
        };
        if entry.observed_at.elapsed() < ttl {
            return entry.value.clone();
        }
    }

    let value = fetch_model_catalog();
    *cached = Some(CachedCatalog {
        observed_at: Instant::now(),
        value: value.clone(),
    });
    value
}

fn fetch_model_catalog() -> Result<CodexModelCatalog, CodexLlmClientError> {
    run_on_control_thread(fetch_model_catalog_on_control_thread)
}

fn run_on_control_thread<T, F>(task: F) -> Result<T, CodexLlmClientError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CodexLlmClientError> + Send + 'static,
{
    std::thread::spawn(task).join().map_err(|_| {
        CodexLlmClientError::CommandFailed("Codex control worker panicked".to_string())
    })?
}

fn fetch_model_catalog_on_control_thread() -> Result<CodexModelCatalog, CodexLlmClientError> {
    let mut session = CodexAppServerClient::default().start_control_session()?;
    let mut cursor = None;
    let mut models = Vec::new();
    let mut model_ids = BTreeSet::new();
    let mut default_model_id = None;

    loop {
        let result = session.request(
            "model/list",
            json!({"cursor": cursor, "includeHidden": false, "limit": 100}),
        )?;
        let data = result
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CodexLlmClientError::Protocol("model/list response missing data".to_string())
            })?;
        for value in data {
            let model = parse_model(value)?;
            if !model_ids.insert(model.id.clone()) {
                return Err(CodexLlmClientError::Protocol(format!(
                    "model/list returned duplicate model {}",
                    model.id.as_str()
                )));
            }
            if value.get("isDefault").and_then(Value::as_bool) == Some(true) {
                if default_model_id.replace(model.id.clone()).is_some() {
                    return Err(CodexLlmClientError::Protocol(
                        "model/list returned more than one default model".to_string(),
                    ));
                }
            }
            models.push(model);
        }
        cursor = result
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }

    Ok(CodexModelCatalog {
        models,
        default_model_id,
    })
}

fn parse_model(value: &Value) -> Result<AgentRuntimeModelRef, CodexLlmClientError> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| CodexLlmClientError::Protocol("model/list model missing id".to_string()))?;
    let display_name = value
        .get("displayName")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CodexLlmClientError::Protocol("model/list model missing displayName".to_string())
        })?;
    Ok(AgentRuntimeModelRef {
        id: AgentRuntimeModelId::new(id).map_err(|error| {
            CodexLlmClientError::Protocol(format!("model/list returned invalid id: {error}"))
        })?,
        display_name: display_name.to_string(),
        context_limit: None,
        input_token_cost_micros: None,
        output_token_cost_micros: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_codex_model_list_shape() {
        let model = parse_model(&json!({
            "id": "gpt-5.6-sol",
            "displayName": "GPT-5.6-Sol",
            "isDefault": true
        }))
        .expect("model");

        assert_eq!(model.id.as_str(), "gpt-5.6-sol");
        assert_eq!(model.display_name, "GPT-5.6-Sol");
    }

    #[test]
    fn rejects_incomplete_codex_model_list_shape() {
        let error = parse_model(&json!({"id": "gpt-5.6-sol"})).expect_err("invalid model");

        assert!(matches!(error, CodexLlmClientError::Protocol(_)));
    }

    #[test]
    fn control_worker_may_run_a_runtime_when_called_from_tokio() {
        let caller = tokio::runtime::Runtime::new().expect("caller runtime");

        let value = caller.block_on(async {
            run_on_control_thread(|| {
                let worker = tokio::runtime::Runtime::new().expect("worker runtime");
                Ok(worker.block_on(async { 7 }))
            })
        });

        assert_eq!(value.expect("control worker result"), 7);
    }
}
