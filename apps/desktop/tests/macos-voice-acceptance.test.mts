import { describe, expect, it } from "bun:test";

import { parseVoiceAcceptanceMetadata } from "../scripts/macos-voice-acceptance.mjs";

describe("macOS Voice acceptance metadata", () => {
  it("accepts only the fixed metadata-only completion schema", () => {
    expect(
      parseVoiceAcceptanceMetadata(
        '{"version":1,"permission":"authorized","captured_frames":1,"completed_playback_tickets":1,"terminal":"interrupted","teardown":true}',
      ),
    ).toEqual({
      version: 1,
      permission: "authorized",
      captured_frames: 1,
      completed_playback_tickets: 1,
      terminal: "interrupted",
      teardown: true,
    });
  });

  it("rejects incomplete or non-metadata oracle values", () => {
    expect(() => parseVoiceAcceptanceMetadata('{"version":1}')).toThrow(
      "Voice acceptance metadata does not satisfy the fixed schema",
    );
  });
});
