use serde_json::Value;

const REVOCATION_ERROR_CODES: &[&str] = &["invalid_grant", "invalid_token", "unauthorized_client"];

pub fn is_revocation_error(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(body.trim()) else {
        return false;
    };
    value
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(is_revocation_error_code)
}

pub(crate) fn is_revocation_error_code(code: &str) -> bool {
    REVOCATION_ERROR_CODES
        .iter()
        .any(|revocation_code| code.eq_ignore_ascii_case(revocation_code))
}

#[cfg(test)]
mod tests {
    use super::is_revocation_error;

    #[test]
    fn detects_explicit_revocation_errors() {
        assert!(is_revocation_error(r#"{"error":"invalid_grant"}"#));
        assert!(is_revocation_error(r#"{"error":"invalid_token"}"#));
        assert!(is_revocation_error(r#"{"error":"unauthorized_client"}"#));
    }

    #[test]
    fn rejects_transient_or_unparseable_errors() {
        assert!(!is_revocation_error(r#"{"error":"server_error"}"#));
        assert!(!is_revocation_error("proxy auth required"));
        assert!(!is_revocation_error(""));
    }
}
