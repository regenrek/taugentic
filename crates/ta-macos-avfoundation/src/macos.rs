//! Safe, bounded macOS microphone and PCM playback mechanics.
//!
//! This is the only file in the workspace allowed to cross the Objective-C FFI
//! boundary for voice PCM. The public types deliberately expose owned frames,
//! never Objective-C objects or borrowed PCM pointers.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
};

pub const SAMPLE_RATE_HZ: u32 = 24_000;
pub const CHANNELS: u16 = 1;
pub const BITS_PER_SAMPLE: u16 = 16;
pub const FRAME_DURATION_MS: u16 = 20;
pub const FRAME_SAMPLES: usize = 480;
pub const FRAME_BYTES: usize = 960;
pub const CAPTURE_QUEUE_CAPACITY: usize = 4;
pub const PLAYBACK_QUEUE_CAPACITY: usize = 50;
pub const PLAYBACK_QUEUE_MAX_BYTES: usize = 48_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionStatus {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalSignal {
    CaptureOverflow,
    PlaybackOverflow,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioError {
    InvalidFrameLength { actual: usize },
    AlreadyTerminated(TerminalSignal),
    PlatformUnavailable,
    EngineStartFailed,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcmFrame([u8; FRAME_BYTES]);

impl PcmFrame {
    pub fn from_le_bytes(bytes: [u8; FRAME_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, AudioError> {
        let actual = bytes.len();
        let frame = bytes
            .try_into()
            .map_err(|_| AudioError::InvalidFrameLength { actual })?;
        Ok(Self(frame))
    }

    pub fn as_le_bytes(&self) -> &[u8; FRAME_BYTES] {
        &self.0
    }
}

#[derive(Debug, Default)]
struct DirectionalQueue {
    frames: VecDeque<PcmFrame>,
    bytes: usize,
}

#[derive(Debug)]
struct PlaybackTicket {
    id: u64,
}

#[derive(Debug, Default)]
struct PlaybackLedger {
    tickets: VecDeque<PlaybackTicket>,
    bytes: usize,
    next_ticket: u64,
    completed: u64,
}

impl PlaybackLedger {
    fn reserve(&mut self) -> Result<u64, ()> {
        if self.tickets.len() == PLAYBACK_QUEUE_CAPACITY
            || self.bytes + FRAME_BYTES > PLAYBACK_QUEUE_MAX_BYTES
        {
            return Err(());
        }
        let ticket = self.next_ticket;
        self.next_ticket = self.next_ticket.wrapping_add(1);
        self.tickets.push_back(PlaybackTicket { id: ticket });
        self.bytes += FRAME_BYTES;
        Ok(ticket)
    }

    fn retire(&mut self, ticket: u64) -> bool {
        let Some(index) = self.tickets.iter().position(|pending| pending.id == ticket) else {
            return false;
        };
        self.tickets.remove(index);
        self.bytes -= FRAME_BYTES;
        self.completed += 1;
        true
    }

    fn rollback(&mut self, ticket: u64) {
        if let Some(index) = self.tickets.iter().position(|pending| pending.id == ticket) {
            self.tickets.remove(index);
            self.bytes -= FRAME_BYTES;
        }
    }

    fn clear(&mut self) {
        self.tickets.clear();
        self.bytes = 0;
    }
}

#[derive(Debug, Default)]
struct ResidualAssembler {
    bytes: Vec<u8>,
}

impl ResidualAssembler {
    fn append(
        &mut self,
        source: &[u8],
        mut emit: impl FnMut(PcmFrame) -> Result<(), ()>,
    ) -> Result<(), ()> {
        let mut remaining = source;
        if !self.bytes.is_empty() {
            let needed = FRAME_BYTES - self.bytes.len();
            let take = needed.min(remaining.len());
            self.bytes.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            if self.bytes.len() == FRAME_BYTES {
                emit(PcmFrame::try_from_slice(&self.bytes).expect("residual has one exact frame"))?;
                self.bytes.clear();
            }
        }
        while remaining.len() >= FRAME_BYTES {
            let (frame_bytes, rest) = remaining.split_at(FRAME_BYTES);
            emit(PcmFrame::try_from_slice(frame_bytes).expect("split has one exact frame"))?;
            remaining = rest;
        }
        self.bytes.extend_from_slice(remaining);
        debug_assert!(self.bytes.len() < FRAME_BYTES);
        Ok(())
    }

    fn clear(&mut self) {
        self.bytes.clear();
    }
}

impl DirectionalQueue {
    fn push(&mut self, frame: PcmFrame, capacity: usize, max_bytes: usize) -> Result<(), ()> {
        if self.frames.len() == capacity || self.bytes + FRAME_BYTES > max_bytes {
            return Err(());
        }
        self.bytes += FRAME_BYTES;
        self.frames.push_back(frame);
        Ok(())
    }

    fn pop(&mut self) -> Option<PcmFrame> {
        let frame = self.frames.pop_front()?;
        self.bytes -= FRAME_BYTES;
        Some(frame)
    }

    fn clear(&mut self) {
        self.frames.clear();
        self.bytes = 0;
    }
}

/// The sole owner of bounded, memory-only capture and playback PCM queues.
///
/// On macOS, all Objective-C graph mutation is serialized on the main thread.
/// Permission, tap, and notification callbacks may run on arbitrary queues, so
/// they only lock this Rust-owned state after producing an owned frame or terminal
/// signal; they never mutate the Objective-C graph or release graph objects.
#[derive(Default)]
pub struct VoiceBoundary {
    state: Arc<VoiceStateOwner>,
    #[cfg(target_os = "macos")]
    platform: Option<PlatformAudioHandle>,
}

#[derive(Clone)]
pub struct VoiceStopHandle {
    state: Arc<VoiceStateOwner>,
}

impl VoiceStopHandle {
    pub fn stop(&self) {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        state.stopped = true;
        state.capture.clear();
        state.playback.clear();
        state.residual.clear();
        drop(state);
        self.state.changed.notify_all();
    }
}

#[derive(Debug, Default)]
struct VoiceState {
    capture: DirectionalQueue,
    playback: PlaybackLedger,
    residual: ResidualAssembler,
    terminal: Option<TerminalSignal>,
    stopped: bool,
    captured_frames: u64,
}

#[derive(Debug, Default)]
struct VoiceStateOwner {
    state: Mutex<VoiceState>,
    changed: Condvar,
}

impl VoiceState {
    fn terminate(&mut self, signal: TerminalSignal) {
        if self.terminal.is_none() {
            self.terminal = Some(signal);
            self.capture.clear();
            self.playback.clear();
            self.residual.clear();
        }
    }

    fn append_capture(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.residual.append(bytes, |frame| {
            self.capture
                .push(
                    frame,
                    CAPTURE_QUEUE_CAPACITY,
                    CAPTURE_QUEUE_CAPACITY * FRAME_BYTES,
                )
                .map_err(|_| ())?;
            self.captured_frames += 1;
            Ok(())
        })
    }
}

impl VoiceBoundary {
    pub fn permission_status() -> PermissionStatus {
        platform_permission_status()
    }

    /// Requests microphone access exactly once for this call and invokes the callback
    /// when AVFoundation resolves the system prompt.
    pub fn request_permission(callback: impl FnOnce(PermissionStatus) + 'static) {
        platform_request_permission(callback);
    }

    pub fn start() -> Result<Self, AudioError> {
        let state = Arc::default();
        Ok(Self {
            state: Arc::clone(&state),
            #[cfg(target_os = "macos")]
            platform: Some(start_platform_audio(state)?),
        })
    }

    pub fn stop_handle(&self) -> VoiceStopHandle {
        VoiceStopHandle {
            state: Arc::clone(&self.state),
        }
    }

    #[cfg(test)]
    fn enqueue_capture(&self, frame: PcmFrame) -> Result<(), AudioError> {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        if state.stopped {
            return Err(AudioError::Stopped);
        }
        if let Some(signal) = state.terminal {
            return Err(AudioError::AlreadyTerminated(signal));
        }
        if state.append_capture(frame.as_le_bytes()).is_err() {
            state.terminate(TerminalSignal::CaptureOverflow);
            self.state.changed.notify_all();
            return Err(AudioError::AlreadyTerminated(
                TerminalSignal::CaptureOverflow,
            ));
        }
        self.state.changed.notify_all();
        Ok(())
    }

    pub fn dequeue_capture(&mut self) -> Option<PcmFrame> {
        self.state
            .state
            .lock()
            .expect("voice state is not poisoned")
            .capture
            .pop()
    }

    pub fn enqueue_playback(&mut self, frame: PcmFrame) -> Result<(), AudioError> {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        if state.stopped {
            return Err(AudioError::Stopped);
        }
        if let Some(signal) = state.terminal {
            return Err(AudioError::AlreadyTerminated(signal));
        }
        let ticket = match state.playback.reserve() {
            Ok(ticket) => ticket,
            Err(()) => {
                state.terminate(TerminalSignal::PlaybackOverflow);
                self.state.changed.notify_all();
                return Err(AudioError::AlreadyTerminated(
                    TerminalSignal::PlaybackOverflow,
                ));
            }
        };
        drop(state);
        #[cfg(target_os = "macos")]
        if let Some(platform) = self.platform
            && let Err(error) = schedule_platform_audio(platform, frame.clone(), ticket)
        {
            self.state
                .state
                .lock()
                .expect("voice state is not poisoned")
                .playback
                .rollback(ticket);
            self.state.changed.notify_all();
            return Err(error);
        }
        Ok(())
    }

    pub fn pending_playback_count(&self) -> usize {
        self.state
            .state
            .lock()
            .expect("voice state is not poisoned")
            .playback
            .tickets
            .len()
    }

    /// Monotonic count of frames confirmed played by AVFoundation.
    pub fn playback_completed_frames(&self) -> u64 {
        self.state
            .state
            .lock()
            .expect("voice state is not poisoned")
            .playback
            .completed
    }

    /// Drops every queued or scheduled output frame while capture keeps running.
    pub fn clear_playback(&mut self) -> Result<(), AudioError> {
        {
            let mut state = self
                .state
                .state
                .lock()
                .expect("voice state is not poisoned");
            if state.stopped {
                return Err(AudioError::Stopped);
            }
            if let Some(signal) = state.terminal {
                return Err(AudioError::AlreadyTerminated(signal));
            }
            state.playback.clear();
        }
        self.state.changed.notify_all();
        #[cfg(target_os = "macos")]
        if let Some(platform) = self.platform {
            clear_platform_playback(platform)?;
        }
        Ok(())
    }

    pub fn interrupt(&mut self) {
        self.state
            .state
            .lock()
            .expect("voice state is not poisoned")
            .terminate(TerminalSignal::Interrupted);
    }

    pub fn terminal_signal(&self) -> Option<TerminalSignal> {
        self.state
            .state
            .lock()
            .expect("voice state is not poisoned")
            .terminal
    }

    pub fn stop(&mut self) {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        if state.stopped {
            return;
        }
        state.stopped = true;
        state.capture.clear();
        state.playback.clear();
        state.residual.clear();
        drop(state);
        self.state.changed.notify_all();
        #[cfg(target_os = "macos")]
        if let Some(platform) = self.platform.take() {
            stop_platform_audio(platform);
        }
    }

    pub fn wait_for_capture(&mut self) -> Result<PcmFrame, AudioError> {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        loop {
            if let Some(frame) = state.capture.pop() {
                return Ok(frame);
            }
            if state.stopped {
                return Err(AudioError::Stopped);
            }
            if let Some(signal) = state.terminal {
                return Err(AudioError::AlreadyTerminated(signal));
            }
            state = self
                .state
                .changed
                .wait(state)
                .expect("voice state is not poisoned");
        }
    }

    #[cfg(feature = "hardware-acceptance")]
    pub fn wait_for_playback_settled(&self) -> Result<(), AudioError> {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        loop {
            if state.playback.tickets.is_empty() && state.playback.completed > 0 {
                return Ok(());
            }
            if state.stopped {
                return Err(AudioError::Stopped);
            }
            if let Some(signal) = state.terminal {
                return Err(AudioError::AlreadyTerminated(signal));
            }
            state = self
                .state
                .changed
                .wait(state)
                .expect("voice state is not poisoned");
        }
    }

    #[cfg(feature = "hardware-acceptance")]
    pub fn wait_for_interruption(&self) -> Result<(), AudioError> {
        let mut state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        loop {
            if state.terminal == Some(TerminalSignal::Interrupted) {
                return Ok(());
            }
            if state.stopped {
                return Err(AudioError::Stopped);
            }
            if let Some(signal) = state.terminal {
                return Err(AudioError::AlreadyTerminated(signal));
            }
            state = self
                .state
                .changed
                .wait(state)
                .expect("voice state is not poisoned");
        }
    }

    #[cfg(feature = "hardware-acceptance")]
    pub fn hardware_snapshot(&self) -> (u64, u64, bool) {
        let state = self
            .state
            .state
            .lock()
            .expect("voice state is not poisoned");
        (
            state.captured_frames,
            state.playback.completed,
            state.stopped,
        )
    }

    #[cfg(all(target_os = "macos", feature = "hardware-acceptance"))]
    pub fn pause_capture_for_hardware_acceptance(&mut self) {
        if let Some(platform) = self.platform {
            pause_platform_audio(platform);
        }
    }
}

impl Drop for VoiceBoundary {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_permission_status() -> PermissionStatus {
    PermissionStatus::Restricted
}

#[cfg(not(target_os = "macos"))]
fn platform_request_permission(callback: impl FnOnce(PermissionStatus) + 'static) {
    callback(PermissionStatus::Restricted);
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_avf_audio::{
        AVAudioBuffer, AVAudioCommonFormat, AVAudioConverter, AVAudioConverterInputBlock,
        AVAudioConverterInputStatus, AVAudioEngine, AVAudioEngineConfigurationChangeNotification,
        AVAudioFormat, AVAudioNodeTapBlock, AVAudioPCMBuffer, AVAudioPlayerNode,
        AVAudioPlayerNodeCompletionCallbackType, AVAudioPlayerNodeCompletionHandler, AVAudioTime,
    };
    use objc2_foundation::{NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
    use std::{
        cell::RefCell,
        collections::BTreeMap,
        marker::PhantomData,
        ptr::NonNull,
        rc::Rc,
        sync::{
            Mutex as StdMutex,
            atomic::{AtomicU64, Ordering},
        },
    };

    pub(super) type PlatformAudioHandle = u64;
    static NEXT_PLATFORM_AUDIO_HANDLE: AtomicU64 = AtomicU64::new(1);
    thread_local! {
        static PLATFORM_AUDIO: RefCell<BTreeMap<PlatformAudioHandle, PlatformAudio>> =
            const { RefCell::new(BTreeMap::new()) };
    }

    pub(super) struct PlatformAudio {
        engine: Retained<AVAudioEngine>,
        player: Retained<AVAudioPlayerNode>,
        format: Retained<AVAudioFormat>,
        input: Retained<objc2_avf_audio::AVAudioInputNode>,
        observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
        notification_center: Retained<NSNotificationCenter>,
        state: Arc<VoiceStateOwner>,
        tap_installed: bool,
        observer_installed: bool,
        stopped: bool,
        _main_thread_only: PhantomData<Rc<()>>,
    }

    impl PlatformAudio {
        pub(super) fn start(state: Arc<VoiceStateOwner>) -> Result<Self, AudioError> {
            let _marker = MainThreadMarker::new().ok_or(AudioError::PlatformUnavailable)?;
            // SAFETY: `init` is the Objective-C `-[AVAudioEngine init]` selector. The
            // retained object is owned by `engine` for this Rust value's lifetime; no raw
            // pointers are passed; this boundary is main-thread-only (enforced by marker).
            let engine = unsafe { AVAudioEngine::init(AVAudioEngine::alloc()) };
            // SAFETY: `initWithCommonFormat:sampleRate:channels:interleaved:` retains the
            // returned format in `format`; no borrowed block or pointer escapes; construction
            // occurs on the main thread required for this AVFoundation graph.
            let format = unsafe {
                AVAudioFormat::initWithCommonFormat_sampleRate_channels_interleaved(
                    AVAudioFormat::alloc(),
                    AVAudioCommonFormat::PCMFormatInt16,
                    f64::from(SAMPLE_RATE_HZ),
                    u32::from(CHANNELS),
                    true,
                )
            }
            .ok_or(AudioError::EngineStartFailed)?;
            // SAFETY: `init` is the Objective-C `-[AVAudioPlayerNode init]` selector. The
            // retained node is kept by `player`; no pointers or blocks escape; graph setup is
            // confined to the main thread by `marker`.
            let player = unsafe { AVAudioPlayerNode::init(AVAudioPlayerNode::alloc()) };
            // SAFETY: `attachNode:` retains graph ownership while Rust keeps `player` alive;
            // there are no pointer bounds or blocks; AVFoundation graph mutation is main-thread-only.
            unsafe { engine.attachNode(&player) };
            // SAFETY: `connect:to:format:` borrows retained nodes only for the call; `format`
            // remains retained by this object; no raw PCM pointer is involved; main-thread-only.
            unsafe { engine.connect_to_format(&player, &engine.mainMixerNode(), Some(&format)) };
            // SAFETY: `inputNode` returns a retained node belonging to the retained engine; it
            // is held in `input` until its tap is removed during stop/drop. No raw PCM pointer
            // or block is used here. The graph's sole mutation owner is this main-thread call.
            let input = unsafe { engine.inputNode() };
            // SAFETY: `outputFormatForBus:` reads bus zero from the retained input node. The
            // returned format is retained for converter construction; there are no pointers or
            // escaping blocks. This occurs on the serialized main-thread graph owner.
            let input_format = unsafe { input.outputFormatForBus(0) };
            // SAFETY: `initFromFormat:toFormat:` constructs a retained converter from retained
            // formats. Its lifetime is captured by the retained tap block; no raw PCM is retained
            // beyond the synchronous conversion. Setup is on the main-thread graph owner.
            let converter = unsafe {
                AVAudioConverter::initFromFormat_toFormat(
                    AVAudioConverter::alloc(),
                    &input_format,
                    &format,
                )
                .ok_or(AudioError::EngineStartFailed)?
            };
            let tap_state = Arc::clone(&state);
            let tap_format = format.clone();
            let tap_input_format = input_format.clone();
            let tap: RcBlock<dyn Fn(NonNull<AVAudioPCMBuffer>, NonNull<AVAudioTime>)> =
                RcBlock::new(
                    move |buffer: NonNull<AVAudioPCMBuffer>, _time: NonNull<AVAudioTime>| {
                        let buffer = unsafe { buffer.as_ref() };
                        let source_rate = unsafe { tap_input_format.sampleRate() };
                        let input_frames = unsafe { buffer.frameLength() } as f64;
                        if !(source_rate.is_finite() && source_rate > 0.0) {
                            return;
                        }
                        let output_capacity =
                            (input_frames * f64::from(SAMPLE_RATE_HZ) / source_rate)
                                .ceil()
                                .clamp(1.0, f64::from(u32::MAX)) as u32;
                        let output = unsafe {
                            AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
                                AVAudioPCMBuffer::alloc(),
                                &tap_format,
                                output_capacity,
                            )
                        };
                        let Some(output) = output else { return };
                        let supplied = std::cell::Cell::new(false);
                        let input_block: RcBlock<
                            dyn Fn(u32, NonNull<AVAudioConverterInputStatus>) -> *mut AVAudioBuffer,
                        > = RcBlock::new(
                            move |_: u32, status: NonNull<AVAudioConverterInputStatus>| {
                                // SAFETY: the converter supplies a valid status pointer for this
                                // synchronous block invocation. The input buffer remains borrowed by
                                // the tap for the conversion call and is never retained by this block.
                                unsafe {
                                    if supplied.replace(true) {
                                        status
                                            .as_ptr()
                                            .write(AVAudioConverterInputStatus::NoDataNow);
                                        std::ptr::null_mut()
                                    } else {
                                        status
                                            .as_ptr()
                                            .write(AVAudioConverterInputStatus::HaveData);
                                        (buffer as *const AVAudioPCMBuffer)
                                            .cast_mut()
                                            .cast::<AVAudioBuffer>()
                                    }
                                }
                            },
                        );
                        let input_pointer: AVAudioConverterInputBlock =
                            RcBlock::as_ptr(&input_block);
                        let mut conversion_error = None;
                        // SAFETY: the generated block API is mandatory for rate conversion. It
                        // receives a valid retained output buffer and a synchronous input block
                        // whose borrowed tap buffer cannot outlive this invocation.
                        unsafe {
                            converter.convertToBuffer_error_withInputFromBlock(
                                output.as_ref(),
                                Some(&mut conversion_error),
                                input_pointer,
                            )
                        };
                        if conversion_error.is_some() {
                            return;
                        }
                        let frame_length = unsafe { output.frameLength() } as usize;
                        let channels = unsafe { output.int16ChannelData() };
                        let Some(samples) = (unsafe { channels.as_ref() }) else {
                            return;
                        };
                        let byte_len = frame_length.saturating_mul(std::mem::size_of::<i16>());
                        let bytes = unsafe {
                            std::slice::from_raw_parts(samples.as_ptr().cast::<u8>(), byte_len)
                        };
                        let mut state =
                            tap_state.state.lock().expect("voice state is not poisoned");
                        if state.terminal.is_some() || state.stopped {
                            return;
                        }
                        if state.append_capture(bytes).is_err() {
                            state.terminate(TerminalSignal::CaptureOverflow);
                        }
                        tap_state.changed.notify_all();
                    },
                );
            let tap_pointer: AVAudioNodeTapBlock = RcBlock::as_ptr(&tap);
            // SAFETY: `installTapOnBus:bufferSize:format:block:` copies the block and retains it
            // until `removeTapOnBus:`. The block captures only retained converter/format and an
            // Arc of Rust-owned state; it copies PCM into owned 960-byte frames before returning.
            // The selector is called by the single main-thread graph owner; callback queues never
            // mutate/release graph objects or retain raw PCM pointers.
            unsafe {
                input.installTapOnBus_bufferSize_format_block(
                    0,
                    FRAME_SAMPLES as u32,
                    Some(&input_format),
                    tap_pointer,
                )
            };
            let notification_center = NSNotificationCenter::defaultCenter();
            let interrupt_state = Arc::clone(&state);
            let interruption = RcBlock::new(move |_| {
                // The notification arrives on an arbitrary internal queue. It only transfers the
                // terminal state into Rust-owned memory; it does not stop, remove, or deallocate
                // the engine, input tap, observer, retained objects, or a block.
                interrupt_state
                    .state
                    .lock()
                    .expect("voice state is not poisoned")
                    .terminate(TerminalSignal::Interrupted);
                interrupt_state.changed.notify_all();
            });
            // SAFETY: `addObserverForName:object:queue:usingBlock:` copies `interruption` and
            // returns a retained observer token held by `observer` until explicit removal. The
            // retained engine is the matching object; no raw PCM pointer escapes. `None` permits
            // Apple's callback queue, which only mutates Rust-owned state as documented above.
            let observer = unsafe {
                notification_center.addObserverForName_object_queue_usingBlock(
                    Some(AVAudioEngineConfigurationChangeNotification),
                    Some(engine.as_ref() as &AnyObject),
                    None,
                    &interruption,
                )
            };
            // SAFETY: `startAndReturnError:` has no borrowed pointer output in objc2's Result
            // wrapper; retained graph objects outlive the call; main-thread-only graph start.
            unsafe { engine.startAndReturnError() }.map_err(|_| AudioError::EngineStartFailed)?;
            // SAFETY: `play` starts the retained player node; it neither receives pointers nor
            // blocks; the node and engine remain retained and this happens on the main thread.
            unsafe { player.play() };
            Ok(Self {
                engine,
                player,
                format,
                input,
                observer,
                notification_center,
                state,
                tap_installed: true,
                observer_installed: true,
                stopped: false,
                _main_thread_only: PhantomData,
            })
        }

        pub(super) fn schedule(&self, frame: &PcmFrame, ticket: u64) -> Result<(), AudioError> {
            let _marker = MainThreadMarker::new().ok_or(AudioError::PlatformUnavailable)?;
            // SAFETY: `initWithPCMFormat:frameCapacity:` returns an owned retained buffer tied
            // to this function until scheduled; the capacity is exactly FRAME_SAMPLES, no borrowed
            // block exists, and this is called on the main thread required by the graph.
            let buffer = unsafe {
                AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
                    AVAudioPCMBuffer::alloc(),
                    &self.format,
                    FRAME_SAMPLES as u32,
                )
                .ok_or(AudioError::EngineStartFailed)?
            };
            // SAFETY: `setFrameLength:` sets exactly FRAME_SAMPLES, which is within the buffer's
            // FRAME_SAMPLES capacity; `buffer` is retained locally; no block escapes; main-thread-only.
            unsafe { buffer.setFrameLength(FRAME_SAMPLES as u32) };
            // SAFETY: `int16ChannelData` selects the first mono channel. The buffer has exactly
            // FRAME_SAMPLES i16 slots (FRAME_BYTES bytes) by the format and capacity above; the
            // raw pointer is used only while `buffer` is retained locally; no block escapes; main-thread-only.
            let samples = unsafe { buffer.int16ChannelData().as_ref() }
                .ok_or(AudioError::EngineStartFailed)?
                .as_ptr();
            // SAFETY: the destination spans FRAME_SAMPLES i16 values proven by allocation and
            // frame length above; source is exactly FRAME_BYTES owned by `frame`; retained buffer
            // stays alive through the copy; this graph operation is main-thread-only.
            unsafe {
                std::ptr::copy_nonoverlapping(frame.0.as_ptr(), samples.cast::<u8>(), FRAME_BYTES)
            };
            let completion_state = Arc::clone(&self.state);
            let completion: RcBlock<dyn Fn(AVAudioPlayerNodeCompletionCallbackType)> =
                RcBlock::new(move |callback_type| {
                    if callback_type == AVAudioPlayerNodeCompletionCallbackType::DataPlayedBack {
                        let mut state = completion_state
                            .state
                            .lock()
                            .expect("voice state is not poisoned");
                        if state.playback.retire(ticket) {
                            completion_state.changed.notify_all();
                        }
                    }
                });
            let completion_pointer: AVAudioPlayerNodeCompletionHandler =
                RcBlock::as_ptr(&completion);
            // SAFETY: `scheduleBuffer:completionCallbackType:completionHandler:` retains the
            // buffer and completion block. The callback removes only this exact ticket on the
            // DataPlayedBack event and does not mutate the graph or access PCM memory.
            unsafe {
                self.player
                    .scheduleBuffer_completionCallbackType_completionHandler(
                        &buffer,
                        AVAudioPlayerNodeCompletionCallbackType::DataPlayedBack,
                        completion_pointer,
                    )
            };
            Ok(())
        }

        #[cfg(feature = "hardware-acceptance")]
        pub(super) fn pause_capture(&mut self) {
            let _marker = MainThreadMarker::new().expect("VoiceBoundary is main-thread-owned");
            if self.tap_installed {
                // SAFETY: the sole retained input owns the sole tap; this is main-thread graph mutation.
                unsafe { self.input.removeTapOnBus(0) };
                self.tap_installed = false;
            }
        }

        pub(super) fn stop(&mut self) {
            let _marker = MainThreadMarker::new().expect("VoiceBoundary is main-thread-owned");
            if self.stopped {
                return;
            }
            self.stopped = true;
            // SAFETY: `removeTapOnBus:` unregisters the sole installed tap before graph release.
            // The input node and its graph remain retained for the selector; block lifetime ends
            // only after AVFoundation removes it; no PCM pointer is used. This is main-thread-only.
            if self.tap_installed {
                unsafe { self.input.removeTapOnBus(0) };
                self.tap_installed = false;
            }
            // SAFETY: `removeObserver:` releases the retained token's registration before the
            // token, engine, and callback block can drop. The observer is the exact value returned
            // from registration; no PCM pointer is used; graph teardown is main-thread-only.
            if self.observer_installed {
                let observer: &ProtocolObject<dyn NSObjectProtocol> = self.observer.as_ref();
                unsafe { self.notification_center.removeObserver(observer.as_ref()) };
                self.observer_installed = false;
            }
            // SAFETY: `stop` selectors release engine/player hardware resources. Both retained
            // objects remain alive for the call; no PCM pointer or callback block is involved;
            // VoiceBoundary's public API is main-thread-only on macOS.
            unsafe {
                self.player.stop();
                self.engine.stop();
            }
        }

        pub(super) fn clear_playback(&self) {
            let _marker = MainThreadMarker::new().expect("VoiceBoundary is main-thread-owned");
            // SAFETY: stopping the retained player discards its scheduled buffers; restarting
            // the same attached node keeps the capture graph active. Both selectors run on the
            // serialized main-thread owner and no PCM pointer crosses either call.
            unsafe {
                self.player.stop();
                self.player.play();
            }
        }
    }

    pub(super) fn start_audio(
        state: Arc<VoiceStateOwner>,
    ) -> Result<PlatformAudioHandle, AudioError> {
        run_on_main(move || {
            let audio = PlatformAudio::start(state)?;
            let id = NEXT_PLATFORM_AUDIO_HANDLE.fetch_add(1, Ordering::Relaxed);
            PLATFORM_AUDIO.with(|registry| {
                registry.borrow_mut().insert(id, audio);
            });
            Ok(id)
        })
    }

    pub(super) fn schedule_audio(
        id: PlatformAudioHandle,
        frame: PcmFrame,
        ticket: u64,
    ) -> Result<(), AudioError> {
        run_on_main(move || {
            PLATFORM_AUDIO.with(|registry| {
                registry
                    .borrow()
                    .get(&id)
                    .ok_or(AudioError::Stopped)?
                    .schedule(&frame, ticket)
            })
        })
    }

    pub(super) fn stop_audio(id: PlatformAudioHandle) {
        let _ = run_on_main(move || {
            PLATFORM_AUDIO.with(|registry| {
                registry.borrow_mut().remove(&id);
            });
            Ok(())
        });
    }

    pub(super) fn clear_playback(id: PlatformAudioHandle) -> Result<(), AudioError> {
        run_on_main(move || {
            PLATFORM_AUDIO.with(|registry| {
                registry
                    .borrow()
                    .get(&id)
                    .ok_or(AudioError::Stopped)?
                    .clear_playback();
                Ok(())
            })
        })
    }

    #[cfg(feature = "hardware-acceptance")]
    pub(super) fn pause_audio(id: PlatformAudioHandle) {
        let _ = run_on_main(move || {
            PLATFORM_AUDIO.with(|registry| {
                if let Some(audio) = registry.borrow_mut().get_mut(&id) {
                    audio.pause_capture();
                }
            });
            Ok(())
        });
    }

    fn run_on_main<T: Send + 'static>(
        operation: impl FnOnce() -> Result<T, AudioError> + Send + 'static,
    ) -> Result<T, AudioError> {
        if MainThreadMarker::new().is_some() {
            return operation();
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let operation = StdMutex::new(Some(operation));
        let block = RcBlock::new(move || {
            let operation = operation
                .lock()
                .expect("main audio operation should not be poisoned")
                .take()
                .expect("main audio operation runs once");
            let _ = sender.send(operation());
        });
        let queue = NSOperationQueue::mainQueue();
        // SAFETY: the queue copies the block and invokes it once on the process main queue.
        // The block owns its closure and result sender; no borrowed PCM or Objective-C object
        // crosses the call.
        unsafe { queue.addOperationWithBlock(&block) };
        receiver
            .recv()
            .map_err(|_| AudioError::PlatformUnavailable)?
    }

    impl Drop for PlatformAudio {
        fn drop(&mut self) {
            self.stop();
        }
    }

    pub(super) fn permission_status() -> PermissionStatus {
        let Some(_marker) = MainThreadMarker::new() else {
            return PermissionStatus::Restricted;
        };
        // SAFETY: `authorizationStatusForMediaType:` is a class selector with a static media
        // type and no raw pointers or blocks; the call does not retain a borrowed object; it is
        // made on the main thread because authorization UI is main-thread-affine.
        let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
            return PermissionStatus::Restricted;
        };
        match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) } {
            AVAuthorizationStatus::NotDetermined => PermissionStatus::NotDetermined,
            AVAuthorizationStatus::Restricted => PermissionStatus::Restricted,
            AVAuthorizationStatus::Denied => PermissionStatus::Denied,
            AVAuthorizationStatus::Authorized => PermissionStatus::Authorized,
            _ => PermissionStatus::Restricted,
        }
    }

    pub(super) fn request_permission(callback: impl FnOnce(PermissionStatus) + 'static) {
        let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
            callback(PermissionStatus::Restricted);
            return;
        };
        let callback = std::cell::RefCell::new(Some(callback));
        let handler = RcBlock::new(move |granted: objc2::runtime::Bool| {
            if let Some(callback) = callback.borrow_mut().take() {
                callback(if granted.as_bool() {
                    PermissionStatus::Authorized
                } else {
                    PermissionStatus::Denied
                });
            }
        });
        // SAFETY: `requestAccessForMediaType:completionHandler:` copies the supplied block before
        // returning; `handler` therefore remains valid for AVFoundation's arbitrary callback queue.
        // The media-type static is non-null above, no PCM pointer is involved, and the request is
        // initiated on the main thread; the callback owns its one-shot closure.
        unsafe {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &handler)
        };
    }
}

#[cfg(all(target_os = "macos", feature = "hardware-acceptance"))]
use platform::pause_audio as pause_platform_audio;
#[cfg(target_os = "macos")]
use platform::{
    PlatformAudioHandle, clear_playback as clear_platform_playback,
    permission_status as platform_permission_status,
    request_permission as platform_request_permission, schedule_audio as schedule_platform_audio,
    start_audio as start_platform_audio, stop_audio as stop_platform_audio,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: u8) -> PcmFrame {
        PcmFrame::from_le_bytes([value; FRAME_BYTES])
    }

    #[test]
    fn exact_contract_frame_shape_is_enforced() {
        assert_eq!(FRAME_BYTES, 960);
        assert_eq!(FRAME_SAMPLES, 480);
        assert!(PcmFrame::try_from_slice(&[0; FRAME_BYTES]).is_ok());
        assert_eq!(
            PcmFrame::try_from_slice(&[0; FRAME_BYTES - 1]),
            Err(AudioError::InvalidFrameLength {
                actual: FRAME_BYTES - 1
            })
        );
    }

    #[test]
    fn capture_overflow_is_terminal_and_releases_both_queues() {
        let mut boundary = VoiceBoundary::default();
        for value in 0..CAPTURE_QUEUE_CAPACITY {
            boundary
                .enqueue_capture(frame(value as u8))
                .expect("capacity accepts frames");
        }
        assert_eq!(
            boundary.enqueue_capture(frame(9)),
            Err(AudioError::AlreadyTerminated(
                TerminalSignal::CaptureOverflow
            ))
        );
        assert_eq!(
            boundary.terminal_signal(),
            Some(TerminalSignal::CaptureOverflow)
        );
        assert!(boundary.dequeue_capture().is_none());
        assert_eq!(boundary.pending_playback_count(), 0);
    }

    #[test]
    fn playback_overflow_is_terminal_and_releases_both_queues() {
        let mut boundary = VoiceBoundary::default();
        for value in 0..PLAYBACK_QUEUE_CAPACITY {
            boundary
                .enqueue_playback(frame(value as u8))
                .expect("capacity accepts frames");
        }
        assert_eq!(
            boundary.enqueue_playback(frame(9)),
            Err(AudioError::AlreadyTerminated(
                TerminalSignal::PlaybackOverflow
            ))
        );
        assert_eq!(
            boundary.terminal_signal(),
            Some(TerminalSignal::PlaybackOverflow)
        );
        assert!(boundary.dequeue_capture().is_none());
        assert_eq!(boundary.pending_playback_count(), 0);
    }

    #[test]
    fn interruption_is_single_terminal_signal_and_clears_queues() {
        let mut boundary = VoiceBoundary::default();
        boundary.enqueue_capture(frame(1)).expect("active");
        boundary.enqueue_playback(frame(2)).expect("active");
        boundary.interrupt();
        boundary.interrupt();
        assert_eq!(
            boundary.terminal_signal(),
            Some(TerminalSignal::Interrupted)
        );
        assert!(boundary.dequeue_capture().is_none());
        assert_eq!(boundary.pending_playback_count(), 0);
    }

    #[test]
    fn stop_clears_memory_without_hardware() {
        let mut boundary = VoiceBoundary::default();
        boundary.enqueue_capture(frame(1)).expect("active");
        boundary.enqueue_playback(frame(2)).expect("active");
        boundary.stop();
        assert!(boundary.dequeue_capture().is_none());
        assert_eq!(boundary.pending_playback_count(), 0);
    }

    #[test]
    fn playback_clear_preserves_capture_and_accepts_new_output() {
        let mut boundary = VoiceBoundary::default();
        boundary.enqueue_capture(frame(1)).expect("active capture");
        boundary
            .enqueue_playback(frame(2))
            .expect("active playback");
        boundary.clear_playback().expect("clear playback");
        assert_eq!(boundary.pending_playback_count(), 0);
        assert_eq!(boundary.dequeue_capture(), Some(frame(1)));
        boundary.enqueue_playback(frame(3)).expect("new playback");
        assert_eq!(boundary.pending_playback_count(), 1);
    }

    #[test]
    fn residual_assembler_emits_continuous_frames_across_arbitrary_chunk_boundaries() {
        let source = (0..(FRAME_BYTES * 2 + 117))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let chunk_lengths = [17, 403, 1, 529, 970, 117];
        let mut state = VoiceState::default();
        let mut offset = 0;

        for chunk_len in chunk_lengths {
            let end = offset + chunk_len;
            state
                .append_capture(&source[offset..end])
                .expect("capture queue has capacity for two emitted frames");
            offset = end;
        }

        assert_eq!(offset, source.len());
        assert_eq!(state.capture.frames.len(), 2);
        assert_eq!(state.capture.bytes, FRAME_BYTES * 2);
        assert_eq!(state.residual.bytes.len(), 117);
        assert!(state.residual.bytes.len() < FRAME_BYTES);
        assert_eq!(
            state.capture.pop().expect("first frame").as_le_bytes(),
            &source[..FRAME_BYTES]
        );
        assert_eq!(
            state.capture.pop().expect("second frame").as_le_bytes(),
            &source[FRAME_BYTES..FRAME_BYTES * 2]
        );
        assert_eq!(state.residual.bytes, source[FRAME_BYTES * 2..]);
    }

    #[test]
    fn playback_ledger_retires_exact_ticket_and_rolls_back_failed_schedule() {
        let mut playback = PlaybackLedger::default();
        let first = playback.reserve().expect("first reservation");
        let second = playback.reserve().expect("second reservation");
        let third = playback.reserve().expect("third reservation");
        assert_eq!(playback.tickets.len(), 3);
        assert_eq!(playback.bytes, FRAME_BYTES * 3);

        assert!(playback.retire(second));
        assert_eq!(playback.tickets.len(), 2);
        assert_eq!(playback.bytes, FRAME_BYTES * 2);
        assert_eq!(playback.completed, 1);
        assert_eq!(
            playback.tickets.front().map(|ticket| ticket.id),
            Some(first)
        );
        assert_eq!(playback.tickets.back().map(|ticket| ticket.id), Some(third));

        playback.rollback(third);
        assert_eq!(playback.tickets.len(), 1);
        assert_eq!(playback.bytes, FRAME_BYTES);
        assert_eq!(playback.completed, 1);
        assert!(!playback.retire(third));
        assert_eq!(playback.completed, 1);
    }

    #[test]
    fn playback_completion_cursor_advances_only_on_confirmed_completion() {
        let mut playback = PlaybackLedger::default();
        let first = playback.reserve().expect("first reservation");
        let second = playback.reserve().expect("second reservation");
        assert_eq!(playback.completed, 0);
        playback.rollback(second);
        assert_eq!(playback.completed, 0);
        assert!(playback.retire(first));
        assert_eq!(playback.completed, 1);
        playback.clear();
        assert_eq!(playback.completed, 1);
    }

    #[test]
    fn stop_is_idempotent_and_rejects_new_work() {
        let mut boundary = VoiceBoundary::default();
        boundary.enqueue_capture(frame(1)).expect("active capture");
        boundary
            .enqueue_playback(frame(2))
            .expect("active playback");

        boundary.stop();
        boundary.stop();

        assert_eq!(boundary.enqueue_capture(frame(3)), Err(AudioError::Stopped));
        assert_eq!(
            boundary.enqueue_playback(frame(4)),
            Err(AudioError::Stopped)
        );
        assert!(boundary.dequeue_capture().is_none());
        assert_eq!(boundary.pending_playback_count(), 0);
    }
}
