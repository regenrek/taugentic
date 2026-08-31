use ta_policy::{Operation, evaluate_execution_context};
use ta_protocol::wire::{AgentRuntimeMediaCapability, StartRunCommand, WorkspaceFileKind};
use ta_store::{CommitRunTransition, UserTurnCommit};
use uuid::Uuid;

use super::provider_sink::RunCompletionProjection;
use super::*;
use crate::{
    DelegateRecipeResolutionRequest, ResolvedDelegateRecipeRequest, resolve_delegate_recipe,
};

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start_run(
        &self,
        session_id: crate::SessionId,
        command: StartRunCommand,
    ) -> Result<RunMutationResult, RunExecutionError> {
        if !command.attachments.is_empty() {
            return Err(RunExecutionError::UnvalidatedWorkspaceFileAttachments);
        }
        self.start_run_with_validated_attachments(session_id, command, Vec::new())
    }

    pub(crate) fn start_run_with_validated_attachments(
        &self,
        session_id: crate::SessionId,
        command: StartRunCommand,
        attachments: Vec<ta_protocol::wire::WorkspaceFileAttachment>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        if command.attachments.len() != attachments.len()
            || command
                .attachments
                .iter()
                .zip(&attachments)
                .any(|(request, attachment)| {
                    request.path != attachment.path
                        || request.expected_revision != attachment.revision
                })
        {
            return Err(RunExecutionError::UnvalidatedWorkspaceFileAttachments);
        }
        let selection = command.selection.clone();
        let resolved_command = self.resolve_start_run_command(command)?;
        let objective = resolved_command.objective.trim();
        if objective.is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
        }
        let user_turn = UserTurnCommit::Append {
            text: objective.to_string(),
            attachments: attachments.clone(),
        };
        let persist_initial_selection = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let Some(session) = store.session(&session_id)? else {
                return Err(RunExecutionError::SessionNotFound(
                    session_id.as_str().to_string(),
                ));
            };
            match session.next_run_selection {
                ta_protocol::wire::SessionNextRunSelection::Unselected => true,
                ta_protocol::wire::SessionNextRunSelection::Selected {
                    selection: persisted,
                } if persisted == selection => false,
                ta_protocol::wire::SessionNextRunSelection::Selected { .. } => {
                    return Err(RunExecutionError::ProviderExecutionFailed(
                        "run selection does not agree with the session's next-run selection"
                            .to_string(),
                    ));
                }
            }
        };
        let validated_selection = self
            .agent_runtime
            .validate_run_selection(&selection)
            .map_err(map_agent_runtime_error)?;
        if persist_initial_selection {
            self.store
                .lock()
                .expect("app store should not be poisoned")
                .commit_session_next_run_selection(ta_store::CommitSessionNextRunSelection {
                    session_id: session_id.clone(),
                    selection: ta_protocol::wire::SessionNextRunSelection::Selected {
                        selection: selection.clone(),
                    },
                })?;
        }
        if attachments
            .iter()
            .any(|attachment| attachment.kind == WorkspaceFileKind::Image)
            && validated_selection.media_capabilities().image_input
                == AgentRuntimeMediaCapability::Unsupported
        {
            return Err(RunExecutionError::ProviderExecutionFailed(
                "the selected runtime does not support image input".to_string(),
            ));
        }
        let runtime_profile = validated_selection.runtime_profile().clone();
        let route = validated_selection.route().clone();

        let run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error| {
            self.runtime
                .finish_scheduled_run(&session_id, &run_id, RunStatus::Failed);
            error
        };
        let prepared_context = self
            .prepare_execution_context(
                &session_id,
                &run_id,
                &runtime_profile,
                ExecutionContextRequest::workspace_write(),
            )
            .map_err(fail_scheduled_run)?;
        let decision = match runtime_profile.execution_kind {
            ta_protocol::wire::RuntimeProfileExecutionKind::AgentRun => evaluate_execution_context(
                &prepared_context.execution_context,
                &Operation::new(ApprovalScope::ProcessExec, "execute run"),
            ),
            ta_protocol::wire::RuntimeProfileExecutionKind::RealtimeVoice => {
                ta_policy::PolicyDecision::Allow
            }
        };
        let harness = validated_selection.execution_harness();

        let (mut run, mut events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, events) = match disposition {
                crate::RunScheduleDisposition::StartNow => build_start_transition(
                    run_id.clone(),
                    decision,
                    resolved_command.recipe_id.clone(),
                ),
                crate::RunScheduleDisposition::Queued { position } => build_queue_transition(
                    run_id.clone(),
                    position,
                    resolved_command.recipe_id.clone(),
                ),
            };
            let run = RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective: objective.to_string(),
                status,
                harness: match runtime_profile.execution_kind {
                    ta_protocol::wire::RuntimeProfileExecutionKind::AgentRun => {
                        run_harness_kind(harness)
                    }
                    ta_protocol::wire::RuntimeProfileExecutionKind::RealtimeVoice => {
                        RunHarnessKind::RealtimeVoice
                    }
                },
                source: RunSource::User {
                    route: route.clone(),
                    output_contract: resolved_command.output_contract,
                    model_id: resolved_command.model_id.clone(),
                    recipe_id: resolved_command.recipe_id.clone(),
                    attachments,
                },
                execution_context: prepared_context.execution_context,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: prepared_context.workspace_info,
                claimed_files: prepared_context.claimed_files,
                conflict_summary: prepared_context.conflict_summary,
            };
            let committed = store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: run.clone(),
                    user_turn,
                    events,
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .map_err(|error| fail_scheduled_run(error.into()))?;
            (committed.run, committed.events)
        };
        if run.status == RunStatus::Running {
            let generation = self
                .runtime
                .claim_live_run(run.id.clone(), session_id.clone());
            let start_result = self.start_provider_execution(
                &session_id,
                &run.id,
                &runtime_profile,
                run.source.route(),
                generation,
            );
            let latest_run = self.load_run_projection(&run.id)?;
            if let Err(error) = start_result
                && latest_run.status == RunStatus::Running
            {
                let failed = self.commit_failed_live_run_for_generation(
                    session_id.clone(),
                    &latest_run.id,
                    error.to_string(),
                    RunCompletionProjection::default(),
                    generation,
                )?;
                run = self.load_run_projection(&latest_run.id)?;
                events.extend(failed.events);
            } else if latest_run.status != RunStatus::Cancelled {
                run = latest_run;
            }
        }
        if matches!(run.status, RunStatus::Failed) {
            events.extend(self.advance_ready_queue(&session_id, &run.id, RunStatus::Failed)?);
        }

        let run = project_run_summary(run);
        Ok(RunMutationResult { run, events })
    }

    fn resolve_start_run_command(
        &self,
        command: StartRunCommand,
    ) -> Result<ResolvedDelegateRecipeRequest, RunExecutionError> {
        resolve_delegate_recipe(
            &self.recipe_registry,
            DelegateRecipeResolutionRequest {
                objective: command.objective,
                output_contract: None,
                model_id: command.selection.model_id,
                recipe_id: command.recipe_id,
            },
        )
        .map_err(map_recipe_resolution_error)
    }

    pub(super) fn start_provider_execution(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        runtime_profile: &crate::RuntimeProfileSummary,
        route: &ta_protocol::wire::RunExecutionRoute,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        self.start_provider_execution_for_generation(
            session_id,
            run_id,
            runtime_profile,
            route,
            generation,
        )
    }

    pub(super) fn start_provider_execution_for_generation(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        runtime_profile: &crate::RuntimeProfileSummary,
        route: &ta_protocol::wire::RunExecutionRoute,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        if runtime_profile.execution_kind
            == ta_protocol::wire::RuntimeProfileExecutionKind::RealtimeVoice
        {
            let run = self.load_run_projection(run_id)?;
            let handle = crate::orchestration::voice::start_realtime_execution(
                self.clone(),
                session_id.clone(),
                run_id.clone(),
                generation,
                runtime_profile,
                route,
                run.objective,
            )?;
            return self
                .runtime
                .attach_voice_run(
                    run_id,
                    generation,
                    handle.clone() as Arc<dyn taugentic_agent::ExecutionHandle>,
                    handle as Arc<dyn crate::orchestration::voice::VoiceFrameExchange>,
                )
                .map_err(map_agent_runtime_error);
        }
        self.enforce_budget_before_dispatch(session_id, run_id, generation)?;
        self.capture_before_user_turn(run_id)?;
        let native_history = self.native_history_initial_state_for_run(session_id, run_id)?;
        self.start_provider_execution_with_optional_initial_state(
            session_id,
            run_id,
            runtime_profile,
            route,
            generation,
            native_history,
        )
    }

    fn start_provider_execution_with_optional_initial_state(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        runtime_profile: &crate::RuntimeProfileSummary,
        route: &ta_protocol::wire::RunExecutionRoute,
        generation: u64,
        native_history: Option<taugentic_agent::NativeHistoryInitialState>,
    ) -> Result<(), RunExecutionError> {
        let run = self.load_run_projection(run_id)?;
        let (objective, attachments) = match &run.source {
            RunSource::User { attachments, .. } => (
                user_message_with_attachments(&run.objective, attachments),
                attachments.clone(),
            ),
            RunSource::ScheduledWork { .. }
            | RunSource::NativeSubagent { .. }
            | RunSource::FreshSpawn { .. }
            | RunSource::Forked { .. }
            | RunSource::RouteSwitchedContinuation { .. } => (run.objective.clone(), Vec::new()),
        };
        let image_attachments = attachments
            .iter()
            .filter(|attachment| attachment.kind == ta_protocol::wire::WorkspaceFileKind::Image)
            .collect::<Vec<_>>();
        if image_attachments.len() > ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT {
            return Err(RunExecutionError::ProviderExecutionFailed(
                "image attachment count exceeds the runtime limit".to_string(),
            ));
        }
        let mut image_total = 0u64;
        for attachment in image_attachments {
            let request = ta_protocol::wire::WorkspaceFileAttachmentRequest {
                path: attachment.path.clone(),
                expected_revision: attachment.revision.clone(),
            };
            let checked = crate::workspace::files::validate_workspace_file_attachment(
                run.execution_context.effective_cwd.as_path(),
                &request,
            )
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
            if checked.kind != ta_protocol::wire::WorkspaceFileKind::Image
                || checked.byte_len > ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES
            {
                return Err(RunExecutionError::ProviderExecutionFailed(
                    "image attachment failed runtime preflight".to_string(),
                ));
            }
            image_total = image_total.saturating_add(checked.byte_len);
        }
        if image_total > ta_protocol::wire::WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES {
            return Err(RunExecutionError::ProviderExecutionFailed(
                "image attachment total exceeds the runtime limit".to_string(),
            ));
        }
        let output_contract = output_contract_for_run(&run);
        self.runtime
            .start_provider_run(
                crate::ProviderRunStart {
                    runtime_profile,
                    route,
                    session_id,
                    run_id,
                    objective: &objective,
                    execution_context: Arc::new(run.execution_context),
                    native_history,
                    output_contract,
                    subagent_recipes: self
                        .recipe_registry
                        .recipes()
                        .into_iter()
                        .cloned()
                        .collect(),
                    attachments,
                    generation,
                },
                Arc::new(ProviderRunExecutionSink {
                    service: self.clone(),
                    session_id: session_id.clone(),
                    run_id: run_id.clone(),
                    generation,
                }),
            )
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::SessionId;
    use crate::orchestration::run_execution::test_support::*;
    use sha2::{Digest, Sha256};
    use ta_protocol::wire::{
        OutputContractKind, RunSource, RunStatus, WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES,
        WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT, WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES,
        WorkspaceFileAttachment, WorkspaceFileAttachmentRequest, WorkspaceFileKind,
    };
    use ta_store::{ProjectionRepository, StoreSeedRepository};

    fn revision(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn image_attachment(path: &str, bytes: &[u8]) -> WorkspaceFileAttachment {
        WorkspaceFileAttachment {
            path: path.to_string(),
            revision: revision(bytes),
            kind: WorkspaceFileKind::Image,
            byte_len: bytes.len() as u64,
        }
    }

    fn replace_run_attachments(
        execution: &RunExecutionService<ta_store::InMemoryStore>,
        run_id: &RunId,
        attachments: Vec<WorkspaceFileAttachment>,
    ) {
        let mut store = execution
            .store
            .lock()
            .expect("test store should not be poisoned");
        let mut run = store.run(run_id).expect("run query").expect("seeded run");
        if let RunSource::User {
            attachments: current,
            ..
        } = &mut run.source
        {
            current.clear();
            current.extend(attachments);
        } else {
            panic!("fixture must seed a user run");
        }
        store.save_run(run).expect("updated fixture run");
    }

    #[test]
    fn start_run_rejects_blank_objective() {
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

        let error = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "   ", "runtime-openai-safe"),
            )
            .expect_err("blank objective must fail");

        assert!(matches!(error, RunExecutionError::EmptyRunObjective));
    }

    #[test]
    fn start_run_rejects_unknown_session() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);

        let error = execution
            .start_run(
                SessionId::new("session-missing").expect("session id"),
                start_run_command(&app, "Ship app server hard cut", "runtime-openai-safe"),
            )
            .expect_err("missing session must fail");

        assert!(matches!(
            error,
            RunExecutionError::SessionNotFound(ref session_id) if session_id == "session-missing"
        ));
    }

    #[test]
    fn run_execution_rejects_workspace_attachments_not_validated_by_the_app_boundary() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Attachment validation owner");
        let mut command = start_run_command(&app, "Use attached context", "runtime-openai-safe");
        command.attachments = vec![WorkspaceFileAttachmentRequest {
            path: "context.md".to_string(),
            expected_revision: "revision-1".to_string(),
        }];

        let error = execution
            .start_run(session.id, command)
            .expect_err("unvalidated attachment must fail closed");

        assert!(matches!(
            error,
            RunExecutionError::UnvalidatedWorkspaceFileAttachments
        ));
    }

    #[test]
    fn provider_user_message_contains_one_structured_attachment_manifest() {
        let message = user_message_with_attachments(
            "Inspect this file",
            &[
                WorkspaceFileAttachment {
                    path: "src/main.rs".to_string(),
                    revision: "sha256:abc".to_string(),
                    kind: WorkspaceFileKind::Text,
                    byte_len: 42,
                },
                image_attachment("diagram.png", b"\x89PNG\r\n\x1a\n"),
            ],
        );

        assert_eq!(
            message,
            "Inspect this file\n\n<taugentic_workspace_attachments>\n[{\"byteLength\":\"42\",\"kind\":\"text\",\"path\":\"src/main.rs\",\"revision\":\"sha256:abc\"}]\n</taugentic_workspace_attachments>"
        );
        assert!(!message.contains("diagram.png"));
    }

    #[test]
    fn unsupported_image_input_rejects_before_run_persistence() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Unsupported image runtime");
        let bytes = b"\x89PNG\r\n\x1a\n";
        let attachment = image_attachment("diagram.png", bytes);
        let mut command = start_run_command(&app, "Inspect the image", "runtime-deepseek-safe");
        command.selection.model_id = Some(
            ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro")
                .expect("known text-only model"),
        );
        command.attachments = vec![WorkspaceFileAttachmentRequest {
            path: attachment.path.clone(),
            expected_revision: attachment.revision.clone(),
        }];

        let error = execution
            .start_run_with_validated_attachments(session.id.clone(), command, vec![attachment])
            .expect_err("unsupported image input must reject before a run is stored");
        assert!(matches!(
            error,
            RunExecutionError::ProviderExecutionFailed(message)
                if message.contains("does not support image input")
        ));
        assert!(
            app.list_runs(&session.id)
                .expect("runs should list")
                .is_empty(),
            "unsupported media must not persist a run"
        );
        assert!(
            app.agent_turns_page(
                &session.id,
                &crate::AgentTurnsPageQuery {
                    limit: 10,
                    before: None
                },
            )
            .expect("turns should list")
            .items
            .is_empty(),
            "unsupported media must not persist a user turn"
        );
    }

    #[test]
    fn image_attachments_are_revalidated_immediately_before_provider_dispatch() {
        let repository = init_dispatch_repo();
        let (runtime, dispatcher) = runtime_with_dispatch_plans([]);
        let (app, execution) = app_and_execution_with_runtime(runtime);
        set_default_test_workspace_root(&app, repository.path());
        let selection = validated_runtime_selection(&app, "runtime-codex-allow");

        let stale_path = repository.path().join("stale.png");
        let stale_before = b"\x89PNG\r\n\x1a\nbefore";
        std::fs::write(&stale_path, stale_before).expect("stale fixture should write");
        let stale = image_attachment("stale.png", stale_before);
        std::fs::write(&stale_path, b"\x89PNG\r\n\x1a\nafter")
            .expect("stale fixture should change");

        let invalid_path = repository.path().join("invalid.png");
        let invalid_bytes = b"not-an-image";
        std::fs::write(&invalid_path, invalid_bytes).expect("invalid fixture should write");
        let invalid = image_attachment("invalid.png", invalid_bytes);

        let oversize_path = repository.path().join("oversize.png");
        let mut oversize = b"\x89PNG\r\n\x1a\n".to_vec();
        oversize.resize(WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES as usize + 1, 0);
        std::fs::write(&oversize_path, &oversize).expect("oversize fixture should write");
        let oversize_attachment = image_attachment("oversize.png", &oversize);

        let total_each = WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES / 3 + 1;
        let mut aggregate = b"\x89PNG\r\n\x1a\n".to_vec();
        aggregate.resize(total_each as usize, 0);
        for index in 0..3 {
            std::fs::write(
                repository.path().join(format!("aggregate-{index}.png")),
                &aggregate,
            )
            .expect("aggregate fixture should write");
        }
        let aggregate_attachments = (0..3)
            .map(|index| image_attachment(&format!("aggregate-{index}.png"), &aggregate))
            .collect::<Vec<_>>();
        let count_attachments =
            std::iter::repeat_with(|| image_attachment("stale.png", stale_before))
                .take(WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT as usize + 1)
                .collect::<Vec<_>>();

        for attachments in [
            vec![stale],
            vec![invalid],
            vec![oversize_attachment],
            count_attachments,
            aggregate_attachments,
        ] {
            let session = open_session(&app, "Image dispatch preflight");
            let run = ensure_running_run_with_profile(
                &app,
                &execution,
                &session.id,
                "Revalidate image before dispatch",
                "runtime-codex-allow",
            );
            replace_run_attachments(&execution, &run.id, attachments);
            let generation = execution
                .runtime
                .live_execution_for(&run.id)
                .expect("live execution")
                .generation;
            let error = execution
                .start_provider_execution(
                    &session.id,
                    &run.id,
                    selection.runtime_profile(),
                    selection.route(),
                    generation,
                )
                .expect_err("changed image input must reject before provider dispatch");
            assert!(matches!(
                error,
                RunExecutionError::ProviderExecutionFailed(_)
            ));
        }
        assert!(
            dispatcher.requests().is_empty(),
            "failed immediate revalidation must not dispatch to the provider"
        );
    }

    #[test]
    fn start_run_with_require_approval_mode_waits_for_approval() {
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

        let started = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship policy gated run", "runtime-codex-safe"),
            )
            .expect("run should start");

        assert_eq!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_some());
    }

    #[test]
    fn native_and_acp_runs_persist_the_same_workspace_context_identity() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Cross-harness context");

        let mut catalog = ta_model_catalog::ModelCatalog::embedded().expect("embedded catalog");
        let acp_model = catalog
            .providers
            .get("openai")
            .and_then(|provider| provider.models.get("gpt-5.6-sol"))
            .cloned()
            .expect("known ACP fixture model");
        catalog.providers.insert(
            "codex-acp".to_string(),
            ta_model_catalog::CatalogProvider {
                id: "codex-acp".to_string(),
                name: "Codex ACP".to_string(),
                models: [(acp_model.id.clone(), acp_model)].into_iter().collect(),
            },
        );
        app.agent_runtime
            .replace_model_catalog_for_tests(catalog)
            .expect("ACP fixture catalog should install");

        let native = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Native context proof", "runtime-openai-safe"),
            )
            .expect("native run should persist before approval");

        let acp_selection = ta_protocol::wire::AgentRuntimeSelection {
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-acp-safe")
                .expect("runtime profile id"),
            auth_profile_id: None,
            model_id: Some(
                ta_protocol::wire::AgentRuntimeModelId::new("gpt-5.6-sol")
                    .expect("known ACP fixture model"),
            ),
        };
        app.set_session_next_run_selection(
            &session.id,
            ta_protocol::wire::SessionNextRunSelection::Selected {
                selection: acp_selection.clone(),
            },
        )
        .expect("ACP selection should persist before its run starts");

        let acp = execution
            .start_run(
                session.id.clone(),
                StartRunCommand::new("ACP context proof", acp_selection),
            )
            .expect("ACP run should queue and persist");

        let native = execution
            .load_run_projection(&native.run.id)
            .expect("native run projection");
        let acp = execution
            .load_run_projection(&acp.run.id)
            .expect("ACP run projection");

        assert_eq!(native.harness, RunHarnessKind::Native);
        assert_eq!(acp.harness, RunHarnessKind::Acp);
        assert_eq!(
            acp.execution_context.workspace_id,
            native.execution_context.workspace_id
        );
        assert_eq!(
            acp.execution_context.workspace_root,
            native.execution_context.workspace_root
        );
        assert_eq!(
            acp.execution_context.effective_cwd,
            native.execution_context.effective_cwd
        );
    }

    #[test]
    fn start_run_with_allow_mode_runs_without_approval() {
        let (runtime, dispatcher) =
            runtime_with_dispatch_plans([DispatchPlan::Succeed(Arc::new(NoopExecutionHandle))]);
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

        let started = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship policy allow run", "runtime-codex-allow"),
            )
            .expect("run should start");

        assert_ne!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_none());
        let requests = dispatcher.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].runtime_profile_id.as_str(),
            "runtime-codex-allow"
        );
    }

    #[test]
    fn start_run_resolves_recipe_before_provider_execution() {
        let (runtime, dispatcher) =
            runtime_with_dispatch_plans([DispatchPlan::Succeed(Arc::new(NoopExecutionHandle))]);
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

        let mut command = start_run_command(
            &app,
            "Find the failing login redirect",
            "runtime-codex-allow",
        );
        command.recipe_id = Some("debug-agent".to_string());
        let started = execution
            .start_run(session.id.clone(), command)
            .expect("recipe-backed run should start");
        let run = execution
            .load_run_projection(&started.run.id)
            .expect("started run should be durable");

        assert!(run.objective.contains("Find the failing login redirect"));
        assert_eq!(
            output_contract_for_run(&run),
            Some(OutputContractKind::Debug)
        );
        assert_eq!(recipe_id_for_run(&run).as_deref(), Some("debug-agent"));
        assert!(matches!(
            &run.source,
            RunSource::User {
                recipe_id: Some(recipe_id),
                ..
            } if recipe_id == "debug-agent"
        ));
        let requests = dispatcher.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].output_contract, Some(OutputContractKind::Debug));
    }

    #[test]
    fn start_run_with_deny_mode_fails_immediately() {
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

        let started = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship policy denied run", "runtime-codex-deny"),
            )
            .expect("run should start");

        assert_eq!(started.run.status, RunStatus::Failed);
        assert!(started.requested_approval_id().is_none());
    }

    #[test]
    fn patching_explicit_profile_updates_live_run_policy() {
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

        let snapshot = app
            .patch_agent_runtime_profile(&crate::DaemonAgentRuntimePatchProfileParams {
                runtime_profile_id: crate::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                patch: crate::RuntimeProfilePatch {
                    policy_mode: Some(crate::RuntimePolicyMode::Allow),
                    ..Default::default()
                },
            })
            .expect("runtime profile patch should succeed");
        let patched_profile = snapshot
            .runtime_profiles
            .iter()
            .find(|profile| profile.id.as_str() == "runtime-codex-safe")
            .expect("patched runtime profile should exist");

        let started = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship patched policy run", "runtime-codex-safe"),
            )
            .expect("run should start");

        assert_eq!(patched_profile.policy_mode, crate::RuntimePolicyMode::Allow);
        assert_ne!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_none());
    }

    #[test]
    fn start_run_queues_when_session_already_has_active_run() {
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

        let first = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship queue owner", "runtime-openai-safe"),
            )
            .expect("first run should start");
        let second = execution
            .start_run(
                session.id.clone(),
                start_run_command(&app, "Ship follow-up queue item", "runtime-openai-safe"),
            )
            .expect("second run should queue");

        let runs = app.list_runs(&session.id).expect("runs should list");

        assert!(matches!(
            first.run.status,
            RunStatus::Running | RunStatus::WaitingForApproval
        ));
        assert_eq!(second.run.status, RunStatus::Queued);
        assert!(second.requested_approval_id().is_none());
        assert!(
            runs.iter()
                .any(|run| run.id == second.run.id && run.status == RunStatus::Queued)
        );
    }
}
