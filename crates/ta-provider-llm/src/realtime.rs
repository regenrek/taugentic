//! OpenAI Realtime WebSocket session for the current provider contract.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use ta_protocol::wire::{
    AgentRuntimeMediaCapabilities, AgentRuntimeMediaCapability, AgentRuntimeModelId,
};
use thiserror::Error;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub const OPENAI_REALTIME_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
pub const SAMPLE_RATE_HZ: u32 = 24_000;
pub const FRAME_BYTES: usize = 960;
const OPENAI_REALTIME_ENDPOINT: &str = "wss://api.openai.com/v1/realtime";
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(5);
const RECEIVE_SLICE: Duration = Duration::from_millis(20);
const MAX_OUTPUT_FRAMES: usize = 50;

/// The Realtime endpoint's compatibility table belongs here, rather than in
/// the catalog or any presentation layer.
pub fn media_capabilities(model_id: &AgentRuntimeModelId) -> Option<AgentRuntimeMediaCapabilities> {
    (model_id.as_str() == "gpt-realtime-2.1").then_some(AgentRuntimeMediaCapabilities {
        image_input: AgentRuntimeMediaCapability::Unsupported,
        image_output: AgentRuntimeMediaCapability::Unsupported,
        voice_input: AgentRuntimeMediaCapability::Supported,
        voice_output: AgentRuntimeMediaCapability::Supported,
    })
}

pub fn credentials_available() -> bool {
    std::env::var(OPENAI_REALTIME_API_KEY_ENV_VAR).is_ok_and(|value| !value.trim().is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeEvent {
    Connected,
    Listening,
    Speaking,
    TranscriptDelta(String),
    ResponseCompleted,
    PlaybackInterrupted,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RealtimeExchange {
    pub output: Option<[u8; FRAME_BYTES]>,
    pub events: Vec<RealtimeEvent>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RealtimeError {
    #[error("OpenAI Realtime credentials are unavailable")]
    CredentialsMissing,
    #[error("the selected model is not compatible with OpenAI Realtime")]
    IncompatibleModel,
    #[error("OpenAI Realtime transport is unavailable")]
    Transport,
    #[error("OpenAI Realtime returned an invalid audio or event payload")]
    Protocol,
    #[error("OpenAI Realtime audio backpressure limit was exceeded")]
    Backpressure,
    #[error("OpenAI Realtime response failed")]
    ResponseFailed,
    #[error("OpenAI Realtime session was cancelled")]
    Cancelled,
}

enum SessionCommand {
    Exchange {
        input: [u8; FRAME_BYTES],
        playback_completed_frames: u64,
        response: mpsc::Sender<Result<RealtimeExchange, RealtimeError>>,
    },
    Cancel,
}

pub struct RealtimeSession {
    commands: SyncSender<SessionCommand>,
    cancelled: Arc<AtomicBool>,
}

impl std::fmt::Debug for RealtimeSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealtimeSession")
            .field("cancelled", &self.cancelled.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl RealtimeSession {
    pub fn start(
        model_id: AgentRuntimeModelId,
        instructions: String,
    ) -> Result<Arc<Self>, RealtimeError> {
        if media_capabilities(&model_id).is_none() {
            return Err(RealtimeError::IncompatibleModel);
        }
        let api_key = std::env::var(OPENAI_REALTIME_API_KEY_ENV_VAR)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or(RealtimeError::CredentialsMissing)?;
        let (commands, receiver) = mpsc::sync_channel(0);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("taugentic-openai-realtime".to_string())
            .spawn(move || run_worker(receiver, worker_cancelled, api_key, model_id, instructions))
            .map_err(|_| RealtimeError::Transport)?;
        Ok(Arc::new(Self {
            commands,
            cancelled,
        }))
    }

    pub fn exchange(
        &self,
        input: [u8; FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<RealtimeExchange, RealtimeError> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(RealtimeError::Cancelled);
        }
        let (response, result) = mpsc::channel();
        self.commands
            .send(SessionCommand::Exchange {
                input,
                playback_completed_frames,
                response,
            })
            .map_err(|_| RealtimeError::Transport)?;
        result
            .recv_timeout(EXCHANGE_TIMEOUT)
            .map_err(|_| RealtimeError::Transport)?
    }

    pub fn cancel(&self) {
        if !self.cancelled.swap(true, Ordering::SeqCst) {
            let _ = self.commands.send(SessionCommand::Cancel);
        }
    }
}

impl Drop for RealtimeSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run_worker(
    receiver: Receiver<SessionCommand>,
    cancelled: Arc<AtomicBool>,
    api_key: String,
    model_id: AgentRuntimeModelId,
    instructions: String,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return,
    };
    runtime.block_on(async move {
        let result = connect(&api_key, &model_id, &instructions).await;
        let mut connection = match result {
            Ok(connection) => Some(connection),
            Err(error) => {
                if let Ok(SessionCommand::Exchange { response, .. }) = receiver.recv() {
                    let _ = response.send(Err(error));
                }
                None
            }
        };
        let Some(mut connection) = connection.take() else {
            return;
        };
        while !cancelled.load(Ordering::SeqCst) {
            match receiver.recv() {
                Ok(SessionCommand::Exchange {
                    input,
                    playback_completed_frames,
                    response,
                }) => {
                    let result = connection.exchange(input, playback_completed_frames).await;
                    let terminal = result.is_err();
                    let _ = response.send(result);
                    if terminal {
                        return;
                    }
                }
                Ok(SessionCommand::Cancel) | Err(_) => {
                    let _ = connection.socket.close(None).await;
                    return;
                }
            }
        }
    });
}

type RealtimeSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Connection {
    socket: RealtimeSocket,
    state: ConnectionState,
    connected_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlaybackCursor {
    item_id: String,
    content_index: u64,
    first_output_ordinal: Option<u64>,
    output_frames_sent: u64,
}

#[derive(Default)]
struct ConnectionState {
    output_frames: VecDeque<[u8; FRAME_BYTES]>,
    output_residual: Vec<u8>,
    playback: Option<PlaybackCursor>,
    response_active: bool,
    output_frame_ordinal: u64,
    playback_completed_frames: u64,
    discarded_output_frames: u64,
}

async fn connect(
    api_key: &str,
    model_id: &AgentRuntimeModelId,
    instructions: &str,
) -> Result<Connection, RealtimeError> {
    let endpoint = format!("{OPENAI_REALTIME_ENDPOINT}?model={}", model_id.as_str());
    let request = tungstenite::http::Request::builder()
        .uri(endpoint)
        .header("Authorization", format!("Bearer {api_key}"))
        .body(())
        .map_err(|_| RealtimeError::Transport)?;
    let (mut socket, _) = connect_async(request)
        .await
        .map_err(|_| RealtimeError::Transport)?;
    socket
        .send(Message::Text(
            json!({
                "type": "session.update",
                "session": {
                    "type": "realtime",
                    "instructions": instructions,
                    "output_modalities": ["audio"],
                    "audio": {
                        "input": {
                            "format": {"type": "audio/pcm", "rate": SAMPLE_RATE_HZ},
                            "turn_detection": {
                                "type": "server_vad",
                                "interrupt_response": false
                            }
                        },
                        "output": {
                            "format": {"type": "audio/pcm", "rate": SAMPLE_RATE_HZ},
                            "voice": "marin"
                        }
                    }
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|_| RealtimeError::Transport)?;
    Ok(Connection {
        socket,
        state: ConnectionState::default(),
        connected_pending: true,
    })
}

impl Connection {
    async fn exchange(
        &mut self,
        input: [u8; FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<RealtimeExchange, RealtimeError> {
        self.state
            .observe_playback_completion(playback_completed_frames)?;
        self.socket
            .send(Message::Text(
                json!({
                    "type": "input_audio_buffer.append",
                    "audio": BASE64.encode(input),
                })
                .to_string()
                .into(),
            ))
            .await
            .map_err(|_| RealtimeError::Transport)?;
        let mut events = Vec::new();
        if std::mem::take(&mut self.connected_pending) {
            events.push(RealtimeEvent::Connected);
            events.push(RealtimeEvent::Listening);
        }
        loop {
            match tokio::time::timeout(RECEIVE_SLICE, self.socket.next()).await {
                Err(_) => break,
                Ok(Some(Ok(message))) => {
                    let commands = self.state.consume(message, &mut events)?;
                    for command in commands {
                        self.socket
                            .send(Message::Text(command.to_string().into()))
                            .await
                            .map_err(|_| RealtimeError::Transport)?;
                    }
                }
                Ok(Some(Err(_))) | Ok(None) => return Err(RealtimeError::Transport),
            }
            if !self.state.output_frames.is_empty() {
                break;
            }
        }
        let output = self.state.take_output();
        Ok(RealtimeExchange { output, events })
    }
}

impl ConnectionState {
    fn consume(
        &mut self,
        message: Message,
        events: &mut Vec<RealtimeEvent>,
    ) -> Result<Vec<Value>, RealtimeError> {
        let Message::Text(text) = message else {
            return Ok(Vec::new());
        };
        let value: Value = serde_json::from_str(&text).map_err(|_| RealtimeError::Protocol)?;
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "input_audio_buffer.speech_started" => {
                events.push(RealtimeEvent::Listening);
                let response_was_active = std::mem::take(&mut self.response_active);
                let playback = self.playback.take();
                let played_frames = playback
                    .as_ref()
                    .map_or(0, |playback| self.played_frames(playback));
                let has_unplayed_audio = playback
                    .as_ref()
                    .is_some_and(|playback| self.has_unplayed_audio(playback));
                let should_truncate =
                    playback.is_some() && (response_was_active || has_unplayed_audio);
                self.output_frames.clear();
                self.output_residual.clear();
                if has_unplayed_audio {
                    events.push(RealtimeEvent::PlaybackInterrupted);
                    self.discarded_output_frames = self.discarded_output_frames.saturating_add(
                        playback
                            .as_ref()
                            .map_or(0, |playback| playback.output_frames_sent)
                            .saturating_sub(played_frames),
                    );
                }
                let mut commands = Vec::new();
                if response_was_active {
                    commands.push(json!({"type": "response.cancel"}));
                }
                if should_truncate {
                    let playback = playback.expect("truncate requires playback");
                    commands.push(json!({
                        "type": "conversation.item.truncate",
                        "item_id": playback.item_id,
                        "content_index": playback.content_index,
                        "audio_end_ms": played_frames.saturating_mul(20),
                    }));
                }
                return Ok(commands);
            }
            "response.created" => {
                self.response_active = true;
                events.push(RealtimeEvent::Speaking);
            }
            "response.output_audio.delta" => {
                let delta = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or(RealtimeError::Protocol)?;
                let item_id = value
                    .get("item_id")
                    .and_then(Value::as_str)
                    .ok_or(RealtimeError::Protocol)?;
                let content_index = value
                    .get("content_index")
                    .and_then(Value::as_u64)
                    .ok_or(RealtimeError::Protocol)?;
                let bytes = BASE64.decode(delta).map_err(|_| RealtimeError::Protocol)?;
                self.append_output(item_id, content_index, &bytes)?;
            }
            "response.output_audio_transcript.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str)
                    && !delta.is_empty()
                {
                    events.push(RealtimeEvent::TranscriptDelta(delta.to_string()));
                }
            }
            "response.done" => {
                let status = value
                    .get("response")
                    .and_then(|response| response.get("status"))
                    .and_then(Value::as_str)
                    .ok_or(RealtimeError::ResponseFailed)?;
                match status {
                    "completed" => {
                        self.response_active = false;
                        events.push(RealtimeEvent::ResponseCompleted);
                        events.push(RealtimeEvent::Listening);
                    }
                    "cancelled" => {
                        self.response_active = false;
                        events.push(RealtimeEvent::Listening);
                    }
                    "failed" | "incomplete" => return Err(RealtimeError::ResponseFailed),
                    _ => return Err(RealtimeError::ResponseFailed),
                }
            }
            "error" => return Err(RealtimeError::Protocol),
            _ => {}
        }
        Ok(Vec::new())
    }

    fn append_output(
        &mut self,
        item_id: &str,
        content_index: u64,
        bytes: &[u8],
    ) -> Result<(), RealtimeError> {
        match self.playback.as_ref() {
            Some(playback)
                if playback.item_id != item_id || playback.content_index != content_index =>
            {
                if self.has_unplayed_audio(playback) {
                    return Err(RealtimeError::Protocol);
                }
                self.playback = Some(PlaybackCursor {
                    item_id: item_id.to_string(),
                    content_index,
                    first_output_ordinal: None,
                    output_frames_sent: 0,
                });
            }
            None => {
                self.playback = Some(PlaybackCursor {
                    item_id: item_id.to_string(),
                    content_index,
                    first_output_ordinal: None,
                    output_frames_sent: 0,
                });
            }
            Some(_) => {}
        }
        self.output_residual.extend_from_slice(bytes);
        while self.output_residual.len() >= FRAME_BYTES {
            if self.output_frames.len() == MAX_OUTPUT_FRAMES {
                return Err(RealtimeError::Backpressure);
            }
            let frame: [u8; FRAME_BYTES] = self.output_residual[..FRAME_BYTES]
                .try_into()
                .map_err(|_| RealtimeError::Protocol)?;
            self.output_residual.drain(..FRAME_BYTES);
            self.output_frames.push_back(frame);
        }
        Ok(())
    }

    fn observe_playback_completion(&mut self, completed_frames: u64) -> Result<(), RealtimeError> {
        let effective_completed_frames = completed_frames
            .checked_add(self.discarded_output_frames)
            .ok_or(RealtimeError::Protocol)?;
        if completed_frames < self.playback_completed_frames
            || effective_completed_frames > self.output_frame_ordinal
        {
            return Err(RealtimeError::Protocol);
        }
        self.playback_completed_frames = completed_frames;
        Ok(())
    }

    fn take_output(&mut self) -> Option<[u8; FRAME_BYTES]> {
        let output = self.output_frames.pop_front()?;
        let playback = self
            .playback
            .as_mut()
            .expect("queued output has a playback cursor");
        playback
            .first_output_ordinal
            .get_or_insert(self.output_frame_ordinal);
        playback.output_frames_sent = playback.output_frames_sent.saturating_add(1);
        self.output_frame_ordinal = self.output_frame_ordinal.saturating_add(1);
        Some(output)
    }

    fn played_frames(&self, playback: &PlaybackCursor) -> u64 {
        let Some(first_output_ordinal) = playback.first_output_ordinal else {
            return 0;
        };
        self.playback_completed_frames
            .saturating_add(self.discarded_output_frames)
            .saturating_sub(first_output_ordinal)
            .min(playback.output_frames_sent)
    }

    fn has_unplayed_audio(&self, playback: &PlaybackCursor) -> bool {
        !self.output_frames.is_empty()
            || !self.output_residual.is_empty()
            || self.played_frames(playback) < playback.output_frames_sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_realtime_model_is_eligible() {
        let realtime = AgentRuntimeModelId::new("gpt-realtime-2.1").expect("model id");
        let chat = AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id");
        assert_eq!(
            media_capabilities(&realtime)
                .expect("realtime capabilities")
                .voice_input,
            AgentRuntimeMediaCapability::Supported
        );
        assert!(media_capabilities(&chat).is_none());
    }

    #[test]
    fn output_audio_is_reframed_to_exact_twenty_millisecond_packets() {
        let mut state = ConnectionState::default();
        state
            .append_output("item-current", 0, &vec![7; FRAME_BYTES * 2 + 17])
            .expect("append");
        assert_eq!(state.output_frames.len(), 2);
        assert_eq!(state.output_residual.len(), 17);
        assert!(
            state
                .output_frames
                .iter()
                .all(|frame| frame.len() == FRAME_BYTES)
        );
    }

    #[test]
    fn current_audio_events_drive_output_and_unrelated_names_are_ignored() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        let delta = BASE64.encode([9; FRAME_BYTES]);
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.output_audio.delta",
                        "item_id": "item-current",
                        "content_index": 0,
                        "delta": delta,
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("current event");
        state
            .consume(
                Message::Text(
                    json!({"type": "response.output_text.delta", "delta": "ignored"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("unknown old event is inert");
        assert_eq!(state.output_frames.len(), 1);
    }

    #[test]
    fn queued_but_unplayed_audio_truncates_at_confirmed_zero_boundary() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(json!({"type": "response.created"}).to_string().into()),
                &mut events,
            )
            .expect("response created");
        state
            .append_output("item-current", 2, &vec![3; FRAME_BYTES * 2 + 17])
            .expect("append");
        assert!(state.take_output().is_some());
        state
            .observe_playback_completion(0)
            .expect("no confirmed playback");
        events.clear();
        let commands = state
            .consume(
                Message::Text(
                    json!({"type": "input_audio_buffer.speech_started"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("speech start");
        assert!(state.output_frames.is_empty());
        assert!(state.output_residual.is_empty());
        assert!(events.contains(&RealtimeEvent::PlaybackInterrupted));
        assert_eq!(commands[0], json!({"type": "response.cancel"}));
        assert_eq!(
            commands[1],
            json!({
                "type": "conversation.item.truncate",
                "item_id": "item-current",
                "content_index": 2,
                "audio_end_ms": 0,
            })
        );
    }

    #[test]
    fn completed_response_with_unplayed_audio_truncates_without_response_cancel() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(json!({"type": "response.created"}).to_string().into()),
                &mut events,
            )
            .expect("response created");
        state
            .append_output("item-completed", 0, &vec![4; FRAME_BYTES * 2])
            .expect("append");
        assert!(state.take_output().is_some());
        state
            .observe_playback_completion(0)
            .expect("no confirmed playback");
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.done",
                        "response": {"status": "completed"},
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("response completed");
        events.clear();
        let commands = state
            .consume(
                Message::Text(
                    json!({"type": "input_audio_buffer.speech_started"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("speech start");
        assert_eq!(
            commands,
            vec![json!({
                "type": "conversation.item.truncate",
                "item_id": "item-completed",
                "content_index": 0,
                "audio_end_ms": 0,
            })]
        );
        assert!(events.contains(&RealtimeEvent::PlaybackInterrupted));
    }

    #[test]
    fn completed_native_playback_needs_no_cancel_or_truncate() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(json!({"type": "response.created"}).to_string().into()),
                &mut events,
            )
            .expect("response created");
        state
            .append_output("item-played", 0, &[5; FRAME_BYTES])
            .expect("append");
        assert!(state.take_output().is_some());
        state
            .observe_playback_completion(1)
            .expect("confirmed playback");
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.done",
                        "response": {"status": "completed"},
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("response completed");
        events.clear();
        let commands = state
            .consume(
                Message::Text(
                    json!({"type": "input_audio_buffer.speech_started"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("speech start");
        assert!(commands.is_empty());
        assert_eq!(events, vec![RealtimeEvent::Listening]);
    }

    #[test]
    fn discarded_output_preserves_next_response_playback_progress() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(json!({"type": "response.created"}).to_string().into()),
                &mut events,
            )
            .expect("first response created");
        state
            .append_output("item-interrupted", 0, &[6; FRAME_BYTES])
            .expect("append interrupted output");
        assert!(state.take_output().is_some());
        state
            .observe_playback_completion(0)
            .expect("interrupted output was not played");
        state
            .consume(
                Message::Text(
                    json!({"type": "input_audio_buffer.speech_started"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("interrupt first response");

        state
            .consume(
                Message::Text(json!({"type": "response.created"}).to_string().into()),
                &mut events,
            )
            .expect("second response created");
        state
            .append_output("item-played", 0, &[7; FRAME_BYTES])
            .expect("append played output");
        assert!(state.take_output().is_some());
        state
            .observe_playback_completion(1)
            .expect("second response playback confirmed");
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.done",
                        "response": {"status": "completed"},
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("second response completed");
        events.clear();
        let commands = state
            .consume(
                Message::Text(
                    json!({"type": "input_audio_buffer.speech_started"})
                        .to_string()
                        .into(),
                ),
                &mut events,
            )
            .expect("speech after completed playback");

        assert!(commands.is_empty());
        assert_eq!(events, vec![RealtimeEvent::Listening]);
    }

    #[test]
    fn response_done_completes_only_for_explicit_completed_status() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.done",
                        "response": {"status": "completed"},
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("completed response");
        assert_eq!(
            events,
            vec![RealtimeEvent::ResponseCompleted, RealtimeEvent::Listening]
        );
    }

    #[test]
    fn cancelled_response_done_returns_to_listening_without_completion() {
        let mut state = ConnectionState::default();
        let mut events = Vec::new();
        state
            .consume(
                Message::Text(
                    json!({
                        "type": "response.done",
                        "response": {"status": "cancelled"},
                    })
                    .to_string()
                    .into(),
                ),
                &mut events,
            )
            .expect("cancelled response");
        assert_eq!(events, vec![RealtimeEvent::Listening]);
    }

    #[test]
    fn unsuccessful_or_invalid_response_done_uses_one_sanitized_failure() {
        let cases = [
            json!({"type": "response.done", "response": {"status": "failed"}}),
            json!({"type": "response.done", "response": {"status": "incomplete"}}),
            json!({"type": "response.done", "response": {}}),
            json!({"type": "response.done", "response": {"status": 7}}),
            json!({"type": "response.done", "response": {"status": "unknown"}}),
        ];
        for value in cases {
            let mut state = ConnectionState::default();
            let mut events = Vec::new();
            assert_eq!(
                state.consume(Message::Text(value.to_string().into()), &mut events),
                Err(RealtimeError::ResponseFailed)
            );
            assert!(events.is_empty());
        }
    }
}
