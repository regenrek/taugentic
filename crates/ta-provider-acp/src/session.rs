use serde_json::Value;

use crate::error::AcpClientError;

#[derive(Debug)]
pub struct AcpSession {
    pub id: String,
    current_mode_id: Option<String>,
    available_modes: Option<Vec<String>>,
    current_model_id: Option<String>,
    available_models: Option<Vec<String>>,
}

impl AcpSession {
    pub fn from_new_session_result(result: &Value) -> Result<Self, AcpClientError> {
        let id = result
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                AcpClientError::ProcessFailed("ACP session/new missing sessionId".to_string())
            })?;
        Ok(Self {
            id,
            current_mode_id: current_mode_id(result),
            available_modes: available_mode_ids(result),
            current_model_id: current_model_id(result),
            available_models: available_model_ids(result),
        })
    }

    pub fn needs_mode_update(&self, mode_id: &str) -> Result<bool, AcpClientError> {
        if self.current_mode_id.as_deref() == Some(mode_id) {
            return Ok(false);
        }
        self.ensure_mode_available(mode_id)?;
        Ok(true)
    }

    pub fn needs_model_update(&self, model_id: &str) -> Result<bool, AcpClientError> {
        if self.current_model_id.as_deref() == Some(model_id) {
            return Ok(false);
        }
        self.ensure_model_available(model_id)?;
        Ok(true)
    }

    fn ensure_mode_available(&self, mode_id: &str) -> Result<(), AcpClientError> {
        let Some(available_modes) = &self.available_modes else {
            return Err(AcpClientError::InvalidConfig(format!(
                "ACP agent did not advertise session modes required for mode {mode_id}"
            )));
        };
        if available_modes.iter().any(|candidate| candidate == mode_id) {
            return Ok(());
        }
        Err(AcpClientError::InvalidConfig(format!(
            "ACP session mode {mode_id} was not advertised by session/new"
        )))
    }

    fn ensure_model_available(&self, model_id: &str) -> Result<(), AcpClientError> {
        let Some(available_models) = &self.available_models else {
            return Err(AcpClientError::InvalidConfig(format!(
                "ACP agent did not advertise session models required for model {model_id}"
            )));
        };
        if available_models
            .iter()
            .any(|candidate| candidate == model_id)
        {
            return Ok(());
        }
        let available = available_models.join(", ");
        Err(AcpClientError::InvalidConfig(format!(
            "ACP session model {model_id} was not advertised by session/new; available models: {available}"
        )))
    }
}

fn current_mode_id(result: &Value) -> Option<String> {
    result
        .pointer("/modes/currentModeId")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn available_mode_ids(result: &Value) -> Option<Vec<String>> {
    result
        .pointer("/modes/availableModes")
        .and_then(Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(|mode| mode.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
}

fn current_model_id(result: &Value) -> Option<String> {
    result
        .pointer("/models/currentModelId")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn available_model_ids(result: &Value) -> Option<Vec<String>> {
    result
        .pointer("/models/availableModels")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    model
                        .get("modelId")
                        .or_else(|| model.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
}
