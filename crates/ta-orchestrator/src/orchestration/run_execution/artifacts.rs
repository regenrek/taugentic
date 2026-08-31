use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha2::{Digest, Sha256};
use ta_protocol::wire::{
    AgentRuntimeMediaCapability, AgentStreamItemId, AgentStreamTurnId, ArtifactKind,
    ArtifactMetadata, ContextReceiptEvent, DaemonEvent, ImageArtifactMetadata,
    ImageArtifactProvenance, ImageMediaType, ReceiptKind, ReceiptProvenance, RunHarnessKind,
    RunSource,
};
use ta_store::{
    ArtifactRecord, CommitArtifactPublish, CommitReceiptEvent, CommitRepository, CreateReceipt,
    EventRecord, ReceiptListQuery, ReceiptRepository, StoreError,
};

use super::*;

pub(super) fn image_media_type(bytes: &[u8]) -> Option<ImageMediaType> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageMediaType::Png)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(ImageMediaType::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageMediaType::Gif)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageMediaType::Webp)
    } else {
        None
    }
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn record_generated_image_for_leased_run(
        &self,
        session_id: crate::SessionId,
        run_id: RunId,
        generation: u64,
        turn_id: AgentStreamTurnId,
        item_id: AgentStreamItemId,
        raw_base64: &str,
    ) -> Result<ArtifactMutationResult, RunExecutionError> {
        let run = self.load_run_projection(&run_id)?;
        if run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                run_id.as_str().to_string(),
            ));
        }
        if self
            .agent_runtime
            .media_capabilities_for_route(run.source.route())
            .map_err(map_agent_runtime_error)?
            .image_output
            == AgentRuntimeMediaCapability::Unsupported
        {
            return Err(RunExecutionError::ProviderExecutionFailed(
                "the selected runtime does not support image output".to_string(),
            ));
        }
        let bytes = BASE64.decode(raw_base64).map_err(|_| {
            RunExecutionError::ProviderExecutionFailed(
                "completed imageGeneration result is not standard base64".to_string(),
            )
        })?;
        if bytes.is_empty()
            || bytes.len() as u64 > ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES
        {
            return Err(RunExecutionError::ProviderExecutionFailed(
                "completed imageGeneration result exceeds the image artifact limit".to_string(),
            ));
        }
        let media_type = image_media_type(&bytes).ok_or_else(|| {
            RunExecutionError::ProviderExecutionFailed(
                "completed imageGeneration result is not a supported image".to_string(),
            )
        })?;
        {
            let store = self.store.lock().expect("app store should not be poisoned");
            let existing_artifacts = store.artifacts_for_run(&run_id)?;
            let previous_image_count = existing_artifacts
                .iter()
                .filter(|artifact| artifact.kind == ArtifactKind::Image)
                .count();
            if previous_image_count >= ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT {
                return Err(RunExecutionError::ProviderExecutionFailed(
                    "completed imageGeneration result exceeds the image artifact count limit"
                        .to_string(),
                ));
            }
            let previous_image_bytes = existing_artifacts
                .into_iter()
                .filter_map(|artifact| match artifact.metadata {
                    ArtifactMetadata::Image(metadata) => Some(metadata.byte_len),
                    ArtifactMetadata::Standard => None,
                })
                .sum::<u64>();
            if previous_image_bytes.saturating_add(bytes.len() as u64)
                > ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES
            {
                return Err(RunExecutionError::ProviderExecutionFailed(
                    "completed imageGeneration results exceed the aggregate image artifact limit"
                        .to_string(),
                ));
            }
        }
        let directory = self.artifact_root().join(run_id.as_str());
        std::fs::create_dir_all(&directory).map_err(|error| {
            RunExecutionError::ProviderExecutionFailed(format!(
                "failed to create image artifact directory: {error}"
            ))
        })?;
        let path = directory.join(format!(
            "image-{}.{}",
            uuid::Uuid::new_v4().simple(),
            media_type.extension()
        ));
        std::fs::write(&path, &bytes).map_err(|error| {
            RunExecutionError::ProviderExecutionFailed(format!(
                "failed to write image artifact: {error}"
            ))
        })?;
        let metadata = ArtifactMetadata::Image(ImageArtifactMetadata {
            media_type,
            sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
            byte_len: bytes.len() as u64,
            provenance: ImageArtifactProvenance {
                runtime_profile_id: run.runtime_profile_id.clone(),
                provider_id: run.source.route().provider_id.clone(),
                turn_id,
                item_id,
            },
        });
        let artifact = ArtifactRecord {
            id: ta_protocol::wire::ArtifactId::new(format!(
                "artifact-{}",
                uuid::Uuid::new_v4().simple()
            ))
            .expect("generated artifact id should be valid"),
            session_id,
            run_id,
            kind: ArtifactKind::Image,
            metadata,
            storage_path: path.to_string_lossy().into_owned(),
        };
        let artifact_run_id = artifact.run_id.clone();
        let artifact_session_id = artifact.session_id.clone();
        let result = self.runtime.with_live_generation_lease(
            &artifact_run_id,
            &artifact_session_id,
            generation,
            || self.record_artifact_for_leased_run(artifact),
        );
        if result.is_err() {
            let _ = std::fs::remove_file(path);
        }
        result
    }
    pub fn record_artifact(
        &self,
        artifact: ArtifactRecord,
    ) -> Result<ArtifactMutationResult, RunExecutionError> {
        let storage_path = artifact.storage_path.trim();
        if storage_path.is_empty() {
            return Err(RunExecutionError::EmptyArtifactStoragePath);
        }

        let artifact = ArtifactRecord {
            storage_path: storage_path.to_string(),
            ..artifact
        };
        {
            let store = self.store.lock().expect("app store should not be poisoned");
            let Some(run) = store.run(&artifact.run_id)? else {
                return Err(RunExecutionError::RunNotFound(
                    artifact.run_id.as_str().to_string(),
                ));
            };
            if run.session_id != artifact.session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    run.id.as_str().to_string(),
                ));
            }
            if run.status != RunStatus::Running
                || !self
                    .runtime
                    .is_live_run_running(&run.id, &artifact.session_id)
            {
                return Err(RunExecutionError::RunNotLiveOwned(
                    run.id.as_str().to_string(),
                ));
            }
        }
        let (artifact, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let committed = match store.commit_artifact_publish(CommitArtifactPublish {
                artifact,
                occurred_at_ms: current_time_ms(),
            }) {
                Ok(committed) => committed,
                Err(StoreError::CommitRunStatusMismatch {
                    entity: "artifact", ..
                }) => {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        "artifact run is no longer actively running".to_string(),
                    ));
                }
                Err(error) => return Err(RunExecutionError::Store(error)),
            };
            let mut events = vec![committed.event.clone()];
            let run = store.run(&committed.artifact.run_id)?.ok_or_else(|| {
                RunExecutionError::RunNotFound(committed.artifact.run_id.as_str().to_string())
            })?;
            if let Some(input) = artifact_receipt_create_input(&run, &committed.artifact)
                && let Err(error) =
                    append_artifact_receipt_event(&mut *store, &committed.artifact, input)
                        .map(|event| events.extend(event))
            {
                tracing::warn!(
                    artifact_id = committed.artifact.id.as_str(),
                    run_id = committed.artifact.run_id.as_str(),
                    error = %error,
                    "failed to auto-create artifact receipt after artifact commit"
                );
            }
            (committed.artifact, events)
        };
        let summary = ta_store::project_artifact_summary(&artifact);
        Ok(ArtifactMutationResult {
            artifact: summary,
            events,
        })
    }

    /// Provider callbacks enter only after ActiveExecutionOwner has retained
    /// their exact generation lease. This variant intentionally omits the
    /// public point-in-time ownership preflight; it performs the same durable
    /// artifact and receipt commit while the caller's owner lease is alive.
    pub(super) fn record_artifact_for_leased_run(
        &self,
        artifact: ArtifactRecord,
    ) -> Result<ArtifactMutationResult, RunExecutionError> {
        let storage_path = artifact.storage_path.trim();
        if storage_path.is_empty() {
            return Err(RunExecutionError::EmptyArtifactStoragePath);
        }
        let artifact = ArtifactRecord {
            storage_path: storage_path.to_string(),
            ..artifact
        };
        let (artifact, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(run) = store.run(&artifact.run_id)? else {
                return Err(RunExecutionError::RunNotFound(
                    artifact.run_id.as_str().to_string(),
                ));
            };
            if run.session_id != artifact.session_id || run.status != RunStatus::Running {
                return Err(RunExecutionError::RunNotLiveOwned(
                    artifact.run_id.as_str().to_string(),
                ));
            }
            let committed = match store.commit_artifact_publish(CommitArtifactPublish {
                artifact,
                occurred_at_ms: current_time_ms(),
            }) {
                Ok(committed) => committed,
                Err(StoreError::CommitRunStatusMismatch {
                    entity: "artifact", ..
                }) => {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        "artifact run is no longer actively running".to_string(),
                    ));
                }
                Err(error) => return Err(RunExecutionError::Store(error)),
            };
            let mut events = vec![committed.event.clone()];
            let run = store.run(&committed.artifact.run_id)?.ok_or_else(|| {
                RunExecutionError::RunNotFound(committed.artifact.run_id.as_str().to_string())
            })?;
            if let Some(input) = artifact_receipt_create_input(&run, &committed.artifact)
                && let Err(error) =
                    append_artifact_receipt_event(&mut *store, &committed.artifact, input)
                        .map(|event| events.extend(event))
            {
                tracing::warn!(artifact_id = committed.artifact.id.as_str(), run_id = committed.artifact.run_id.as_str(), error = %error, "failed to auto-create artifact receipt after artifact commit");
            }
            (committed.artifact, events)
        };
        Ok(ArtifactMutationResult {
            artifact: ta_store::project_artifact_summary(&artifact),
            events,
        })
    }
}

fn artifact_receipt_create_input(
    run: &ta_store::RunProjection,
    artifact: &ArtifactRecord,
) -> Option<CreateReceipt> {
    // External harnesses can still write artifacts, but context receipts are a native-run membrane.
    if run.harness != RunHarnessKind::Native {
        return None;
    }
    let parent_run_id = match &run.source {
        RunSource::ScheduledWork { .. } | RunSource::User { .. } => None,
        RunSource::NativeSubagent { parent_run_id, .. }
        | RunSource::FreshSpawn { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. }
        | RunSource::RouteSwitchedContinuation { parent_run_id, .. } => Some(parent_run_id.clone()),
    };
    Some(CreateReceipt {
        session_id: artifact.session_id.clone(),
        run_id: artifact.run_id.clone(),
        parent_run_id,
        kind: receipt_kind_for_artifact(artifact.kind),
        provenance: ReceiptProvenance {
            artifact_id: Some(artifact.id.clone()),
            agent_turn_id: None,
            event_seq: None,
            stream_cursor: None,
        },
        title: Some(format!("Artifact {}", artifact.id.as_str())),
        summary: Some(format!(
            "{} artifact returned from native run",
            artifact_kind_label(artifact.kind)
        )),
    })
}

/// Best-effort receipt derivation for an already committed artifact.
///
/// The artifact event is the durable source of truth. Receipt rows and their
/// context events are derived metadata, so failures here must not turn a
/// successful artifact commit into a caller-visible failure or invite duplicate
/// artifact writes on retry. Receipt identity remains idempotent at the store
/// layer, so a later write can converge without producing duplicate receipts.
fn append_artifact_receipt_event<S>(
    store: &mut S,
    artifact: &ArtifactRecord,
    input: CreateReceipt,
) -> Result<Option<EventRecord>, StoreError>
where
    S: CommitRepository + ReceiptRepository,
{
    let exists = store
        .list(&ReceiptListQuery {
            session_id: input.session_id.clone(),
            run_id: Some(input.run_id.clone()),
            state: None,
            kind: Some(input.kind),
            parent_run_id: input.parent_run_id.clone(),
            limit: None,
        })?
        .into_iter()
        .any(|receipt| receipt.provenance.artifact_id.as_ref() == Some(&artifact.id));
    if exists {
        return Ok(None);
    }

    let receipt = store.create(input)?;
    store
        .commit_receipt_event(CommitReceiptEvent {
            session_id: artifact.session_id.clone(),
            event: DaemonEvent::ContextReceipt(ContextReceiptEvent::Created { receipt }),
            occurred_at_ms: current_time_ms(),
        })
        .map(|committed| Some(committed.event))
}

fn receipt_kind_for_artifact(kind: ArtifactKind) -> ReceiptKind {
    match kind {
        ArtifactKind::Patch => ReceiptKind::Patch,
        ArtifactKind::FileSnapshot => ReceiptKind::Evidence,
        ArtifactKind::Transcript | ArtifactKind::CommandLog | ArtifactKind::Image => {
            ReceiptKind::Artifact
        }
    }
}

fn artifact_kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Transcript => "transcript",
        ArtifactKind::Patch => "patch",
        ArtifactKind::FileSnapshot => "file snapshot",
        ArtifactKind::CommandLog => "command log",
        ArtifactKind::Image => "image",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{ArtifactId, ArtifactKind, DaemonEventKind, ListArtifactsQuery};
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    use ta_protocol::wire::{
        AgentStreamEvent, AgentStreamFrame, AgentToolCallOutcome, DaemonEvent,
    };
    use ta_store::StoreError;
    use taugentic_agent::{ExecutionSink, StreamEmission};

    #[test]
    fn record_artifact_rejects_missing_run_projection() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let artifact_id = ArtifactId::new("artifact-a").expect("artifact id");

        let error = execution
            .record_artifact(ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session.id.clone(),
                run_id: RunId::new("run-missing").expect("run id"),
                kind: ArtifactKind::Transcript,
                metadata: ArtifactMetadata::Standard,
                storage_path: "artifacts/run-missing/transcript.md".to_string(),
            })
            .expect_err("missing run must fail");

        let listed = app
            .list_artifacts(
                &session.id,
                &ListArtifactsQuery {
                    run_id: None,
                    artifact_id: Some(artifact_id),
                },
            )
            .expect("artifact list should work");
        let page = app
            .activity_page(
                &session.id,
                &crate::ActivityPageQuery {
                    limit: 10,
                    before: None,
                    kinds: vec![DaemonEventKind::Artifact],
                },
            )
            .expect("activity page");

        assert!(matches!(
            error,
            RunExecutionError::RunNotFound(ref run_id) if run_id == "run-missing"
        ));
        assert!(listed.items.is_empty());
        assert!(listed.latest_cursor.is_some());
        assert!(page.items.is_empty());
    }

    #[test]
    fn push_stream_publishes_transient_and_durable_frames_but_replays_only_durable() {
        let runtime = crate::RuntimeService::bootstrap();
        let subscription_runtime = runtime.clone();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let started = ensure_running_run(&app, &execution, &session.id, "Ship patch");
        let sink = provider_sink(&execution, &session.id, &started.id);
        let subscription = subscription_runtime.subscribe_events(
            &session.id,
            &[DaemonEventKind::AgentStream],
            None,
            None,
        );

        sink.push_stream(StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::ToolCallStarted {
                tool_name: "shell".to_string(),
                input: "{}".to_string(),
            },
        })
        .expect("durable agent stream event should publish");
        sink.push_stream(StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::AssistantTurnStarted,
        })
        .expect("assistant turn start should publish");
        sink.push_stream(StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: Some(1),
            frame: AgentStreamFrame::AssistantMessageDelta {
                delta: "partial".to_string(),
            },
        })
        .expect("transient agent stream event should publish");
        sink.push_stream(StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed,
            },
        })
        .expect("final durable agent stream event should publish");

        let live = (0..4)
            .map(|_| {
                subscription
                    .receiver
                    .recv_timeout(std::time::Duration::from_millis(200))
                    .expect("subscriber should receive live agent stream event")
            })
            .collect::<Vec<_>>();
        let replay = app
            .activity_page(
                &session.id,
                &crate::ActivityPageQuery {
                    limit: 10,
                    before: None,
                    kinds: vec![DaemonEventKind::AgentStream],
                },
            )
            .expect("activity page should load");

        assert_eq!(
            live.iter().map(|event| event.sequence).collect::<Vec<_>>(),
            vec![3, 4, 5, 6]
        );
        assert!(matches!(
            &live[0].event,
            DaemonEvent::AgentStream(AgentStreamEvent {
                emission: StreamEmission {
                    frame: AgentStreamFrame::ToolCallStarted { .. },
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            &live[1].event,
            DaemonEvent::AgentStream(AgentStreamEvent {
                emission: StreamEmission {
                    frame: AgentStreamFrame::AssistantTurnStarted,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            &live[2].event,
            DaemonEvent::AgentStream(AgentStreamEvent {
                emission: StreamEmission {
                    frame: AgentStreamFrame::AssistantMessageDelta { .. },
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            &live[3].event,
            DaemonEvent::AgentStream(AgentStreamEvent {
                emission: StreamEmission {
                    frame: AgentStreamFrame::ToolCallCompleted { .. },
                    ..
                },
                ..
            })
        ));
        assert_eq!(
            replay
                .items
                .iter()
                .map(|item| item.cursor.sequence)
                .collect::<Vec<_>>(),
            vec![6, 4, 3]
        );
        assert!(replay.items.iter().all(|item| {
            matches!(
                item.event,
                crate::PublicDaemonEvent::AgentStream(AgentStreamEvent {
                    emission: StreamEmission {
                        frame: AgentStreamFrame::AssistantTurnStarted
                            | AgentStreamFrame::ToolCallStarted { .. }
                            | AgentStreamFrame::ToolCallCompleted { .. },
                        ..
                    },
                    ..
                })
            )
        }));
    }

    #[test]
    fn record_artifact_does_not_leave_ghost_activity_on_duplicate_artifact() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let started = ensure_running_run(&app, &execution, &session.id, "Ship patch");
        let artifact_id = ArtifactId::new("artifact-a").expect("artifact id");

        execution
            .record_artifact(ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session.id.clone(),
                run_id: started.id.clone(),
                kind: ArtifactKind::Patch,
                metadata: ArtifactMetadata::Standard,
                storage_path: "artifacts/run-a/patch.diff".to_string(),
            })
            .expect("first artifact should record");

        let error = execution
            .record_artifact(ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session.id.clone(),
                run_id: started.id,
                kind: ArtifactKind::Patch,
                metadata: ArtifactMetadata::Standard,
                storage_path: "artifacts/run-a/patch-2.diff".to_string(),
            })
            .expect_err("duplicate artifact must fail");

        let listed = app
            .list_artifacts(
                &session.id,
                &ListArtifactsQuery {
                    run_id: None,
                    artifact_id: Some(artifact_id),
                },
            )
            .expect("artifacts");
        let page = app
            .activity_page(
                &session.id,
                &crate::ActivityPageQuery {
                    limit: 10,
                    before: None,
                    kinds: vec![DaemonEventKind::Artifact],
                },
            )
            .expect("activity page");

        assert!(matches!(
            error,
            RunExecutionError::Store(StoreError::DuplicateRecord { .. })
        ));
        assert_eq!(listed.items.len(), 1);
        assert!(listed.latest_cursor.is_some());
        assert_eq!(page.items.len(), 1);
    }

    #[test]
    fn record_artifact_keeps_artifact_when_auto_receipt_create_fails() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Receipt failure".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let started = ensure_running_run(&app, &execution, &session.id, "Ship patch");
        let artifact_id = ArtifactId::new("artifact-receipt-fails").expect("artifact id");
        {
            let mut store = app.store.lock().expect("store lock");
            store.fail_next_receipt_create_for_tests();
        }

        let (recorded, logs) = capture_logs(|| {
            execution
                .record_artifact(ArtifactRecord {
                    id: artifact_id.clone(),
                    session_id: session.id.clone(),
                    run_id: started.id.clone(),
                    kind: ArtifactKind::Patch,
                    metadata: ArtifactMetadata::Standard,
                    storage_path: "artifacts/run-a/patch.diff".to_string(),
                })
                .expect("artifact should still record")
        });

        let artifacts = app
            .list_artifacts(
                &session.id,
                &ListArtifactsQuery {
                    run_id: Some(started.id.clone()),
                    artifact_id: Some(artifact_id.clone()),
                },
            )
            .expect("artifacts should list");
        let receipts = app
            .list_receipts(
                &session.id,
                &crate::ListReceiptsRequest {
                    session_id: session.id.clone(),
                    run_id: Some(started.id),
                    parent_run_id: None,
                    state: None,
                    kind: None,
                    limit: None,
                },
            )
            .expect("receipts should list");

        assert_eq!(recorded.events.len(), 1);
        assert!(matches!(
            &recorded.events[0].payload,
            DaemonEvent::Artifact(event) if event.artifact.id == artifact_id
        ));
        assert_eq!(artifacts.items.len(), 1);
        assert_eq!(artifacts.items[0].id, artifact_id);
        assert!(receipts.receipts.is_empty());
        assert!(logs.contains("failed to auto-create artifact receipt after artifact commit"));
        assert!(logs.contains("injected receipt create failure"));
    }

    #[test]
    fn record_artifact_rejects_run_without_live_owner() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime.clone());
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                    workspace_id: ta_store::default_test_workspace_id(),
                },
            )
            .expect("session should open");
        let started = ensure_running_run(&app, &execution, &session.id, "Ship patch");

        let execution_runtime = runtime.run_execution_runtime();
        let generation = execution_runtime
            .live_execution_for(&started.id)
            .filter(|live_execution| live_execution.session_id == session.id)
            .expect("started run should have an execution")
            .generation;
        execution_runtime
            .with_terminal_live_generation_lease_and_take_handle(
                &started.id,
                &session.id,
                generation,
                || Ok(()),
            )
            .expect("test terminal lease should retire the owner");

        let error = execution
            .record_artifact(ArtifactRecord {
                id: ArtifactId::new("artifact-live-owned").expect("artifact id"),
                session_id: session.id.clone(),
                run_id: started.id,
                kind: ArtifactKind::Patch,
                metadata: ArtifactMetadata::Standard,
                storage_path: "artifacts/run-a/patch.diff".to_string(),
            })
            .expect_err("artifact publish must require live owner");

        assert!(matches!(error, RunExecutionError::RunNotLiveOwned(_)));
    }

    fn capture_logs<T>(f: impl FnOnce() -> T) -> (T, String) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(BufferWriterFactory(buffer.clone()))
            .with_ansi(false)
            .with_level(true)
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let logs = String::from_utf8(buffer.lock().expect("log buffer lock").clone())
            .expect("logs should be utf8");
        (result, logs)
    }

    struct BufferWriterFactory(Arc<Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriterFactory {
        type Writer = BufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter(self.0.clone())
        }
    }

    struct BufferWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for BufferWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("log buffer lock")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
