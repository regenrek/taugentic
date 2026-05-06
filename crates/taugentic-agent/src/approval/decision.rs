use ta_protocol::wire::ApprovalDecision;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDescriptor {
    pub call_id: String,
    pub tool_name: String,
    pub reason: String,
}

impl ApprovalDescriptor {
    pub fn new(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOutcome {
    Allow,
    Deny,
    TurnInterrupted,
}

impl ApprovalOutcome {
    pub fn decision(self) -> ApprovalDecision {
        match self {
            Self::Allow => ApprovalDecision::Approved,
            Self::Deny | Self::TurnInterrupted => ApprovalDecision::Rejected,
        }
    }

    pub fn rejection_reason(self) -> Option<&'static str> {
        match self {
            Self::Allow => None,
            Self::Deny => Some("approval_denied"),
            Self::TurnInterrupted => Some("turn_interrupted"),
        }
    }

    pub fn commentary(self) -> &'static str {
        match self {
            Self::Allow => "approval_allowed",
            Self::Deny => "approval_denied",
            Self::TurnInterrupted => "turn_interrupted",
        }
    }
}
