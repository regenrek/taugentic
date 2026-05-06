use serde_json::Value;

const REDACTED: &str = "<redacted>";
const SENSITIVE_OAUTH_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "id_token",
    "code",
    "code_verifier",
    "client_secret",
    "authorization",
    "bearer",
    "token",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenEndpointErrorDetail {
    pub error_code: Option<String>,
    pub message: String,
}

pub fn redact_oauth_error_text(text: &str) -> String {
    let json_redacted = redact_json_text(text).unwrap_or_else(|| text.to_string());
    redact_query_pairs(&json_redacted)
}

pub fn parse_token_endpoint_error(body: &str) -> TokenEndpointErrorDetail {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return TokenEndpointErrorDetail {
            error_code: None,
            message: "empty response body".to_string(),
        };
    }

    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return TokenEndpointErrorDetail {
            error_code: None,
            message: redact_oauth_error_text(trimmed),
        };
    };

    if let Some(error) = value.get("error") {
        if let Some(error_code) = error.as_str() {
            let description = value
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or(error_code);
            return TokenEndpointErrorDetail {
                error_code: Some(error_code.to_string()),
                message: redact_oauth_error_text(description),
            };
        }

        if let Some(error_object) = error.as_object() {
            let error_code = error_object
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string);
            let message = error_object
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| error_code.clone())
                .unwrap_or_else(|| "OAuth token endpoint error".to_string());
            return TokenEndpointErrorDetail {
                error_code,
                message: redact_oauth_error_text(&message),
            };
        }
    }

    TokenEndpointErrorDetail {
        error_code: None,
        message: "OAuth token endpoint error".to_string(),
    }
}

fn redact_json_text(text: &str) -> Option<String> {
    let mut value = serde_json::from_str::<Value>(text).ok()?;
    redact_json_value(&mut value);
    serde_json::to_string(&value).ok()
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if is_sensitive_key(key) {
                    *value = Value::String(REDACTED.to_string());
                } else {
                    redact_json_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_json_value(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn redact_query_pairs(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    for (index, part) in text.split('&').enumerate() {
        if index > 0 {
            redacted.push('&');
        }
        let Some((key, _value)) = part.split_once('=') else {
            redacted.push_str(part);
            continue;
        };
        redacted.push_str(key);
        redacted.push('=');
        if is_sensitive_key(key.rsplit(['?', ' ', '"', '\'']).next().unwrap_or(key)) {
            redacted.push_str(REDACTED);
        } else {
            redacted.push_str(&part[key.len() + 1..]);
        }
    }
    redacted
}

fn is_sensitive_key(key: &str) -> bool {
    SENSITIVE_OAUTH_KEYS
        .iter()
        .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
}

#[cfg(test)]
mod tests {
    use super::{TokenEndpointErrorDetail, parse_token_endpoint_error, redact_oauth_error_text};

    #[test]
    fn parse_token_endpoint_error_prefers_description() {
        assert_eq!(
            parse_token_endpoint_error(
                r#"{"error":"invalid_grant","error_description":"refresh token expired"}"#
            ),
            TokenEndpointErrorDetail {
                error_code: Some("invalid_grant".to_string()),
                message: "refresh token expired".to_string()
            }
        );
    }

    #[test]
    fn parse_token_endpoint_error_reads_nested_shape() {
        assert_eq!(
            parse_token_endpoint_error(
                r#"{"error":{"code":"proxy_auth_required","message":"proxy auth required"}}"#
            ),
            TokenEndpointErrorDetail {
                error_code: Some("proxy_auth_required".to_string()),
                message: "proxy auth required".to_string()
            }
        );
    }

    #[test]
    fn redact_oauth_error_text_strips_sensitive_json_keys_case_insensitively() {
        let redacted = redact_oauth_error_text(
            r#"{"error":"invalid_grant","Access_Token":"fake-access","nested":{"refresh_token":"fake-refresh","id_token":"fake-id"}}"#,
        );

        assert!(redacted.contains(r#""Access_Token":"<redacted>""#));
        assert!(redacted.contains(r#""refresh_token":"<redacted>""#));
        assert!(redacted.contains(r#""id_token":"<redacted>""#));
        assert!(!redacted.contains("fake-access"));
        assert!(!redacted.contains("fake-refresh"));
        assert!(!redacted.contains("fake-id"));
    }

    #[test]
    fn redact_oauth_error_text_strips_sensitive_query_keys_case_insensitively() {
        let redacted = redact_oauth_error_text(
            "error=bad&access_token=fake-access&Refresh_Token=fake-refresh&code_verifier=fake-verifier",
        );

        assert_eq!(
            redacted,
            "error=bad&access_token=<redacted>&Refresh_Token=<redacted>&code_verifier=<redacted>"
        );
    }

    #[test]
    fn parse_token_endpoint_error_redacts_description_before_display() {
        let detail = parse_token_endpoint_error(
            r#"{"error":"invalid_grant","error_description":"refresh failed access_token=fake-access&refresh_token=fake-refresh"}"#,
        );

        assert_eq!(detail.error_code.as_deref(), Some("invalid_grant"));
        assert!(detail.message.contains("access_token=<redacted>"));
        assert!(detail.message.contains("refresh_token=<redacted>"));
        assert!(!detail.message.contains("fake-access"));
        assert!(!detail.message.contains("fake-refresh"));
    }
}
