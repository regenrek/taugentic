use serde_json::Value;

use super::CodexLlmClientError;

pub fn parse_jsonl_frame(frame: &[u8]) -> Result<Value, CodexLlmClientError> {
    let frame = trim_jsonl_frame(frame);
    if frame.is_empty() {
        return Err(CodexLlmClientError::Protocol(
            "codex app-server emitted an empty JSONL frame".to_string(),
        ));
    }
    let text = std::str::from_utf8(frame).map_err(|error| {
        CodexLlmClientError::Protocol(format!("codex app-server emitted invalid UTF-8: {error}"))
    })?;
    serde_json::from_str(text).map_err(|error| {
        CodexLlmClientError::Protocol(format!("codex app-server emitted invalid JSON: {error}"))
    })
}

fn trim_jsonl_frame(mut frame: &[u8]) -> &[u8] {
    while frame.last().is_some_and(u8::is_ascii_whitespace) {
        frame = &frame[..frame.len().saturating_sub(1)];
    }
    frame
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn jsonl_frame_parse_is_utf8_safe_at_frame_boundary() {
        let value = parse_jsonl_frame(
            "{\"method\":\"item/agentMessage/delta\",\"params\":{\"delta\":\"☃\"}}\n".as_bytes(),
        )
        .expect("frame");
        assert_eq!(
            value
                .get("params")
                .and_then(|params| params.get("delta"))
                .and_then(Value::as_str),
            Some("☃")
        );
    }

    #[test]
    fn malformed_jsonl_frame_returns_typed_protocol_error() {
        let error = parse_jsonl_frame(b"Content-Length: 10\r\n\r\n{}")
            .expect_err("content-length frame is invalid for codex stdio JSONL");
        assert!(matches!(error, CodexLlmClientError::Protocol(_)));
    }
}
