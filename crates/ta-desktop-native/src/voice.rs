//! Private composition of the safe AVFoundation boundary and Rust daemon stream.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use ta_daemon_client::VoiceStream;
use ta_macos_avfoundation::{
    AudioError, PcmFrame, PermissionStatus, TerminalSignal, VoiceBoundary, VoiceStopHandle,
};
use ta_protocol::wire::{RunId, VoiceEvent, VoicePermissionState, VoiceStreamEndReason};

use crate::bridge::NativeJsonCallback;

pub(crate) struct VoiceOwner {
    active: Mutex<Option<NativeVoiceSession>>,
    listener: Mutex<Option<Arc<NativeJsonCallback>>>,
}

struct NativeVoiceSession {
    run_id: RunId,
    stop: VoiceStopHandle,
    stream_end: Arc<VoiceStreamEnd>,
}

struct VoiceStreamEnd {
    stream: VoiceStream,
    ended: EndOnce,
}

#[derive(Default)]
struct EndOnce(AtomicBool);

impl EndOnce {
    fn claim(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
}

impl VoiceStreamEnd {
    fn new(stream: VoiceStream) -> Self {
        Self {
            stream,
            ended: EndOnce::default(),
        }
    }

    fn end_once(&self, reason: VoiceStreamEndReason) {
        if self.ended.claim() {
            let _ = self.stream.end(reason);
        }
    }
}

impl VoiceOwner {
    pub(crate) fn new() -> Self {
        Self {
            active: Mutex::new(None),
            listener: Mutex::new(None),
        }
    }

    pub(crate) fn set_listener(&self, listener: Arc<NativeJsonCallback>) {
        *self.listener.lock().expect("voice listener lock poisoned") = Some(listener);
    }

    pub(crate) fn start(&self, stream: VoiceStream, initial: VoiceEvent) -> Result<(), AudioError> {
        let mut active = self.active.lock().expect("voice owner lock poisoned");
        if active
            .as_ref()
            .is_some_and(|current| current.run_id == *stream.run_id())
        {
            return Ok(());
        }
        if let Some(previous) = active.take() {
            previous.stream_end.end_once(VoiceStreamEndReason::Replaced);
            previous.stop.stop();
        }
        let boundary = match VoiceBoundary::start() {
            Ok(boundary) => boundary,
            Err(error) => {
                let _ = stream.end(VoiceStreamEndReason::DeviceUnavailable);
                return Err(error);
            }
        };
        let stop = boundary.stop_handle();
        let listener = self
            .listener
            .lock()
            .expect("voice listener lock poisoned")
            .clone();
        deliver_state(listener.as_ref(), &initial);
        let run_id = stream.run_id().clone();
        let stream_end = Arc::new(VoiceStreamEnd::new(stream));
        let worker_stream_end = Arc::clone(&stream_end);
        if thread::Builder::new()
            .name(format!("taugentic-native-voice-{}", run_id.as_str()))
            .spawn(move || run_voice(boundary, worker_stream_end, listener))
            .is_err()
        {
            stream_end.end_once(VoiceStreamEndReason::DeviceUnavailable);
            return Err(AudioError::PlatformUnavailable);
        }
        *active = Some(NativeVoiceSession {
            run_id,
            stop,
            stream_end,
        });
        Ok(())
    }

    pub(crate) fn stop(&self) {
        if let Some(active) = self
            .active
            .lock()
            .expect("voice owner lock poisoned")
            .take()
        {
            active.stream_end.end_once(VoiceStreamEndReason::Shutdown);
            active.stop.stop();
        }
    }

    pub(crate) fn stop_run(&self, run_id: &RunId) {
        let mut active = self.active.lock().expect("voice owner lock poisoned");
        if active
            .as_ref()
            .is_some_and(|active| active.run_id == *run_id)
            && let Some(active) = active.take()
        {
            active.stop.stop();
        }
    }
}

fn run_voice(
    mut boundary: VoiceBoundary,
    stream_end: Arc<VoiceStreamEnd>,
    listener: Option<Arc<NativeJsonCallback>>,
) {
    loop {
        let input = match boundary.wait_for_capture() {
            Ok(frame) => frame,
            Err(AudioError::Stopped) => return,
            Err(error) => {
                stream_end.end_once(end_reason(&error));
                return;
            }
        };
        let Some(exchange) = exchange_or_end(
            stream_end
                .stream
                .exchange(*input.as_le_bytes(), boundary.playback_completed_frames()),
            || stream_end.end_once(VoiceStreamEndReason::DeviceUnavailable),
        ) else {
            return;
        };
        if exchange.playback_interrupted
            && let Err(error) = boundary.clear_playback()
        {
            stream_end.end_once(end_reason(&error));
            return;
        }
        if let Some(state) = exchange.state {
            deliver_state(listener.as_ref(), &state);
        }
        if let Some(output) = exchange.output
            && let Err(error) = boundary.enqueue_playback(PcmFrame::from_le_bytes(output))
        {
            stream_end.end_once(end_reason(&error));
            return;
        }
    }
}

fn exchange_or_end<T, E>(result: Result<T, E>, end: impl FnOnce()) -> Option<T> {
    match result {
        Ok(exchange) => Some(exchange),
        Err(_) => {
            end();
            None
        }
    }
}

fn deliver_state(listener: Option<&Arc<NativeJsonCallback>>, state: &VoiceEvent) {
    let Some(listener) = listener else { return };
    let Ok(value) = serde_json::to_string(state) else {
        return;
    };
    let _ = listener.call(value, ThreadsafeFunctionCallMode::NonBlocking);
}

fn end_reason(error: &AudioError) -> VoiceStreamEndReason {
    match error {
        AudioError::AlreadyTerminated(TerminalSignal::CaptureOverflow) => {
            VoiceStreamEndReason::CaptureOverflow
        }
        AudioError::AlreadyTerminated(TerminalSignal::PlaybackOverflow) => {
            VoiceStreamEndReason::PlaybackOverflow
        }
        AudioError::AlreadyTerminated(TerminalSignal::Interrupted) => {
            VoiceStreamEndReason::Interrupted
        }
        AudioError::InvalidFrameLength { .. }
        | AudioError::PlatformUnavailable
        | AudioError::EngineStartFailed
        | AudioError::Stopped => VoiceStreamEndReason::DeviceUnavailable,
    }
}

pub(crate) fn permission_state() -> VoicePermissionState {
    permission_state_from_native(VoiceBoundary::permission_status())
}

pub(crate) fn request_permission(callback: Arc<NativeJsonCallback>) {
    VoiceBoundary::request_permission(move |status| {
        let Ok(value) = serde_json::to_string(&permission_state_from_native(status)) else {
            return;
        };
        let _ = callback.call(value, ThreadsafeFunctionCallMode::NonBlocking);
    });
}

fn permission_state_from_native(status: PermissionStatus) -> VoicePermissionState {
    match status {
        PermissionStatus::NotDetermined => VoicePermissionState::NotDetermined,
        PermissionStatus::Denied => VoicePermissionState::Denied,
        PermissionStatus::Restricted => VoicePermissionState::Restricted,
        PermissionStatus::Authorized => VoicePermissionState::Authorized,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{EndOnce, exchange_or_end};

    #[test]
    fn exchange_failure_claims_the_terminal_end_before_exit() {
        let gate = EndOnce::default();
        let terminal_ends = AtomicUsize::new(0);
        let exchange: Result<(), ()> = Err(());

        assert!(
            exchange_or_end(exchange, || {
                if gate.claim() {
                    terminal_ends.fetch_add(1, Ordering::SeqCst);
                }
            })
            .is_none()
        );
        if gate.claim() {
            terminal_ends.fetch_add(1, Ordering::SeqCst);
        }

        assert_eq!(terminal_ends.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn replacement_shutdown_and_worker_races_have_one_stream_end_owner() {
        let gate = Arc::new(EndOnce::default());
        let claims = (0..8)
            .map(|_| {
                let gate = Arc::clone(&gate);
                std::thread::spawn(move || gate.claim())
            })
            .map(|worker| worker.join().expect("claim worker"))
            .filter(|claimed| *claimed)
            .count();
        assert_eq!(claims, 1);
    }
}
