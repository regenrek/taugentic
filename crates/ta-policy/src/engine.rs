use ta_protocol::wire::RuntimePolicyMode;

use crate::{Operation, PolicyDecision, evaluate_runtime_policy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEngine {
    mode: RuntimePolicyMode,
}

impl PolicyEngine {
    pub fn from_runtime_policy_mode(mode: RuntimePolicyMode) -> Self {
        Self { mode }
    }

    pub fn mode(&self) -> RuntimePolicyMode {
        self.mode
    }

    pub fn evaluate(&self, operation: &Operation, lane_allows_network: bool) -> PolicyDecision {
        evaluate_runtime_policy(self.mode, operation, lane_allows_network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::ApprovalScope;

    #[test]
    fn require_approval_mode_requires_supported_operations() {
        let engine = PolicyEngine::from_runtime_policy_mode(RuntimePolicyMode::RequireApproval);

        let file_write = engine.evaluate(
            &Operation::new(ApprovalScope::FileWrite, "apply patch"),
            true,
        );
        let process_exec = engine.evaluate(
            &Operation::new(ApprovalScope::ProcessExec, "spawn command"),
            true,
        );
        let network = engine.evaluate(
            &Operation::new(ApprovalScope::NetworkAccess, "fetch metadata"),
            true,
        );

        assert_eq!(
            file_write,
            PolicyDecision::RequireApproval {
                reason: "apply patch writes to the workspace".to_string(),
            }
        );
        assert_eq!(
            process_exec,
            PolicyDecision::RequireApproval {
                reason: "spawn command executes a process".to_string(),
            }
        );
        assert_eq!(
            network,
            PolicyDecision::RequireApproval {
                reason: "fetch metadata requires network access".to_string(),
            }
        );
    }

    #[test]
    fn allow_mode_allows_supported_operations() {
        let engine = PolicyEngine::from_runtime_policy_mode(RuntimePolicyMode::Allow);

        let file_write = engine.evaluate(
            &Operation::new(ApprovalScope::FileWrite, "apply patch"),
            true,
        );
        let process_exec = engine.evaluate(
            &Operation::new(ApprovalScope::ProcessExec, "spawn command"),
            true,
        );
        let network = engine.evaluate(
            &Operation::new(ApprovalScope::NetworkAccess, "fetch metadata"),
            true,
        );

        assert_eq!(file_write, PolicyDecision::Allow);
        assert_eq!(process_exec, PolicyDecision::Allow);
        assert_eq!(network, PolicyDecision::Allow);
    }

    #[test]
    fn deny_mode_denies_supported_operations() {
        let engine = PolicyEngine::from_runtime_policy_mode(RuntimePolicyMode::Deny);

        let file_write = engine.evaluate(
            &Operation::new(ApprovalScope::FileWrite, "apply patch"),
            true,
        );
        let process_exec = engine.evaluate(
            &Operation::new(ApprovalScope::ProcessExec, "spawn command"),
            true,
        );
        let network = engine.evaluate(
            &Operation::new(ApprovalScope::NetworkAccess, "fetch metadata"),
            true,
        );

        assert_eq!(
            file_write,
            PolicyDecision::Deny {
                reason: "apply patch is denied by the active runtime policy".to_string(),
            }
        );
        assert_eq!(
            process_exec,
            PolicyDecision::Deny {
                reason: "spawn command is denied by the active runtime policy".to_string(),
            }
        );
        assert_eq!(
            network,
            PolicyDecision::Deny {
                reason:
                    "fetch metadata requires network access but the active runtime policy denies it"
                        .to_string(),
            }
        );
    }

    #[test]
    fn network_lane_denial_overrides_allow_mode() {
        let decision = PolicyEngine::from_runtime_policy_mode(RuntimePolicyMode::Allow).evaluate(
            &Operation::new(ApprovalScope::NetworkAccess, "fetch metadata"),
            false,
        );

        assert_eq!(
            decision,
            PolicyDecision::Deny {
                reason:
                    "fetch metadata requires network access but the active lane does not allow it"
                        .to_string(),
            }
        );
    }
}
