use std::collections::HashMap;

use ta_protocol::wire::RuntimePolicyMode;

use crate::error::AcpClientError;

pub type ModeMapping = HashMap<RuntimePolicyMode, String>;

pub fn translate(
    runtime_mode: RuntimePolicyMode,
    mapping: &ModeMapping,
) -> Result<String, AcpClientError> {
    mapping.get(&runtime_mode).cloned().ok_or_else(|| {
        AcpClientError::InvalidConfig(format!(
            "ACP mode mapping missing runtime policy mode {runtime_mode:?}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_maps_all_runtime_policy_modes() {
        let mapping = HashMap::from([
            (RuntimePolicyMode::Allow, "allow".to_string()),
            (RuntimePolicyMode::RequireApproval, "approve".to_string()),
            (RuntimePolicyMode::Deny, "chat".to_string()),
        ]);

        assert_eq!(
            translate(RuntimePolicyMode::Allow, &mapping).expect("allow mode"),
            "allow"
        );
        assert_eq!(
            translate(RuntimePolicyMode::RequireApproval, &mapping).expect("approval mode"),
            "approve"
        );
        assert_eq!(
            translate(RuntimePolicyMode::Deny, &mapping).expect("deny mode"),
            "chat"
        );
    }

    #[test]
    fn translate_fails_fast_for_missing_mode() {
        let mapping = HashMap::from([(RuntimePolicyMode::Allow, "allow".to_string())]);

        let error = translate(RuntimePolicyMode::Deny, &mapping).expect_err("missing deny mode");

        assert!(
            matches!(error, AcpClientError::InvalidConfig(message) if message.contains("Deny"))
        );
    }
}
