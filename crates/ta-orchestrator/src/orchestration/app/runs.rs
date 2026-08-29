use std::collections::BTreeSet;

use ta_protocol::wire::{
    TrustState, VoiceEvent, VoicePhase, VoiceStreamEndParams, VoiceStreamEndResult,
    VoiceStreamExchangeParams, VoiceStreamExchangeResult, VoiceStreamOpenParams,
    VoiceStreamOpenResult, WORKSPACE_FILE_ATTACHMENT_MAX_COUNT, decode_voice_audio,
    encode_voice_audio,
};
use ta_store::{ArtifactRecord, PersistenceStore, RunEventRangeQuery, WorkspaceProjection};

use crate::orchestration::run_events_subscribe::{
    self, RUN_EVENT_REPLAY_BATCH_LIMIT, RunEventSubscription,
};
use crate::{
    ApprovalActor, ArtifactSummary, DaemonRunCompleteWithResultParams, PublicDaemonEvent,
    ResumeRunRequest, ResumeRunResult, RunEventDelta, RunSummary, StartRunCommand,
    SubscribeRunEventsRequest, SubscribeRunEventsResult,
};

use super::{
    AppDeferredMutationResult, AppService, AppServiceError, map_artifact_mutation_result,
    map_run_execution_error, map_run_mutation_result,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(dead_code)]
    pub fn record_artifact(
        &self,
        artifact: ArtifactRecord,
    ) -> Result<AppDeferredMutationResult<ArtifactSummary>, AppServiceError> {
        self.run_execution
            .record_artifact(artifact)
            .map(map_artifact_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub fn start_run(
        &self,
        session_id: &crate::SessionId,
        command: &StartRunCommand,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        let attachments = self.validate_run_attachments(session_id, command)?;
        self.run_execution
            .start_run_with_validated_attachments(session_id.clone(), command.clone(), attachments)
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    fn validate_run_attachments(
        &self,
        session_id: &crate::SessionId,
        command: &StartRunCommand,
    ) -> Result<Vec<ta_protocol::wire::WorkspaceFileAttachment>, AppServiceError> {
        if command.attachments.len() > WORKSPACE_FILE_ATTACHMENT_MAX_COUNT {
            return Err(AppServiceError::WorkspaceFileAttachmentLimitExceeded {
                max: WORKSPACE_FILE_ATTACHMENT_MAX_COUNT,
            });
        }
        let root = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let session = store
                .session(session_id)?
                .ok_or_else(|| AppServiceError::SessionNotFound(session_id.as_str().to_string()))?;
            let workspace = store
                .workspace(&session.workspace_id)?
                .map(WorkspaceProjection::into_inner)
                .ok_or_else(|| {
                    AppServiceError::WorkspaceNotFound(session.workspace_id.as_str().to_string())
                })?;
            if !matches!(workspace.trust_state, TrustState::UserConfirmed { .. }) {
                return Err(AppServiceError::WorkspaceTrustRequired(
                    session.workspace_id.as_str().to_string(),
                ));
            }
            workspace.root_realpath
        };
        let mut paths = BTreeSet::new();
        command
            .attachments
            .iter()
            .map(|request| {
                let attachment = crate::workspace::files::validate_workspace_file_attachment(
                    root.as_path(),
                    request,
                )?;
                if !paths.insert(attachment.path.clone()) {
                    return Err(AppServiceError::WorkspaceFileAttachmentDuplicate(
                        attachment.path,
                    ));
                }
                Ok(attachment)
            })
            .collect()
    }

    pub fn resume_run(
        &self,
        session_id: &crate::SessionId,
        request: &ResumeRunRequest,
    ) -> Result<ResumeRunResult, AppServiceError> {
        self.run_execution
            .resume_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn complete_run_with_result(
        &self,
        session_id: &crate::SessionId,
        request: &DaemonRunCompleteWithResultParams,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .complete_run_with_result(
                session_id.clone(),
                &request.run_id,
                request.detail.clone(),
                request.result.clone(),
            )
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub fn replay_run_events(
        &self,
        session_id: &crate::SessionId,
        request: &SubscribeRunEventsRequest,
    ) -> Result<SubscribeRunEventsResult, AppServiceError> {
        if request.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                request.run_id.as_str().to_string(),
            ));
        }

        let store = self.store.lock().expect("app store should not be poisoned");
        let Some(run) = store.run(&request.run_id)? else {
            return Err(AppServiceError::RunNotFound(
                request.run_id.as_str().to_string(),
            ));
        };
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        let range = store.read_run_events(&RunEventRangeQuery {
            session_id: session_id.clone(),
            run_id: request.run_id.clone(),
            after_sequence: request.after_seq,
            limit: RUN_EVENT_REPLAY_BATCH_LIMIT,
        })?;
        Ok(SubscribeRunEventsResult {
            events: range
                .records
                .into_iter()
                .map(|record| RunEventDelta {
                    seq: record.sequence,
                    event: PublicDaemonEvent::from(record.payload),
                })
                .collect(),
            latest_event_seq: range.latest_sequence,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn subscribe_run_events(
        &self,
        session_id: &crate::SessionId,
        request: &SubscribeRunEventsRequest,
    ) -> Result<RunEventSubscription, AppServiceError> {
        run_events_subscribe::subscribe_run_events(self, session_id, request)
    }

    pub fn cancel_run(
        &self,
        session_id: &crate::SessionId,
        actor: &ApprovalActor,
        run_id: &crate::RunId,
        reason: Option<String>,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .cancel_run(session_id.clone(), actor.clone(), run_id, reason)
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub(crate) fn open_voice_stream(
        &self,
        session_id: &crate::SessionId,
        params: &VoiceStreamOpenParams,
    ) -> Result<VoiceStreamOpenResult, AppServiceError> {
        let run = self
            .run_execution
            .load_run_projection(&params.run_id)
            .map_err(map_run_execution_error)?;
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                params.run_id.as_str().to_string(),
            ));
        }
        let accepted = self.run_execution.is_voice_run(&params.run_id);
        Ok(VoiceStreamOpenResult {
            accepted,
            state: accepted.then(|| VoiceEvent {
                run_id: params.run_id.clone(),
                phase: VoicePhase::Connecting,
            }),
        })
    }

    pub(crate) fn exchange_voice_stream(
        &self,
        session_id: &crate::SessionId,
        params: &VoiceStreamExchangeParams,
    ) -> Result<VoiceStreamExchangeResult, AppServiceError> {
        let run = self
            .run_execution
            .load_run_projection(&params.run_id)
            .map_err(map_run_execution_error)?;
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                params.run_id.as_str().to_string(),
            ));
        }
        let input = decode_voice_audio(&params.audio_base64).ok_or_else(|| {
            AppServiceError::AgentRuntime(crate::AgentRuntimeServiceError::ProviderExecutionFailed(
                "voice audio packet must contain exactly 960 bytes".to_string(),
            ))
        })?;
        let exchange = self
            .run_execution
            .exchange_voice_frame(&params.run_id, input, params.playback_completed_frames)
            .map_err(map_run_execution_error)?;
        Ok(VoiceStreamExchangeResult {
            audio_base64: exchange.output.as_ref().map(encode_voice_audio),
            state: exchange.state,
            playback_interrupted: exchange.playback_interrupted,
        })
    }

    pub(crate) fn end_voice_stream(
        &self,
        session_id: &crate::SessionId,
        params: &VoiceStreamEndParams,
    ) -> Result<VoiceStreamEndResult, AppServiceError> {
        let run = self
            .run_execution
            .load_run_projection(&params.run_id)
            .map_err(map_run_execution_error)?;
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                params.run_id.as_str().to_string(),
            ));
        }
        self.run_execution
            .end_voice(&params.run_id, params.reason)
            .map_err(map_run_execution_error)?;
        Ok(VoiceStreamEndResult {})
    }

    #[cfg(test)]
    pub(crate) fn seed_running_run_for_tests(
        &self,
        session_id: &crate::SessionId,
        objective: &str,
        selection: &crate::AgentRuntimeSelection,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        let validated_selection = self
            .agent_runtime
            .validate_run_selection(selection)
            .map_err(AppServiceError::from)?;
        self.run_execution
            .seed_running_run_for_tests(
                session_id.clone(),
                objective.to_string(),
                validated_selection,
            )
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }
}
