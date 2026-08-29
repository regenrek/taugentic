use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::RunId;

pub const VOICE_SAMPLE_RATE_HZ: u32 = 24_000;
pub const VOICE_FRAME_BYTES: usize = 960;
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[doc(hidden)]
pub fn encode_voice_audio(bytes: &[u8; VOICE_FRAME_BYTES]) -> String {
    let mut encoded = String::with_capacity(VOICE_FRAME_BYTES.div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(BASE64_ALPHABET[(first >> 2) as usize] as char);
        encoded.push(BASE64_ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[doc(hidden)]
pub fn decode_voice_audio(value: &str) -> Option<[u8; VOICE_FRAME_BYTES]> {
    if value.len() != VOICE_FRAME_BYTES.div_ceil(3) * 4 || value.len() % 4 != 0 {
        return None;
    }
    let mut decoded = Vec::with_capacity(VOICE_FRAME_BYTES);
    for chunk in value.as_bytes().chunks_exact(4) {
        let a = decode_base64_symbol(chunk[0])?;
        let b = decode_base64_symbol(chunk[1])?;
        let c = (chunk[2] != b'=')
            .then(|| decode_base64_symbol(chunk[2]))
            .flatten();
        let d = (chunk[3] != b'=')
            .then(|| decode_base64_symbol(chunk[3]))
            .flatten();
        decoded.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            decoded.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                decoded.push((c << 6) | d);
            }
        }
    }
    decoded.try_into().ok()
}

fn decode_base64_symbol(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum VoicePermissionState {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum VoicePhase {
    Connecting,
    Listening,
    Speaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct VoiceEvent {
    pub run_id: RunId,
    pub phase: VoicePhase,
}

// The local audio transport is intentionally Rust-only. These types are not
// exported by the protocol generator and cannot enter TypeScript or N-API.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamOpenParams {
    pub run_id: RunId,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamOpenResult {
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<VoiceEvent>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamExchangeParams {
    pub run_id: RunId,
    pub audio_base64: String,
    pub playback_completed_frames: u64,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamExchangeResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<VoiceEvent>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub playback_interrupted: bool,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceStreamEndReason {
    Interrupted,
    CaptureOverflow,
    PlaybackOverflow,
    DeviceUnavailable,
    Replaced,
    Shutdown,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamEndParams {
    pub run_id: RunId,
    pub reason: VoiceStreamEndReason,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceStreamEndResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_exchange_serializes_interruption_without_audio_when_absent() {
        let result = VoiceStreamExchangeResult {
            audio_base64: None,
            state: None,
            playback_interrupted: true,
        };
        assert_eq!(
            serde_json::to_value(result).expect("private exchange"),
            serde_json::json!({"playbackInterrupted": true})
        );
    }

    #[test]
    fn private_exchange_omits_inactive_interruption_control() {
        let result = VoiceStreamExchangeResult {
            audio_base64: None,
            state: None,
            playback_interrupted: false,
        };
        assert_eq!(
            serde_json::to_value(result).expect("private exchange"),
            serde_json::json!({})
        );
    }
}
