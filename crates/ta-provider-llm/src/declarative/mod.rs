use std::sync::{Arc, OnceLock};

use crate::error::LlmClientError;
use include_dir::{Dir, include_dir};
use serde::Deserialize;

use crate::families::openai_compatible::AuthSource;

static DECLARATIVE_PROVIDER_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/declarative");
static DECLARATIVE_PROVIDER_SPECS: OnceLock<Vec<DeclarativeProviderSpec>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarativeProviderSpec {
    pub id: Arc<str>,
    pub family: DeclarativeProviderFamily,
    pub display_name: Arc<str>,
    pub description: Arc<str>,
    pub doc_url: Option<Arc<str>>,
    pub setup_steps: Vec<Arc<str>>,
    pub base_url: Arc<str>,
    pub completions_prefix: Arc<str>,
    pub auth: AuthSource,
    pub default_model: Arc<str>,
    pub fast_model: Option<Arc<str>>,
    pub models: Vec<DeclarativeModelSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DeclarativeProviderFamily {
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarativeModelSpec {
    pub id: Arc<str>,
    pub display_name: Arc<str>,
    pub context_limit: Option<u64>,
    pub input_token_cost_micros: Option<u64>,
    pub output_token_cost_micros: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeclarativeProviderSpec {
    id: String,
    family: DeclarativeProviderFamily,
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    doc_url: Option<String>,
    #[serde(default)]
    setup_steps: Vec<String>,
    base_url: String,
    #[serde(default)]
    completions_prefix: String,
    auth: RawAuthSource,
    default_model: String,
    fast_model: Option<String>,
    models: Vec<RawDeclarativeModelSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeclarativeModelSpec {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    context_limit: Option<u64>,
    #[serde(default)]
    input_token_cost_micros: Option<u64>,
    #[serde(default)]
    output_token_cost_micros: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAuthSource {
    kind: RawAuthKind,
    #[serde(default)]
    env: Option<String>,
    #[serde(default)]
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawAuthKind {
    BearerEnv,
    BearerStatic,
}

pub fn specs() -> &'static [DeclarativeProviderSpec] {
    DECLARATIVE_PROVIDER_SPECS.get_or_init(load_specs)
}

pub fn auth_env_var(spec: &DeclarativeProviderSpec) -> Option<&'static str> {
    match &spec.auth {
        AuthSource::BearerEnv(env_var) => Some(*env_var),
        AuthSource::BearerStatic(_) => None,
    }
}

fn load_specs() -> Vec<DeclarativeProviderSpec> {
    let mut specs = DECLARATIVE_PROVIDER_DIR
        .files()
        .filter(|file| {
            file.path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|file| {
            let contents = file
                .contents_utf8()
                .expect("embedded declarative provider JSON must be UTF-8");
            parse_spec(contents)
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("embedded declarative provider specs must parse");
    specs.sort_by(|left, right| left.id.cmp(&right.id));
    specs
}

fn parse_spec(contents: &str) -> Result<DeclarativeProviderSpec, LlmClientError> {
    let raw = serde_json::from_str::<RawDeclarativeProviderSpec>(contents).map_err(|error| {
        LlmClientError::InvalidConfig(format!("declarative provider JSON is invalid: {error}"))
    })?;
    raw.into_spec()
}

impl RawDeclarativeProviderSpec {
    fn into_spec(self) -> Result<DeclarativeProviderSpec, LlmClientError> {
        validate_required("id", &self.id)?;
        validate_required("display_name", &self.display_name)?;
        validate_required("base_url", &self.base_url)?;
        validate_required("default_model", &self.default_model)?;
        if self.models.is_empty() {
            return Err(LlmClientError::InvalidConfig(format!(
                "declarative provider {} must define at least one model",
                self.id
            )));
        }
        if !self
            .models
            .iter()
            .any(|model| model.id == self.default_model)
        {
            return Err(LlmClientError::InvalidConfig(format!(
                "declarative provider {} defaultModel {} is not in models",
                self.id, self.default_model
            )));
        }

        Ok(DeclarativeProviderSpec {
            id: Arc::from(self.id),
            family: self.family,
            display_name: Arc::from(self.display_name),
            description: Arc::from(self.description),
            doc_url: self.doc_url.map(Arc::from),
            setup_steps: self.setup_steps.into_iter().map(Arc::from).collect(),
            base_url: Arc::from(self.base_url),
            completions_prefix: Arc::from(self.completions_prefix),
            auth: self.auth.into_auth_source()?,
            default_model: Arc::from(self.default_model),
            fast_model: self.fast_model.map(Arc::from),
            models: self
                .models
                .into_iter()
                .map(|model| DeclarativeModelSpec {
                    display_name: Arc::from(model.display_name.unwrap_or_else(|| model.id.clone())),
                    id: Arc::from(model.id),
                    context_limit: model.context_limit,
                    input_token_cost_micros: model.input_token_cost_micros,
                    output_token_cost_micros: model.output_token_cost_micros,
                })
                .collect(),
        })
    }
}

impl RawAuthSource {
    fn into_auth_source(self) -> Result<AuthSource, LlmClientError> {
        match self.kind {
            RawAuthKind::BearerEnv => {
                let env = self.env.ok_or_else(|| {
                    LlmClientError::InvalidConfig(
                        "bearer_env declarative auth requires env".to_string(),
                    )
                })?;
                validate_required("auth.env", &env)?;
                Ok(AuthSource::BearerEnv(leak_static(env)))
            }
            RawAuthKind::BearerStatic => {
                let token = self.token.ok_or_else(|| {
                    LlmClientError::InvalidConfig(
                        "bearer_static declarative auth requires token".to_string(),
                    )
                })?;
                validate_required("auth.token", &token)?;
                Ok(AuthSource::BearerStatic(Arc::from(token)))
            }
        }
    }
}

fn validate_required(field: &str, value: &str) -> Result<(), LlmClientError> {
    if value.trim().is_empty() {
        return Err(LlmClientError::InvalidConfig(format!(
            "declarative provider field {field} must not be empty"
        )));
    }
    Ok(())
}

fn leak_static(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_declarative_specs_parse() {
        let ids = specs()
            .iter()
            .map(|spec| spec.id.as_ref())
            .collect::<Vec<_>>();

        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        for expected in ["deepseek", "groq", "openrouter", "xai"] {
            assert!(
                ids.contains(&expected),
                "embedded declarative specs must include {expected}"
            );
        }
    }

    #[test]
    fn loaded_specs_keep_provider_ids() {
        for spec in specs() {
            assert!(!spec.id.is_empty());
        }
    }

    #[test]
    fn declarative_loader_sanity() {
        loaded_specs_keep_provider_ids();
    }

    #[test]
    fn specs_preserve_declarative_model_metadata() {
        let spec = specs()
            .iter()
            .find(|spec| spec.id.as_ref() == "groq")
            .expect("groq spec");
        let model = spec
            .models
            .iter()
            .find(|model| model.id.as_ref() == "openai/gpt-oss-120b")
            .expect("groq model");

        assert_eq!(model.context_limit, Some(131072));
    }

    #[test]
    fn rejects_default_model_outside_catalog() {
        let error = parse_spec(
            r#"{
              "id": "bad",
              "family": "openai_compatible",
              "display_name": "Bad",
              "description": "Bad provider",
              "doc_url": null,
              "setup_steps": [],
              "base_url": "https://example.test/v1",
              "auth": {"kind": "bearer_env", "env": "BAD_API_KEY"},
              "default_model": "missing",
              "models": [{"id": "present", "display_name": "Present"}]
            }"#,
        )
        .expect_err("invalid default model must fail");

        assert!(matches!(error, LlmClientError::InvalidConfig(_)));
    }
}
