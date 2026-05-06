use ta_protocol::wire::{
    CapsuleResult, DebugResult, FindingSeverity, OutputContractKind, PatchResult, PlanResult,
    PlanStep, ReviewFinding, ReviewResult, ReviewVerdict, TestResult, ValidationError,
    validate_result_against_contract,
};

#[test]
fn debug_result_validation_accepts_boundaries_and_reproduction_evidence() {
    let mut result = debug_result();
    result.confidence = 0.0;
    validate_result_against_contract(OutputContractKind::Debug, &CapsuleResult::Debug(result))
        .expect("confidence lower boundary should validate");

    let mut result = debug_result();
    result.confidence = 1.0;
    validate_result_against_contract(OutputContractKind::Debug, &CapsuleResult::Debug(result))
        .expect("confidence upper boundary should validate");
}

#[test]
fn debug_result_validation_rejects_out_of_range_confidence() {
    for confidence in [-0.1, 1.1] {
        let mut result = debug_result();
        result.confidence = confidence;

        let error = validate_result_against_contract(
            OutputContractKind::Debug,
            &CapsuleResult::Debug(result),
        )
        .expect_err("out-of-range confidence should fail");

        assert_eq!(
            error,
            ValidationError::ConfidenceOutOfRange { value: confidence }
        );
    }
}

#[test]
fn debug_result_validation_requires_evidence_when_reproduced() {
    let mut result = debug_result();
    result.evidence_receipt_ids.clear();

    let error =
        validate_result_against_contract(OutputContractKind::Debug, &CapsuleResult::Debug(result))
            .expect_err("reproduced debug result needs evidence receipts");

    assert!(matches!(error, ValidationError::Custom(message) if message.contains("evidence")));
}

#[test]
fn patch_result_validation_requires_patch_receipts() {
    validate_result_against_contract(
        OutputContractKind::Patch,
        &CapsuleResult::Patch(patch_result()),
    )
    .expect("patch with receipt should validate");

    let mut result = patch_result();
    result.patch_receipt_ids.clear();
    let error =
        validate_result_against_contract(OutputContractKind::Patch, &CapsuleResult::Patch(result))
            .expect_err("patch without receipt should fail");

    assert!(matches!(error, ValidationError::Custom(message) if message.contains("patch receipt")));
}

#[test]
fn review_result_validation_accepts_structured_review_payload() {
    validate_result_against_contract(
        OutputContractKind::Review,
        &CapsuleResult::Review(review_result()),
    )
    .expect("review result should validate");
}

#[test]
fn test_result_validation_requires_consistent_counts() {
    validate_result_against_contract(
        OutputContractKind::Test,
        &CapsuleResult::Test(test_result()),
    )
    .expect("consistent test counts should validate");

    let mut result = test_result();
    result.total = 4;
    let error =
        validate_result_against_contract(OutputContractKind::Test, &CapsuleResult::Test(result))
            .expect_err("inconsistent test counts should fail");

    assert_eq!(
        error,
        ValidationError::TestCountsInconsistent {
            total: 4,
            sum_of_parts: 3,
        }
    );
}

#[test]
fn test_counts_at_u32_max_boundary_validates() {
    let mut result = test_result();
    result.total = u32::MAX;
    result.passed = u32::MAX;
    result.failed = 0;
    result.skipped = 0;

    let outcome =
        validate_result_against_contract(OutputContractKind::Test, &CapsuleResult::Test(result));

    assert!(outcome.is_ok(), "u32::MAX boundary should validate cleanly");
}

#[test]
fn test_counts_overflow_detected_when_sum_exceeds_u32_max() {
    let mut result = test_result();
    result.total = u32::MAX;
    result.passed = u32::MAX;
    result.failed = 1;
    result.skipped = 0;

    let error =
        validate_result_against_contract(OutputContractKind::Test, &CapsuleResult::Test(result))
            .expect_err("overflowing test counts should fail");

    assert_eq!(
        error,
        ValidationError::TestCountsOverflow {
            passed: u32::MAX,
            failed: 1,
            skipped: 0,
        }
    );
}

#[test]
fn plan_result_validation_checks_dependencies() {
    validate_result_against_contract(
        OutputContractKind::Plan,
        &CapsuleResult::Plan(plan_result()),
    )
    .expect("valid dependency index should validate");

    let mut out_of_range = plan_result();
    out_of_range.steps[1].depends_on = vec![2];
    let error = validate_result_against_contract(
        OutputContractKind::Plan,
        &CapsuleResult::Plan(out_of_range),
    )
    .expect_err("out-of-range dependency should fail");

    assert_eq!(
        error,
        ValidationError::PlanStepDependencyOutOfRange {
            step_index: 1,
            dependency: 2,
            total_steps: 2,
        }
    );

    let mut self_dep = plan_result();
    self_dep.steps[1].depends_on = vec![1];
    let error =
        validate_result_against_contract(OutputContractKind::Plan, &CapsuleResult::Plan(self_dep))
            .expect_err("self dependency should fail");

    assert!(
        matches!(error, ValidationError::Custom(message) if message.contains("cannot depend on itself"))
    );
}

#[test]
fn validation_error_json_uses_camel_case_struct_variant_fields() {
    let counts = serde_json::to_value(ValidationError::TestCountsInconsistent {
        total: 5,
        sum_of_parts: 7,
    })
    .expect("validation error should serialize");
    assert_eq!(
        counts,
        serde_json::json!({
            "kind": "testCountsInconsistent",
            "value": {
                "total": 5,
                "sumOfParts": 7
            }
        })
    );
    assert!(counts["value"].get("sum_of_parts").is_none(), "{counts}");

    let dependency = serde_json::to_value(ValidationError::PlanStepDependencyOutOfRange {
        step_index: 1,
        dependency: 3,
        total_steps: 2,
    })
    .expect("validation error should serialize");
    assert_eq!(
        dependency,
        serde_json::json!({
            "kind": "planStepDependencyOutOfRange",
            "value": {
                "stepIndex": 1,
                "dependency": 3,
                "totalSteps": 2
            }
        })
    );
    assert!(
        dependency["value"].get("step_index").is_none(),
        "{dependency}"
    );
    assert!(
        dependency["value"].get("total_steps").is_none(),
        "{dependency}"
    );
}

#[test]
fn custom_result_validation_only_requires_custom_contract() {
    let result = CapsuleResult::Custom(serde_json::json!({
        "externalKind": "nonStandard",
        "payload": { "accepted": true }
    }));

    validate_result_against_contract(OutputContractKind::Custom, &result)
        .expect("custom result should validate for custom contract");

    let error = validate_result_against_contract(OutputContractKind::Patch, &result)
        .expect_err("custom result should not satisfy patch contract");

    assert_eq!(
        error,
        ValidationError::KindMismatch {
            expected: OutputContractKind::Patch,
            got: OutputContractKind::Custom,
        }
    );
}

#[test]
fn validation_rejects_kind_mismatch() {
    let error = validate_result_against_contract(
        OutputContractKind::Review,
        &CapsuleResult::Debug(debug_result()),
    )
    .expect_err("debug result should not satisfy review contract");

    assert_eq!(
        error,
        ValidationError::KindMismatch {
            expected: OutputContractKind::Review,
            got: OutputContractKind::Debug,
        }
    );
}

#[test]
fn capsule_result_variants_roundtrip_through_json() {
    for result in [
        CapsuleResult::Debug(debug_result()),
        CapsuleResult::Patch(patch_result()),
        CapsuleResult::Review(review_result()),
        CapsuleResult::Test(test_result()),
        CapsuleResult::Plan(plan_result()),
        CapsuleResult::Custom(serde_json::json!({
            "externalKind": "custom",
            "value": ["free", "form"]
        })),
    ] {
        let json = serde_json::to_value(&result).expect("capsule result should serialize");
        let decoded: CapsuleResult =
            serde_json::from_value(json.clone()).expect("capsule result should deserialize");

        assert_eq!(decoded, result);
        assert!(json.get("kind").is_some(), "{json}");
        assert!(json.get("value").is_some(), "{json}");
    }
}

#[test]
fn capsule_result_json_uses_camel_case_contract_shape() {
    let json = serde_json::to_value(CapsuleResult::Debug(debug_result()))
        .expect("result should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "kind": "debug",
            "value": {
                "reproduced": true,
                "rootCause": "input parser drops the last byte",
                "evidenceReceiptIds": ["receipt-evidence"],
                "patchReceiptId": "receipt-patch",
                "confidence": 0.75,
                "blockers": []
            }
        })
    );
}

fn debug_result() -> DebugResult {
    DebugResult {
        reproduced: true,
        root_cause: Some("input parser drops the last byte".to_string()),
        evidence_receipt_ids: vec!["receipt-evidence".to_string()],
        patch_receipt_id: Some("receipt-patch".to_string()),
        confidence: 0.75,
        blockers: vec![],
    }
}

fn patch_result() -> PatchResult {
    PatchResult {
        patch_receipt_ids: vec!["receipt-patch".to_string()],
        touched_files: vec!["crates/ta-protocol/src/wire/output_contract.rs".to_string()],
        tests_run_receipt_ids: vec!["receipt-test".to_string()],
        passing: true,
        blockers: vec![],
    }
}

fn review_result() -> ReviewResult {
    ReviewResult {
        verdict: ReviewVerdict::RequestChanges,
        findings: vec![ReviewFinding {
            severity: FindingSeverity::High,
            file: Some("crates/ta-protocol/src/wire/output_contract.rs".to_string()),
            line: Some(42),
            message: "validation must reject mismatched contract kinds".to_string(),
            suggestion: Some("compare the declared contract with the result variant".to_string()),
        }],
        risks: vec!["review consumer must handle needsHuman".to_string()],
        touched_files: vec!["crates/ta-protocol/src/wire/output_contract.rs".to_string()],
    }
}

fn test_result() -> TestResult {
    TestResult {
        total: 3,
        passed: 2,
        failed: 1,
        skipped: 0,
        failed_test_names: vec!["output_contract_rejects_bad_counts".to_string()],
        log_receipt_ids: vec!["receipt-test-log".to_string()],
    }
}

fn plan_result() -> PlanResult {
    PlanResult {
        steps: vec![
            PlanStep {
                title: "Define protocol shape".to_string(),
                description: Some("Add the canonical wire result payloads".to_string()),
                estimated_minutes: Some(30),
                depends_on: vec![],
            },
            PlanStep {
                title: "Validate protocol shape".to_string(),
                description: None,
                estimated_minutes: Some(20),
                depends_on: vec![0],
            },
        ],
        estimated_total_minutes: Some(50),
        risks: vec!["completion wiring is intentionally out of scope".to_string()],
    }
}
