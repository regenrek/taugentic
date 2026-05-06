use serde::Deserialize;
use serde_json::{Map, Value};

const MAX_JSON_RPC_ERROR_DATA_STRING_CHARS: usize = 256;
const MAX_JSON_RPC_ERROR_DATA_SUMMARY_CHARS: usize = 1024;
const MAX_JSON_RPC_ERROR_DATA_ARRAY_ITEMS: usize = 8;
const MAX_JSON_RPC_ERROR_DATA_OBJECT_FIELDS: usize = 16;
const TRUNCATED_SUFFIX: &str = "...[truncated]";
const REDACTED_VALUE: &str = "[redacted]";

pub(super) fn format_json_rpc_error(error: &JsonRpcError) -> String {
    let mut detail = error.message.clone();
    if let Some(code) = error.code {
        detail.push_str(&format!(" (code {code})"));
    }
    if let Some(data) = error.data.as_ref().and_then(json_rpc_error_data_summary) {
        detail.push_str(": ");
        detail.push_str(&data);
    }
    detail
}

fn json_rpc_error_data_summary(data: &Value) -> Option<String> {
    if data.is_null() {
        return None;
    }
    if let Some(message) = data.as_str() {
        return Some(sanitize_json_rpc_error_data_string(message));
    }
    let sanitized = sanitize_json_rpc_error_data(data);
    serde_json::to_string(&sanitized)
        .ok()
        .map(|summary| truncate_chars(&summary, MAX_JSON_RPC_ERROR_DATA_SUMMARY_CHARS))
}

fn sanitize_json_rpc_error_data(data: &Value) -> Value {
    match data {
        Value::Null | Value::Bool(_) | Value::Number(_) => data.clone(),
        Value::String(value) => Value::String(sanitize_json_rpc_error_data_string(value)),
        Value::Array(values) => {
            let mut sanitized = values
                .iter()
                .take(MAX_JSON_RPC_ERROR_DATA_ARRAY_ITEMS)
                .map(sanitize_json_rpc_error_data)
                .collect::<Vec<_>>();
            if values.len() > MAX_JSON_RPC_ERROR_DATA_ARRAY_ITEMS {
                sanitized.push(Value::String(format!(
                    "{} additional items truncated",
                    values.len() - MAX_JSON_RPC_ERROR_DATA_ARRAY_ITEMS
                )));
            }
            Value::Array(sanitized)
        }
        Value::Object(fields) => {
            let mut sanitized = Map::new();
            for key in prioritized_error_data_keys(fields) {
                if let Some(value) = fields.get(&key) {
                    let value = if is_sensitive_error_data_key(&key, value) {
                        Value::String(REDACTED_VALUE.to_string())
                    } else {
                        sanitize_json_rpc_error_data(value)
                    };
                    sanitized.insert(key, value);
                }
            }
            if fields.len() > MAX_JSON_RPC_ERROR_DATA_OBJECT_FIELDS {
                sanitized.insert(
                    "_truncatedFields".to_string(),
                    Value::String(format!(
                        "{} additional fields truncated",
                        fields.len() - MAX_JSON_RPC_ERROR_DATA_OBJECT_FIELDS
                    )),
                );
            }
            Value::Object(sanitized)
        }
    }
}

fn prioritized_error_data_keys(fields: &Map<String, Value>) -> Vec<String> {
    let mut keys = Vec::new();
    for priority_key in ["reason", "message", "error", "detail"] {
        if fields.contains_key(priority_key) {
            keys.push(priority_key.to_string());
        }
    }
    for key in fields.keys() {
        if keys.iter().any(|existing| existing == key) {
            continue;
        }
        if keys.len() >= MAX_JSON_RPC_ERROR_DATA_OBJECT_FIELDS {
            break;
        }
        keys.push(key.clone());
    }
    keys
}

fn is_sensitive_error_data_key(key: &str, value: &Value) -> bool {
    let key = key.to_ascii_lowercase();
    if matches!(
        key.as_str(),
        "token"
            | "access_token"
            | "refresh_token"
            | "secret"
            | "password"
            | "api_key"
            | "apikey"
            | "authorization"
            | "bearer"
            | "credential"
            | "credentials"
    ) {
        return true;
    }
    if key.contains("password")
        || key.contains("secret")
        || key.contains("token")
        || key.contains("authorization")
        || key.contains("credential")
    {
        return true;
    }
    if key.contains("key")
        && (key.contains("api")
            || key.contains("auth")
            || key.contains("secret")
            || key.contains("token")
            || key.contains("access")
            || key.contains("private")
            || value.as_str().is_some_and(looks_secret_like))
    {
        return true;
    }
    key == "key" && value.as_str().is_some_and(looks_secret_like)
}

fn sanitize_json_rpc_error_data_string(value: &str) -> String {
    if looks_secret_like(value) {
        return REDACTED_VALUE.to_string();
    }
    let redacted = redact_secret_fragments(value);
    truncate_chars(&redacted, MAX_JSON_RPC_ERROR_DATA_STRING_CHARS)
}

fn redact_secret_fragments(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let mut index = 0;

    while index < value.len() {
        if let Some((replacement, end)) = bearer_fragment_at(value, index) {
            redacted.push_str(&replacement);
            index = end;
            continue;
        }
        if let Some(end) = sk_token_fragment_at(value, index) {
            redacted.push_str(REDACTED_VALUE);
            index = end;
            continue;
        }

        let next = value[index..]
            .chars()
            .next()
            .expect("index is within the string");
        redacted.push(next);
        index += next.len_utf8();
    }

    redacted
}

fn bearer_fragment_at(value: &str, index: usize) -> Option<(String, usize)> {
    const BEARER: &str = "bearer";

    if !is_secret_fragment_boundary(value, index)
        || !starts_with_ignore_ascii_case_at(value, index, BEARER)
    {
        return None;
    }

    let bearer_end = index + BEARER.len();
    let mut whitespace_end = bearer_end;
    for (offset, ch) in value[bearer_end..].char_indices() {
        if !ch.is_whitespace() {
            break;
        }
        whitespace_end = bearer_end + offset + ch.len_utf8();
    }
    if whitespace_end == bearer_end || whitespace_end >= value.len() {
        return None;
    }

    let token_start = whitespace_end;
    let mut token_end = token_start;
    for (offset, ch) in value[token_start..].char_indices() {
        if ch.is_whitespace() {
            break;
        }
        token_end = token_start + offset + ch.len_utf8();
    }
    if token_end == token_start {
        return None;
    }

    Some((
        format!(
            "{}{}{}",
            &value[index..bearer_end],
            &value[bearer_end..token_start],
            REDACTED_VALUE
        ),
        token_end,
    ))
}

fn sk_token_fragment_at(value: &str, index: usize) -> Option<usize> {
    if !is_secret_fragment_boundary(value, index)
        || !starts_with_ignore_ascii_case_at(value, index, "sk-")
    {
        return None;
    }

    let token_start = index + "sk-".len();
    let mut token_end = token_start;
    for (offset, ch) in value[token_start..].char_indices() {
        if !is_token_fragment_char(ch) {
            break;
        }
        token_end = token_start + offset + ch.len_utf8();
    }
    (token_end > token_start).then_some(token_end)
}

fn starts_with_ignore_ascii_case_at(value: &str, index: usize, pattern: &str) -> bool {
    value[index..]
        .get(..pattern.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(pattern))
}

fn is_secret_fragment_boundary(value: &str, index: usize) -> bool {
    match value[..index].chars().next_back() {
        Some(ch) => !is_token_fragment_char(ch),
        None => true,
    }
}

fn is_token_fragment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

fn looks_secret_like(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("bearer ") || lower.starts_with("sk-") {
        return true;
    }
    trimmed.len() >= 16
        && !trimmed.chars().any(|ch| ch.is_whitespace())
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(TRUNCATED_SUFFIX.len());
    let mut truncated = value.chars().take(keep).collect::<String>();
    truncated.push_str(TRUNCATED_SUFFIX);
    truncated
}

#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcError {
    pub(super) code: Option<i64>,
    message: String,
    data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_rpc_error_data_summary_preserves_code_and_reason() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "reason": "model requires a newer runtime",
                "detail": { "message": "upgrade the provider" },
            })),
        });

        assert!(detail.contains("Internal error (code -32603)"));
        assert!(detail.contains("model requires a newer runtime"));
        assert!(detail.contains("upgrade the provider"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_secret_like_fields() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "reason": "provider rejected credentials",
                "authorization": "Bearer live-secret-token",
                "nested": {
                    "api_key": "sk-live-1234567890",
                    "password": "hunter2",
                    "key": "abc1234567890secret",
                    "message": "authentication failed",
                },
            })),
        });

        assert!(detail.contains("provider rejected credentials"));
        assert!(detail.contains("authentication failed"));
        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("Bearer live-secret-token"));
        assert!(!detail.contains("sk-live-1234567890"));
        assert!(!detail.contains("hunter2"));
        assert!(!detail.contains("abc1234567890secret"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_top_level_secret_like_string() {
        let secret_detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!("Bearer live-secret-token")),
        });
        let reason_detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!("model requires a newer runtime")),
        });

        assert!(secret_detail.contains("Internal error (code -32603)"));
        assert!(secret_detail.contains(REDACTED_VALUE));
        assert!(!secret_detail.contains("Bearer live-secret-token"));
        assert!(reason_detail.contains("Internal error (code -32603)"));
        assert!(reason_detail.contains("model requires a newer runtime"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_embedded_top_level_secret_fragment() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!(
                "request failed with Bearer live-secret-token after provider rejection"
            )),
        });

        assert!(detail.contains("request failed with Bearer"));
        assert!(detail.contains("after provider rejection"));
        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("live-secret-token"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_nested_secret_like_string_under_non_sensitive_key() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "reason": "provider rejected credentials",
                "detail": "Bearer live-secret-token",
            })),
        });

        assert!(detail.contains("provider rejected credentials"));
        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("Bearer live-secret-token"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_embedded_nested_secret_fragment() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "reason": "provider rejected credentials",
                "detail": "retry failed after token sk-live-1234567890 was rejected",
            })),
        });

        assert!(detail.contains("provider rejected credentials"));
        assert!(detail.contains("retry failed after token"));
        assert!(detail.contains("was rejected"));
        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("sk-live-1234567890"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_secret_like_string_in_array() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "items": ["sk-live-1234567890"],
            })),
        });

        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("sk-live-1234567890"));
    }

    #[test]
    fn json_rpc_error_data_summary_redacts_embedded_array_secret_fragment() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "items": [
                    "first attempt failed with Bearer live-secret-token",
                    "second attempt preserved normal diagnostic context",
                ],
            })),
        });

        assert!(detail.contains("first attempt failed with Bearer"));
        assert!(detail.contains("second attempt preserved normal diagnostic context"));
        assert!(detail.contains(REDACTED_VALUE));
        assert!(!detail.contains("live-secret-token"));
    }

    #[test]
    fn json_rpc_error_data_summary_preserves_normal_nested_diagnostic_string() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "detail": {
                    "message": "model requires a newer runtime",
                },
            })),
        });

        assert!(detail.contains("model requires a newer runtime"));
        assert!(!detail.contains(REDACTED_VALUE));
    }

    #[test]
    fn json_rpc_error_data_summary_truncates_long_payloads() {
        let long_detail = "x".repeat(8_000);
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(json!({
                "reason": "payload too large",
                "detail": long_detail,
                "items": (0..32)
                    .map(|index| json!({ "message": format!("item-{index}") }))
                    .collect::<Vec<_>>(),
            })),
        });

        assert!(detail.contains("payload too large"));
        assert!(detail.contains(TRUNCATED_SUFFIX));
        assert!(
            detail.len()
                <= "Internal error (code -32603): ".len() + MAX_JSON_RPC_ERROR_DATA_SUMMARY_CHARS
        );
        assert!(!detail.contains(&"x".repeat(MAX_JSON_RPC_ERROR_DATA_STRING_CHARS + 1)));
    }

    #[test]
    fn json_rpc_error_data_summary_omits_null_data() {
        let detail = format_json_rpc_error(&JsonRpcError {
            code: Some(-32603),
            message: "Internal error".to_string(),
            data: Some(Value::Null),
        });

        assert_eq!(detail, "Internal error (code -32603)");
    }
}
