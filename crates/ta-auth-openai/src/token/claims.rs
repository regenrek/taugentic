use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::OAuthError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatGptAccountInfo {
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub organization_id: Option<String>,
    pub user_id: Option<String>,
    pub plan_type: Option<String>,
    pub is_fedramp: Option<bool>,
    pub expires_at: Option<u64>,
}

pub fn parse_chatgpt_account_info(jwt: &str) -> Result<ChatGptAccountInfo, OAuthError> {
    let payload = decode_jwt_payload(jwt)?;
    let auth_claims = payload
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object);

    Ok(ChatGptAccountInfo {
        email: string_claim(&payload, auth_claims, "email", "profile.email"),
        account_id: string_claim(&payload, auth_claims, "", "chatgpt_account_id"),
        organization_id: string_claim(&payload, auth_claims, "organization_id", "organization_id"),
        user_id: string_claim(&payload, auth_claims, "user_id", "chatgpt_user_id"),
        plan_type: string_claim(&payload, auth_claims, "", "chatgpt_plan_type"),
        is_fedramp: bool_claim(auth_claims, "chatgpt_account_is_fedramp"),
        expires_at: payload.get("exp").and_then(Value::as_u64),
    })
}

fn decode_jwt_payload(jwt: &str) -> Result<Value, OAuthError> {
    let mut parts = jwt.split('.');
    let header = parts.next();
    let payload = parts.next();
    let signature = parts.next();
    if header.is_none() || payload.is_none() || signature.is_none() || parts.next().is_some() {
        return Err(OAuthError::InvalidJwt(
            "expected three JWT segments".to_string(),
        ));
    }

    let payload = payload
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| OAuthError::InvalidJwt("missing JWT payload segment".to_string()))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| OAuthError::InvalidJwt("payload is not base64url".to_string()))?;
    serde_json::from_slice::<Value>(&bytes)
        .map_err(|_| OAuthError::InvalidJwt("payload is not JSON".to_string()))
}

fn string_claim(
    payload: &Value,
    auth_claims: Option<&serde_json::Map<String, Value>>,
    direct_key: &str,
    namespaced_key: &str,
) -> Option<String> {
    if !direct_key.is_empty()
        && let Some(value) = payload.get(direct_key).and_then(Value::as_str)
    {
        return Some(value.to_string());
    }

    let direct_namespaced_key = format!("https://api.openai.com/{namespaced_key}");
    if let Some(value) = payload
        .get(&direct_namespaced_key)
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Some(value);
    }

    let full_key = format!("https://api.openai.com/auth.{namespaced_key}");
    payload
        .get(&full_key)
        .and_then(Value::as_str)
        .or_else(|| {
            auth_claims
                .and_then(|claims| claims.get(namespaced_key))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn bool_claim(
    auth_claims: Option<&serde_json::Map<String, Value>>,
    namespaced_key: &str,
) -> Option<bool> {
    auth_claims
        .and_then(|claims| claims.get(namespaced_key))
        .and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use serde_json::json;

    use super::parse_chatgpt_account_info;

    #[test]
    fn parse_chatgpt_account_info_reads_nested_auth_claims()
    -> Result<(), Box<dyn std::error::Error>> {
        let payload = json!({
            "exp": 1_800_000_000_u64,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc_123",
                "organization_id": "org_123",
                "chatgpt_user_id": "user_123",
                "chatgpt_plan_type": "plus",
                "chatgpt_account_is_fedramp": false
            },
            "https://api.openai.com/profile.email": "user@example.com"
        });
        let encoded_payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?);
        let jwt = format!("e30.{encoded_payload}.sig");

        let claims = parse_chatgpt_account_info(&jwt)?;

        assert_eq!(claims.account_id.as_deref(), Some("acc_123"));
        assert_eq!(claims.organization_id.as_deref(), Some("org_123"));
        assert_eq!(claims.user_id.as_deref(), Some("user_123"));
        assert_eq!(claims.plan_type.as_deref(), Some("plus"));
        assert_eq!(claims.email.as_deref(), Some("user@example.com"));
        assert_eq!(claims.is_fedramp, Some(false));
        assert_eq!(claims.expires_at, Some(1_800_000_000));
        Ok(())
    }
}
