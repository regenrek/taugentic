use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, ApprovalResolution,
    RuntimeProfileExecutionKind, StreamEmission, VOICE_FRAME_BYTES, VoiceEvent, VoicePhase,
    VoiceStreamEndReason,
};
use ta_provider_llm::realtime::{RealtimeEvent, RealtimeSession};
use ta_store::{InMemoryStore, PersistenceStore};
use taugentic_agent::{ExecutionError, ExecutionHandle, ExecutionSink};

use crate::orchestration::run_execution::provider_sink::ProviderRunExecutionSink;
use crate::orchestration::run_execution::{RunExecutionError, RunExecutionService};
use crate::{RunId, SessionId};

pub(crate) struct VoiceExchange {
    pub(crate) output: Option<[u8; VOICE_FRAME_BYTES]>,
    pub(crate) state: Option<VoiceEvent>,
    pub(crate) playback_interrupted: bool,
}

pub(crate) trait VoiceFrameExchange: Send + Sync {
    fn exchange(
        &self,
        input: [u8; VOICE_FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<VoiceExchange, String>;
    fn end(&self, reason: VoiceStreamEndReason) -> Result<(), String>;
}

pub(crate) struct RealtimeExecutionHandle<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    session: Arc<RealtimeSession>,
    sink: Arc<ProviderRunExecutionSink<S>>,
    fragment_sequence: Mutex<u64>,
    terminal: TerminalOnce,
}

#[derive(Default)]
struct TerminalOnce(AtomicBool);

impl TerminalOnce {
    fn claim(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
}

impl<S> std::fmt::Debug for RealtimeExecutionHandle<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealtimeExecutionHandle")
            .finish_non_exhaustive()
    }
}

impl<S> ExecutionHandle for RealtimeExecutionHandle<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.session.cancel();
        Ok(())
    }

    fn resolve_approval(&self, _: ApprovalResolution) -> Result<(), ExecutionError> {
        Err(ExecutionError::Unsupported(
            "realtime voice does not request approvals".to_string(),
        ))
    }
}

impl<S> VoiceFrameExchange for RealtimeExecutionHandle<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn exchange(
        &self,
        input: [u8; VOICE_FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<VoiceExchange, String> {
        match self.session.exchange(input, playback_completed_frames) {
            Ok(exchange) => {
                let (state, playback_interrupted) = self.apply_events(exchange.events)?;
                Ok(VoiceExchange {
                    output: exchange.output,
                    state,
                    playback_interrupted,
                })
            }
            Err(error) => {
                let detail = error.to_string();
                self.fail_once(&detail)?;
                Err(detail)
            }
        }
    }

    fn end(&self, reason: VoiceStreamEndReason) -> Result<(), String> {
        self.session.cancel();
        let detail = match reason {
            VoiceStreamEndReason::Interrupted => "voice input was interrupted",
            VoiceStreamEndReason::CaptureOverflow => {
                "voice capture backpressure limit was exceeded"
            }
            VoiceStreamEndReason::PlaybackOverflow => {
                "voice playback backpressure limit was exceeded"
            }
            VoiceStreamEndReason::DeviceUnavailable => "voice device became unavailable",
            VoiceStreamEndReason::Replaced => "voice session was replaced",
            VoiceStreamEndReason::Shutdown => "voice bridge shut down",
        };
        self.fail_once(detail)
    }
}

impl<S> RealtimeExecutionHandle<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn apply_events(
        &self,
        events: Vec<RealtimeEvent>,
    ) -> Result<(Option<VoiceEvent>, bool), String> {
        let mut state = None;
        let mut playback_interrupted = false;
        for event in events {
            match event {
                RealtimeEvent::Connected | RealtimeEvent::Listening => {
                    state = Some(VoiceEvent {
                        run_id: self.sink.run_id.clone(),
                        phase: VoicePhase::Listening,
                    });
                }
                RealtimeEvent::Speaking => {
                    state = Some(VoiceEvent {
                        run_id: self.sink.run_id.clone(),
                        phase: VoicePhase::Speaking,
                    });
                    self.push_stream(AgentStreamFrame::AssistantTurnStarted)?;
                }
                RealtimeEvent::TranscriptDelta(delta) => {
                    self.push_stream(AgentStreamFrame::AssistantMessageDelta { delta })?;
                }
                RealtimeEvent::ResponseCompleted => {
                    self.push_stream(AgentStreamFrame::AssistantTurnCompleted)?;
                }
                RealtimeEvent::PlaybackInterrupted => {
                    playback_interrupted = true;
                }
            }
        }
        Ok((state, playback_interrupted))
    }

    fn fail_once(&self, detail: &str) -> Result<(), String> {
        if !self.terminal.claim() {
            return Ok(());
        }
        self.sink
            .fail(ExecutionError::ProcessFailed(detail.to_string()))
            .map_err(|error| error.to_string())
    }

    fn push_stream(&self, frame: AgentStreamFrame) -> Result<(), String> {
        let mut sequence = self
            .fragment_sequence
            .lock()
            .expect("voice fragment sequence should not be poisoned");
        *sequence = sequence.saturating_add(1);
        self.sink
            .push_stream(StreamEmission {
                turn_id: Some(
                    AgentStreamTurnId::new(format!("voice-{}", self.sink.run_id.as_str()))
                        .expect("run-derived voice turn id"),
                ),
                item_id: Some(
                    AgentStreamItemId::new(format!("voice-{}", self.sink.run_id.as_str()))
                        .expect("run-derived voice item id"),
                ),
                fragment_sequence: Some(*sequence),
                frame,
            })
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn start_realtime_execution<S>(
    service: RunExecutionService<S>,
    session_id: SessionId,
    run_id: RunId,
    generation: u64,
    runtime_profile: &ta_protocol::wire::RuntimeProfileSummary,
    route: &ta_protocol::wire::RunExecutionRoute,
    instructions: String,
) -> Result<Arc<RealtimeExecutionHandle<S>>, RunExecutionError>
where
    S: PersistenceStore + Send + 'static,
{
    if runtime_profile.execution_kind != RuntimeProfileExecutionKind::RealtimeVoice
        || route.harness != ta_protocol::wire::RunHarnessKind::RealtimeVoice
    {
        return Err(RunExecutionError::ProviderExecutionFailed(
            "stored run route is not realtime voice".to_string(),
        ));
    }
    let model_id = route.model_id.clone().ok_or_else(|| {
        RunExecutionError::ProviderExecutionFailed("realtime voice requires a model".to_string())
    })?;
    let session = RealtimeSession::start(model_id, instructions)
        .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
    let sink = Arc::new(ProviderRunExecutionSink {
        service,
        session_id,
        run_id,
        generation,
    });
    Ok(Arc::new(RealtimeExecutionHandle {
        session,
        sink,
        fragment_sequence: Mutex::new(0),
        terminal: TerminalOnce::default(),
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::TerminalOnce;

    #[test]
    fn provider_device_and_cancel_races_have_one_terminal_owner() {
        let terminal = Arc::new(TerminalOnce::default());
        let claims = (0..8)
            .map(|_| {
                let terminal = Arc::clone(&terminal);
                std::thread::spawn(move || terminal.claim())
            })
            .map(|worker| worker.join().expect("terminal worker"))
            .filter(|claimed| *claimed)
            .count();
        assert_eq!(claims, 1);
    }
}
