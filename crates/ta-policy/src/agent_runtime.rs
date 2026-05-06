use ta_protocol::wire::{ApprovalScope, RuntimePolicyMode};

use crate::{Operation, PolicyDecision};

pub fn evaluate_runtime_policy(
    mode: RuntimePolicyMode,
    operation: &Operation,
    lane_allows_network: bool,
) -> PolicyDecision {
    if matches!(operation.scope, ApprovalScope::NetworkAccess) && !lane_allows_network {
        return PolicyDecision::Deny {
            reason: format!(
                "{} requires network access but the active lane does not allow it",
                operation.label
            ),
        };
    }

    match mode {
        RuntimePolicyMode::RequireApproval => PolicyDecision::RequireApproval {
            reason: approval_reason(operation),
        },
        RuntimePolicyMode::Allow => PolicyDecision::Allow,
        RuntimePolicyMode::Deny => PolicyDecision::Deny {
            reason: deny_reason(operation),
        },
    }
}

fn approval_reason(operation: &Operation) -> String {
    match operation.scope {
        ApprovalScope::FileWrite => format!("{} writes to the workspace", operation.label),
        ApprovalScope::ProcessExec => format!("{} executes a process", operation.label),
        ApprovalScope::NetworkAccess => {
            format!("{} requires network access", operation.label)
        }
    }
}

fn deny_reason(operation: &Operation) -> String {
    match operation.scope {
        ApprovalScope::FileWrite => {
            format!("{} is denied by the active runtime policy", operation.label)
        }
        ApprovalScope::ProcessExec => {
            format!("{} is denied by the active runtime policy", operation.label)
        }
        ApprovalScope::NetworkAccess => {
            format!(
                "{} requires network access but the active runtime policy denies it",
                operation.label
            )
        }
    }
}
