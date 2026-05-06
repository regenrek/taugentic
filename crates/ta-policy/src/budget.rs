#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetPolicy {
    per_run: BudgetLimits,
    parent_aggregate: BudgetLimits,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetLimits {
    pub max_tokens: Option<u64>,
    pub max_wall_clock_ms: Option<u64>,
    pub max_tool_calls: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetUsage {
    pub total_tokens: u64,
    pub wall_clock_ms: u64,
    pub tool_calls: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetScope {
    Run,
    ParentAggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetMetric {
    Tokens,
    WallClockMs,
    ToolCalls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetExceeded {
    pub scope: BudgetScope,
    pub metric: BudgetMetric,
    pub limit: u64,
    pub actual: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDecision {
    WithinBudget,
    Exceeded(BudgetExceeded),
}

impl BudgetPolicy {
    pub const fn new(per_run: BudgetLimits, parent_aggregate: BudgetLimits) -> Self {
        Self {
            per_run,
            parent_aggregate,
        }
    }

    pub const fn unbounded() -> Self {
        Self::new(BudgetLimits::unbounded(), BudgetLimits::unbounded())
    }

    pub const fn per_run(self) -> BudgetLimits {
        self.per_run
    }

    pub const fn parent_aggregate(self) -> BudgetLimits {
        self.parent_aggregate
    }

    pub fn decide_run(self, usage: BudgetUsage) -> BudgetDecision {
        decide(BudgetScope::Run, self.per_run, usage)
    }

    pub fn decide_parent_aggregate(self, usage: BudgetUsage) -> BudgetDecision {
        decide(BudgetScope::ParentAggregate, self.parent_aggregate, usage)
    }
}

impl BudgetLimits {
    pub const fn unbounded() -> Self {
        Self {
            max_tokens: None,
            max_wall_clock_ms: None,
            max_tool_calls: None,
        }
    }
}

impl BudgetExceeded {
    pub fn redacted_reason(self) -> &'static str {
        match self.metric {
            BudgetMetric::Tokens => "token budget exceeded",
            BudgetMetric::WallClockMs => "wall-clock budget exceeded",
            BudgetMetric::ToolCalls => "tool-call budget exceeded",
        }
    }
}

fn decide(scope: BudgetScope, limits: BudgetLimits, usage: BudgetUsage) -> BudgetDecision {
    if let Some(limit) = limits.max_tokens
        && usage.total_tokens > limit
    {
        return BudgetDecision::Exceeded(BudgetExceeded {
            scope,
            metric: BudgetMetric::Tokens,
            limit,
            actual: usage.total_tokens,
        });
    }

    if let Some(limit) = limits.max_wall_clock_ms
        && usage.wall_clock_ms > limit
    {
        return BudgetDecision::Exceeded(BudgetExceeded {
            scope,
            metric: BudgetMetric::WallClockMs,
            limit,
            actual: usage.wall_clock_ms,
        });
    }

    if let Some(limit) = limits.max_tool_calls
        && usage.tool_calls > limit
    {
        return BudgetDecision::Exceeded(BudgetExceeded {
            scope,
            metric: BudgetMetric::ToolCalls,
            limit,
            actual: usage.tool_calls,
        });
    }

    BudgetDecision::WithinBudget
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_budget_exceeds_when_usage_is_above_limit() {
        let policy = BudgetPolicy::new(
            BudgetLimits {
                max_tokens: Some(10),
                ..BudgetLimits::unbounded()
            },
            BudgetLimits::unbounded(),
        );

        assert!(matches!(
            policy.decide_run(BudgetUsage {
                total_tokens: 11,
                ..BudgetUsage::default()
            }),
            BudgetDecision::Exceeded(BudgetExceeded {
                scope: BudgetScope::Run,
                metric: BudgetMetric::Tokens,
                limit: 10,
                actual: 11,
            })
        ));
    }

    #[test]
    fn unbounded_policy_allows_usage() {
        assert_eq!(
            BudgetPolicy::unbounded().decide_run(BudgetUsage {
                total_tokens: u64::MAX,
                wall_clock_ms: u64::MAX,
                tool_calls: u64::MAX,
            }),
            BudgetDecision::WithinBudget
        );
    }
}
