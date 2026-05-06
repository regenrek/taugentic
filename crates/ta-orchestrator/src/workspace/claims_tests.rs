use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use super::{CapsuleId, ClaimError, ClaimHandle, ClaimKind, ClaimRegistry, ConflictWarning};

fn capsule_id(value: &str) -> CapsuleId {
    match CapsuleId::new(value.to_string()) {
        Ok(id) => id,
        Err(error) => panic!("valid capsule id rejected: {error}"),
    }
}

fn claim_write(
    registry: &ClaimRegistry,
    capsule_id: CapsuleId,
    files: Vec<&str>,
    ttl: Option<Duration>,
) -> (ClaimHandle, Option<ConflictWarning>) {
    let paths = files.into_iter().map(PathBuf::from).collect();
    match registry.claim(capsule_id, paths, ttl, ClaimKind::Write) {
        Ok(outcome) => outcome,
        Err(error) => panic!("claim failed: {error}"),
    }
}

fn check_write(
    registry: &ClaimRegistry,
    capsule_id: &CapsuleId,
    files: &[&str],
) -> Option<ConflictWarning> {
    let paths: Vec<_> = files.iter().map(PathBuf::from).collect();
    match registry.check_conflict(capsule_id, &paths, ClaimKind::Write) {
        Ok(warning) => warning,
        Err(error) => panic!("conflict check failed: {error}"),
    }
}

#[test]
fn claim_release_roundtrip_removes_active_claim() {
    let registry = ClaimRegistry::new();
    let capsule = capsule_id("run-a");
    let (handle, warning) = claim_write(&registry, capsule.clone(), vec!["src/a.rs"], None);

    assert!(warning.is_none());
    let active = registry.active_claims();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].capsule_id, capsule);
    assert_eq!(active[0].files, vec![PathBuf::from("src/a.rs")]);

    drop(handle);
    assert!(registry.active_claims().is_empty());
}

#[test]
fn overlapping_claim_returns_conflict_warning() {
    let registry = ClaimRegistry::new();
    let capsule_a = capsule_id("run-a");
    let capsule_b = capsule_id("run-b");
    let (_handle_a, warning_a) = claim_write(
        &registry,
        capsule_a.clone(),
        vec!["src/a.rs", "src/b.rs"],
        None,
    );

    assert!(warning_a.is_none());

    let (_handle_b, warning_b) = claim_write(
        &registry,
        capsule_b.clone(),
        vec!["src/b.rs", "src/c.rs"],
        None,
    );
    let Some(warning_b) = warning_b else {
        panic!("expected conflict warning");
    };

    assert_eq!(warning_b.requesting_capsule, capsule_b);
    assert_eq!(warning_b.conflicts.len(), 1);
    assert_eq!(warning_b.conflicts[0].file, PathBuf::from("src/b.rs"));
    assert_eq!(warning_b.conflicts[0].holding_capsule, capsule_a);
    assert_eq!(warning_b.conflicts[0].holding_kind, ClaimKind::Write);
}

#[test]
fn disjoint_claims_do_not_warn() {
    let registry = ClaimRegistry::new();
    let capsule_a = capsule_id("run-a");
    let capsule_b = capsule_id("run-b");
    let (_handle_a, warning_a) = claim_write(&registry, capsule_a, vec!["src/a.rs"], None);
    let (_handle_b, warning_b) = claim_write(&registry, capsule_b, vec!["src/b.rs"], None);

    assert!(warning_a.is_none());
    assert!(warning_b.is_none());
}

#[test]
fn same_capsule_reclaim_replaces_previous_files() {
    let registry = ClaimRegistry::new();
    let capsule_a = capsule_id("run-a");
    let capsule_b = capsule_id("run-b");
    let (first_handle, warning_a) =
        claim_write(&registry, capsule_a.clone(), vec!["src/a.rs"], None);
    let (second_handle, warning_b) = claim_write(&registry, capsule_a, vec!["src/b.rs"], None);

    assert!(warning_a.is_none());
    assert!(warning_b.is_none());
    assert!(check_write(&registry, &capsule_b, &["src/a.rs"]).is_none());

    let Some(warning) = check_write(&registry, &capsule_b, &["src/b.rs"]) else {
        panic!("expected conflict for replacement file");
    };
    assert_eq!(warning.conflicts.len(), 1);
    assert_eq!(warning.conflicts[0].file, PathBuf::from("src/b.rs"));

    drop(first_handle);
    assert_eq!(registry.active_claims().len(), 1);
    drop(second_handle);
    assert!(registry.active_claims().is_empty());
}

#[test]
fn expired_claims_are_swept_and_stop_conflicting() {
    let registry = ClaimRegistry::new();
    let capsule_a = capsule_id("run-a");
    let capsule_b = capsule_id("run-b");
    let (_handle, warning) = claim_write(
        &registry,
        capsule_a,
        vec!["src/a.rs"],
        Some(Duration::from_millis(100)),
    );

    assert!(warning.is_none());
    thread::sleep(Duration::from_millis(200));
    assert_eq!(registry.sweep_expired(), 1);
    assert!(registry.active_claims().is_empty());
    assert!(check_write(&registry, &capsule_b, &["src/a.rs"]).is_none());
}

#[test]
fn refresh_extends_current_claim_expiry() {
    let registry = ClaimRegistry::new();
    let capsule = capsule_id("run-a");
    let (handle, warning) = claim_write(
        &registry,
        capsule,
        vec!["src/a.rs"],
        Some(Duration::from_millis(100)),
    );

    assert!(warning.is_none());
    thread::sleep(Duration::from_millis(50));
    handle.refresh(Duration::from_secs(5));
    thread::sleep(Duration::from_millis(100));

    let active = registry.active_claims();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].files, vec![PathBuf::from("src/a.rs")]);
}

#[test]
fn concurrent_disjoint_claims_are_all_visible() {
    let registry = ClaimRegistry::new();
    let mut joins = Vec::new();

    for index in 0..5 {
        let registry = registry.clone();
        joins.push(thread::spawn(move || {
            let capsule = capsule_id(&format!("run-{index}"));
            let file = format!("src/{index}.rs");
            let (handle, warning) = claim_write(&registry, capsule, vec![file.as_str()], None);
            assert!(warning.is_none());
            handle
        }));
    }

    let mut handles = Vec::new();
    for join in joins {
        match join.join() {
            Ok(handle) => handles.push(handle),
            Err(error) => std::panic::resume_unwind(error),
        }
    }

    assert_eq!(registry.active_claims().len(), 5);
    drop(handles);
    assert!(registry.active_claims().is_empty());
}

#[test]
fn drop_after_registry_value_drops_never_panics() {
    let registry = ClaimRegistry::new();
    let capsule = capsule_id("run-a");
    let (handle, warning) = claim_write(&registry, capsule, vec!["src/a.rs"], None);

    assert!(warning.is_none());
    drop(registry);
    drop(handle);
}

#[test]
fn lexical_normalization_allows_nonexistent_relative_write_targets() {
    let registry = ClaimRegistry::new();
    let capsule = capsule_id("run-a");
    let (handle, warning) = claim_write(&registry, capsule, vec!["does/not/exist.rs"], None);

    assert!(warning.is_none());
    assert_eq!(handle.files(), &[PathBuf::from("does/not/exist.rs")]);

    let absolute_result = registry.claim(
        capsule_id("run-b"),
        vec![PathBuf::from("/tmp/not-claimable.rs")],
        None,
        ClaimKind::Write,
    );
    assert!(matches!(absolute_result, Err(ClaimError::InvalidPath(_))));
}

#[test]
fn equivalent_relative_paths_normalize_to_same_key() {
    let registry = ClaimRegistry::new();
    let capsule_a = capsule_id("run-a");
    let capsule_b = capsule_id("run-b");
    let (_handle_a, warning_a) =
        claim_write(&registry, capsule_a.clone(), vec!["./src/./foo.rs"], None);

    assert!(warning_a.is_none());

    let Some(warning_b) = check_write(&registry, &capsule_b, &["src/foo.rs"]) else {
        panic!("expected normalized path conflict");
    };
    assert_eq!(warning_b.conflicts.len(), 1);
    assert_eq!(warning_b.conflicts[0].file, PathBuf::from("src/foo.rs"));
    assert_eq!(warning_b.conflicts[0].holding_capsule, capsule_a);
}
