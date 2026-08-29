//! One-shot physical macOS Voice acceptance executable.
//!
//! It is deliberately feature-gated and only emits temporary metadata. PCM stays
//! inside `ta-macos-avfoundation` for the lifetime of the process.

#![cfg(target_os = "macos")]

use std::{env, fs, path::PathBuf, sync::mpsc};

use ta_macos_avfoundation::{AudioError, FRAME_BYTES, PcmFrame, PermissionStatus, VoiceBoundary};

fn result_path() -> Result<PathBuf, AudioError> {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(flag), Some(path), None) if flag == "--voice-acceptance-result" => Ok(path.into()),
        _ => Err(AudioError::PlatformUnavailable),
    }
}

fn main() -> Result<(), AudioError> {
    let output = result_path()?;
    let (permission_sender, permission_receiver) = mpsc::sync_channel(1);
    VoiceBoundary::request_permission(move |status| {
        let _ = permission_sender.send(status);
    });
    if permission_receiver
        .recv()
        .map_err(|_| AudioError::PlatformUnavailable)?
        != PermissionStatus::Authorized
    {
        return Err(AudioError::PlatformUnavailable);
    }

    let mut voice = VoiceBoundary::start()?;
    voice.enqueue_playback(PcmFrame::from_le_bytes([0; FRAME_BYTES]))?;
    let _captured = voice.wait_for_capture()?;
    voice.pause_capture_for_hardware_acceptance();
    voice.wait_for_playback_settled()?;
    // A human now changes the physical audio output or input device. The native
    // configuration observer is the only source that releases this wait.
    voice.wait_for_interruption()?;
    voice.stop();

    let (captured_frames, completed_playback_tickets, stopped) = voice.hardware_snapshot();
    if captured_frames == 0 || completed_playback_tickets == 0 || !stopped {
        return Err(AudioError::PlatformUnavailable);
    }
    let metadata = format!(
        concat!(
            "{{\"version\":1,\"permission\":\"authorized\",",
            "\"captured_frames\":{captured_frames},",
            "\"completed_playback_tickets\":{completed_playback_tickets},",
            "\"terminal\":\"interrupted\",\"teardown\":true}}\n"
        ),
        captured_frames = captured_frames,
        completed_playback_tickets = completed_playback_tickets,
    );
    fs::write(output, metadata).map_err(|_| AudioError::PlatformUnavailable)
}
