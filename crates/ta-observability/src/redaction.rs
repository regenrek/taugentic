use serde_json::Value;

pub const REDACTED_VALUE: &str = "[REDACTED]";

pub fn redact_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let redacted = if is_sensitive_key(key) {
                        Value::String(REDACTED_VALUE.to_string())
                    } else {
                        redact_json_value(value)
                    };
                    (key.clone(), redacted)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact_json_value).collect()),
        _ => value.clone(),
    }
}

pub fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    [
        "apikey",
        "authorization",
        "accesstoken",
        "refreshtoken",
        "bearertoken",
        "authtoken",
        "sessiontoken",
        "cookie",
        "password",
        "passwd",
        "secret",
        "clientsecret",
        "token",
    ]
    .into_iter()
    .any(|needle| normalized == needle || normalized.ends_with(needle))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{REDACTED_VALUE, is_sensitive_key, redact_json_value};

    #[test]
    fn redacts_nested_sensitive_keys() {
        let value = json!({
            "profile": {
                "apiKey": "secret-value",
                "tokenCount": 42,
                "nested": {
                    "authorization": "Bearer abc"
                }
            }
        });

        assert_eq!(
            redact_json_value(&value),
            json!({
                "profile": {
                    "apiKey": REDACTED_VALUE,
                    "tokenCount": 42,
                    "nested": {
                        "authorization": REDACTED_VALUE
                    }
                }
            })
        );
    }

    #[test]
    fn detects_sensitive_keys_case_insensitively() {
        assert!(is_sensitive_key("ApiKey"));
        assert!(is_sensitive_key("refresh_token"));
        assert!(is_sensitive_key("openai_api_key"));
        assert!(is_sensitive_key("github_auth_token"));
        assert!(!is_sensitive_key("tokenCount"));
        assert!(!is_sensitive_key("socket_path"));
    }
}
