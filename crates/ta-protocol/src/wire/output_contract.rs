use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::wire::ReceiptId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum OutputContractKind {
    Debug,
    Patch,
    Review,
    Test,
    Plan,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum CapsuleResult {
    Debug(DebugResult),
    Patch(PatchResult),
    Review(ReviewResult),
    Test(TestResult),
    Plan(PlanResult),
    Custom(#[ts(type = "unknown")] serde_json::Value),
}

impl CapsuleResult {
    pub fn contract_kind(&self) -> OutputContractKind {
        match self {
            Self::Debug(_) => OutputContractKind::Debug,
            Self::Patch(_) => OutputContractKind::Patch,
            Self::Review(_) => OutputContractKind::Review,
            Self::Test(_) => OutputContractKind::Test,
            Self::Plan(_) => OutputContractKind::Plan,
            Self::Custom(_) => OutputContractKind::Custom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DebugResult {
    pub reproduced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<String>,
    pub evidence_receipt_ids: Vec<ReceiptId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_receipt_id: Option<ReceiptId>,
    #[schemars(schema_with = "confidence_score_json_schema")]
    pub confidence: f32,
    pub blockers: Vec<String>,
}

impl Eq for DebugResult {}

fn confidence_score_json_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "number",
        "minimum": 0,
        "maximum": 1
    })
}

fn json_number_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "number"
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct PatchResult {
    pub patch_receipt_ids: Vec<ReceiptId>,
    pub touched_files: Vec<String>,
    pub tests_run_receipt_ids: Vec<ReceiptId>,
    pub passing: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ReviewResult {
    pub verdict: ReviewVerdict,
    pub findings: Vec<ReviewFinding>,
    pub risks: Vec<String>,
    pub touched_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    NeedsHuman,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ReviewFinding {
    pub severity: FindingSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct TestResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub failed_test_names: Vec<String>,
    pub log_receipt_ids: Vec<ReceiptId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct PlanResult {
    pub steps: Vec<PlanStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_total_minutes: Option<u32>,
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct PlanStep {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_minutes: Option<u32>,
    pub depends_on: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS, Error)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum ValidationError {
    #[error("output contract kind mismatch: expected {expected:?}, got {got:?}")]
    KindMismatch {
        expected: OutputContractKind,
        got: OutputContractKind,
    },
    #[error("debug confidence is out of range: {value}")]
    ConfidenceOutOfRange {
        #[schemars(schema_with = "json_number_schema")]
        value: f32,
    },
    #[error("invalid receipt id: {0}")]
    InvalidReceiptId(ReceiptId),
    #[error("test counts are inconsistent: total {total}, sum of parts {sum_of_parts}")]
    TestCountsInconsistent { total: u32, sum_of_parts: u32 },
    #[error("test counts overflow u32: passed {passed}, failed {failed}, skipped {skipped}")]
    TestCountsOverflow {
        passed: u32,
        failed: u32,
        skipped: u32,
    },
    #[error(
        "plan step dependency is out of range: step {step_index}, dependency {dependency}, total steps {total_steps}"
    )]
    PlanStepDependencyOutOfRange {
        step_index: u32,
        dependency: u32,
        total_steps: u32,
    },
    #[error("{0}")]
    Custom(String),
}

impl Eq for ValidationError {}

pub fn validate_result_against_contract(
    contract: OutputContractKind,
    result: &CapsuleResult,
) -> Result<(), ValidationError> {
    let got = result.contract_kind();
    if contract != got {
        return Err(ValidationError::KindMismatch {
            expected: contract,
            got,
        });
    }

    match result {
        CapsuleResult::Debug(result) => validate_debug_result(result),
        CapsuleResult::Patch(result) => validate_patch_result(result),
        CapsuleResult::Review(_) | CapsuleResult::Custom(_) => Ok(()),
        CapsuleResult::Test(result) => validate_test_result(result),
        CapsuleResult::Plan(result) => validate_plan_result(result),
    }
}

fn validate_debug_result(result: &DebugResult) -> Result<(), ValidationError> {
    if !result.confidence.is_finite() || !(0.0..=1.0).contains(&result.confidence) {
        return Err(ValidationError::ConfidenceOutOfRange {
            value: result.confidence,
        });
    }

    if result.reproduced && result.evidence_receipt_ids.is_empty() {
        return Err(ValidationError::Custom(
            "debug result must include evidence receipts when reproduced is true".to_string(),
        ));
    }

    Ok(())
}

fn validate_patch_result(result: &PatchResult) -> Result<(), ValidationError> {
    if result.patch_receipt_ids.is_empty() {
        return Err(ValidationError::Custom(
            "patch result must include at least one patch receipt".to_string(),
        ));
    }

    Ok(())
}

fn validate_test_result(result: &TestResult) -> Result<(), ValidationError> {
    let sum_of_parts =
        u64::from(result.passed) + u64::from(result.failed) + u64::from(result.skipped);

    if sum_of_parts > u64::from(u32::MAX) {
        return Err(ValidationError::TestCountsOverflow {
            passed: result.passed,
            failed: result.failed,
            skipped: result.skipped,
        });
    }

    if sum_of_parts != u64::from(result.total) {
        return Err(ValidationError::TestCountsInconsistent {
            total: result.total,
            sum_of_parts: sum_of_parts as u32,
        });
    }

    Ok(())
}

fn validate_plan_result(result: &PlanResult) -> Result<(), ValidationError> {
    let total_steps = result.steps.len();
    for (step_index, step) in result.steps.iter().enumerate() {
        for dependency in &step.depends_on {
            if *dependency as usize >= total_steps {
                return Err(ValidationError::PlanStepDependencyOutOfRange {
                    step_index: bounded_usize_to_u32(step_index),
                    dependency: *dependency,
                    total_steps: bounded_usize_to_u32(total_steps),
                });
            }

            if *dependency as usize == step_index {
                return Err(ValidationError::Custom(format!(
                    "plan step {step_index} cannot depend on itself"
                )));
            }
        }
    }

    Ok(())
}

fn bounded_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
