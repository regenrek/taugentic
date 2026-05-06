use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! identifier {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema, TS)]
        #[schemars(transparent)]
        #[ts(export_to = "generated/")]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, crate::wire::DomainError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(crate::wire::DomainError::EmptyIdentifier($label));
                }

                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use identifier;

identifier!(SessionId, "session");
identifier!(SessionAuthority, "session authority");
identifier!(RunId, "run");
identifier!(StepId, "step");
identifier!(ApprovalId, "approval");
identifier!(ArtifactId, "artifact");
