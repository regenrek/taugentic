use crate::SessionId;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha2::{Digest, Sha256};
use ta_protocol::wire::{
    AgentRuntimeStrategyId, AgentStreamItemId, AgentStreamTurnId, ArtifactId, ArtifactKind,
    ArtifactMetadata, ImageArtifactMetadata, ImageArtifactProvenance, ImageMediaType,
    RuntimeProfileId, WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES,
    WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES,
};
use ta_store::{ArtifactRecord, ArtifactRepository, StoreSeedRepository};

use super::test_support::{
    app_and_execution_with_runtime, ensure_running_run, ensure_running_run_with_profile,
    runtime_with_dispatch_plans,
};

fn image_payload(magic: &[u8]) -> String {
    BASE64.encode(magic)
}

fn record_image(
    execution: &super::RunExecutionService<ta_store::InMemoryStore>,
    session_id: &SessionId,
    run: &super::RunSummary,
    sequence: usize,
    payload: &str,
) -> Result<super::ArtifactMutationResult, super::RunExecutionError> {
    let generation = execution
        .runtime
        .live_execution_for(&run.id)
        .expect("seeded run must retain a live execution")
        .generation;
    execution.record_generated_image_for_leased_run(
        session_id.clone(),
        run.id.clone(),
        generation,
        AgentStreamTurnId::new(format!("turn-image-{sequence}")).expect("turn id"),
        AgentStreamItemId::new(format!("item-image-{sequence}")).expect("item id"),
        payload,
    )
}

fn image_metadata(byte_len: u64) -> ArtifactMetadata {
    ArtifactMetadata::Image(ImageArtifactMetadata {
        media_type: ImageMediaType::Png,
        sha256: "sha256:fixture".to_string(),
        byte_len,
        provenance: ImageArtifactProvenance {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
            turn_id: AgentStreamTurnId::new("turn-output-cap").expect("turn id"),
            item_id: AgentStreamItemId::new("item-output-cap").expect("item id"),
        },
    })
}

fn artifact_count(
    execution: &super::RunExecutionService<ta_store::InMemoryStore>,
    run: &super::RunSummary,
) -> usize {
    execution
        .store
        .lock()
        .expect("test store should not be poisoned")
        .artifacts_for_run(&run.id)
        .expect("artifact query")
        .len()
}

#[test]
fn generated_images_publish_only_supported_magic_with_typed_provenance_and_hash() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Generated image outputs");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate images",
        "runtime-codex-safe",
    );
    let projection = execution
        .load_run_projection(&run.id)
        .expect("seeded run projection");
    let fixtures = [
        (ImageMediaType::Png, b"\x89PNG\r\n\x1a\n".as_slice()),
        (ImageMediaType::Jpeg, b"\xff\xd8\xff\xe0".as_slice()),
        (ImageMediaType::Gif, b"GIF89a".as_slice()),
        (ImageMediaType::Webp, b"RIFF\x04\0\0\0WEBP".as_slice()),
    ];

    for (sequence, (media_type, bytes)) in fixtures.iter().enumerate() {
        let artifact = record_image(
            &execution,
            &session.id,
            &run,
            sequence,
            &image_payload(bytes),
        )
        .expect("supported completed image must publish atomically")
        .artifact;
        assert_eq!(artifact.kind, ArtifactKind::Image);
        assert!(matches!(
            artifact.metadata,
            ArtifactMetadata::Image(ref metadata)
                if metadata.media_type == *media_type
                    && metadata.byte_len == bytes.len() as u64
                    && metadata.sha256 == format!("sha256:{:x}", Sha256::digest(bytes))
                    && metadata.provenance.runtime_profile_id == run.runtime_profile_id
                    && metadata.provenance.provider_id == projection.source.route().provider_id
                    && metadata.provenance.turn_id
                        == AgentStreamTurnId::new(format!("turn-image-{sequence}")).expect("turn id")
                    && metadata.provenance.item_id
                        == AgentStreamItemId::new(format!("item-image-{sequence}")).expect("item id")
        ));
    }

    let _ = std::fs::remove_dir_all(execution.artifact_root().join(run.id.as_str()));
}

#[test]
fn unsupported_runtime_image_output_rejects_before_file_or_commit() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Unsupported image output");
    let run = ensure_running_run(&app, &execution, &session.id, "Generate image");
    let directory = execution.artifact_root().join(run.id.as_str());

    let error = record_image(
        &execution,
        &session.id,
        &run,
        0,
        &image_payload(b"\x89PNG\r\n\x1a\n"),
    )
    .expect_err("unsupported runtime must reject image output before publication");

    assert!(matches!(
        error,
        super::RunExecutionError::ProviderExecutionFailed(_)
    ));
    assert!(
        !directory.exists(),
        "unsupported output must not create an artifact directory"
    );
    assert_eq!(
        artifact_count(&execution, &run),
        0,
        "unsupported output must not commit an artifact"
    );
}

#[test]
fn generated_images_reject_malformed_empty_and_unsupported_payloads_before_publication() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Rejected image outputs");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate images",
        "runtime-codex-safe",
    );
    let unsupported = image_payload(b"not-an-image");

    for (sequence, payload) in [(0, "%%%"), (1, ""), (2, unsupported.as_str())] {
        let error = record_image(&execution, &session.id, &run, sequence, payload)
            .expect_err("invalid completed image must not create an artifact");
        assert!(matches!(
            error,
            super::RunExecutionError::ProviderExecutionFailed(_)
        ));
        assert!(
            !execution.artifact_root().join(run.id.as_str()).exists(),
            "rejected output must not create an artifact directory"
        );
        assert_eq!(
            artifact_count(&execution, &run),
            0,
            "rejected output must not commit an artifact"
        );
    }
}

#[test]
fn generated_image_count_cap_rejects_before_an_eleventh_file_or_commit() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Image output cap");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate images",
        "runtime-codex-safe",
    );
    let payload = image_payload(b"\x89PNG\r\n\x1a\n");

    for sequence in 0..ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT as usize {
        record_image(&execution, &session.id, &run, sequence, &payload)
            .expect("output at the configured image count limit must publish");
    }
    let directory = execution.artifact_root().join(run.id.as_str());
    let files_before = std::fs::read_dir(&directory)
        .expect("published output directory")
        .count();
    let artifacts_before = artifact_count(&execution, &run);
    let error = record_image(
        &execution,
        &session.id,
        &run,
        ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT as usize,
        &payload,
    )
    .expect_err("eleventh image must reject before write and commit");
    assert!(matches!(
        error,
        super::RunExecutionError::ProviderExecutionFailed(_)
    ));
    assert_eq!(
        std::fs::read_dir(&directory)
            .expect("published output directory")
            .count(),
        files_before,
        "rejected output must not add a filesystem artifact"
    );
    assert_eq!(
        artifact_count(&execution, &run),
        artifacts_before,
        "rejected output must not commit an additional artifact"
    );

    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn generated_image_aggregate_cap_rejects_before_file_or_commit() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Image aggregate output cap");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate image",
        "runtime-codex-safe",
    );
    {
        let mut store = execution
            .store
            .lock()
            .expect("test store should not be poisoned");
        store
            .save_artifact(ArtifactRecord {
                id: ArtifactId::new("artifact-existing-image").expect("artifact id"),
                session_id: session.id.clone(),
                run_id: run.id.clone(),
                kind: ArtifactKind::Image,
                metadata: image_metadata(WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES),
                storage_path: "already-published.png".to_string(),
            })
            .expect("existing artifact should seed");
    }

    let directory = execution.artifact_root().join(run.id.as_str());
    let error = record_image(
        &execution,
        &session.id,
        &run,
        1,
        &image_payload(b"\x89PNG\r\n\x1a\n"),
    )
    .expect_err("aggregate overflow must reject before file or commit");
    assert!(matches!(
        error,
        super::RunExecutionError::ProviderExecutionFailed(_)
    ));
    assert!(
        !directory.exists(),
        "aggregate rejection must not create an output file"
    );
    assert_eq!(
        artifact_count(&execution, &run),
        1,
        "aggregate rejection must not commit an additional artifact"
    );
}

#[test]
fn generated_image_per_item_cap_rejects_before_file_or_commit() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Image per-item output cap");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate image",
        "runtime-codex-safe",
    );
    let mut bytes = vec![0; WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES as usize + 1];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    let directory = execution.artifact_root().join(run.id.as_str());

    let error = record_image(&execution, &session.id, &run, 0, &image_payload(&bytes))
        .expect_err("oversized image must reject before write and commit");

    assert!(matches!(
        error,
        super::RunExecutionError::ProviderExecutionFailed(_)
    ));
    assert!(
        !directory.exists(),
        "per-item cap rejection must not create an output directory"
    );
    assert_eq!(
        artifact_count(&execution, &run),
        0,
        "per-item cap rejection must not commit an artifact"
    );
}

#[test]
fn failed_leased_image_publication_removes_file_and_leaves_no_artifact() {
    let (runtime, _) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = super::test_support::open_session(&app, "Failed leased image publication");
    let run = ensure_running_run_with_profile(
        &app,
        &execution,
        &session.id,
        "Generate image",
        "runtime-codex-safe",
    );
    let generation = execution
        .runtime
        .live_execution_for(&run.id)
        .expect("live execution")
        .generation;
    let directory = execution.artifact_root().join(run.id.as_str());

    let error = execution
        .record_generated_image_for_leased_run(
            session.id.clone(),
            run.id.clone(),
            generation + 1,
            AgentStreamTurnId::new("turn-stale-lease").expect("turn id"),
            AgentStreamItemId::new("item-stale-lease").expect("item id"),
            &image_payload(b"\x89PNG\r\n\x1a\n"),
        )
        .expect_err("stale lease must reject the publication");
    assert!(matches!(
        error,
        super::RunExecutionError::ProviderExecutionFailed(_)
    ));
    assert_eq!(
        std::fs::read_dir(&directory)
            .expect("image directory should exist after attempted write")
            .count(),
        0,
        "failed lease publication must remove the attempted image file"
    );
    assert_eq!(
        artifact_count(&execution, &run),
        0,
        "failed lease publication must leave no durable artifact"
    );
    let _ = std::fs::remove_dir_all(directory);
}
