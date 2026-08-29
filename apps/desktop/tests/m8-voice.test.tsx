import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import { VoicePanel } from "../src/features/voice/voice-panel.js"
import { createDesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import {
  observeVoice,
  readVoicePermission,
  requestVoicePermission,
} from "../src/platform/daemon/voice-query.js"

describe("M8 realtime voice", () => {
  it("shows permission, live progress, completion, and failure without exposing audio", () => {
    const { render, renderer, unmount } = createTestRoot()
    let requests = 0
    try {
      render(<VoicePanel visible permission="notDetermined" onRequestPermission={() => { requests += 1 }} />)
      expect(renderer.getPaintedText()).toContain("Microphone access is required")
      const permission = renderer.findByTestId("request-voice-permission")!
      renderer.nativeSimulateKeystrokes(permission.id, "enter")
      expect(requests).toBe(1)

      render(<VoicePanel visible permission="authorized" state={{ runId: "run-voice", phase: "listening" }} onRequestPermission={() => {}} />)
      expect(renderer.getPaintedText()).toContain("Listening")
      render(<VoicePanel visible permission="authorized" state={{ runId: "run-voice", phase: "speaking" }} onRequestPermission={() => {}} />)
      expect(renderer.getPaintedText()).toContain("Assistant speaking")
      render(<VoicePanel visible permission="authorized" runStatus="completed" onRequestPermission={() => {}} />)
      expect(renderer.getPaintedText()).toContain("Voice session completed")
      render(<VoicePanel visible permission="authorized" runStatus="failed" onRequestPermission={() => {}} />)
      expect(renderer.getPaintedText()).toContain("Voice session failed")
    } finally {
      unmount()
    }
  })

  it("renders nothing for a non-voice runtime profile", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<VoicePanel visible={false} permission="restricted" onRequestPermission={() => {}} />)
      expect(renderer.findByTestId("voice-panel")).toBeUndefined()
    } finally {
      unmount()
    }
  })

  it("does not offer a microphone request when the system cannot request one", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<VoicePanel visible permission="restricted" onRequestPermission={() => { throw new Error("restricted permission cannot be requested") }} />)
      expect(renderer.getPaintedText()).toContain("Microphone access is unavailable")
      expect(renderer.findByTestId("request-voice-permission")).toBeUndefined()

      render(<VoicePanel visible permission="denied" onRequestPermission={() => { throw new Error("denied permission cannot be requested") }} />)
      expect(renderer.findByTestId("request-voice-permission")).toBeUndefined()
    } finally {
      unmount()
    }
  })

  it("calls the required native Voice boundary and preserves native failures", () => {
    const calls: string[] = []
    const bridge = {
      voicePermissionState() {
        calls.push("read")
        return JSON.stringify("authorized")
      },
      requestVoicePermission(callback: (permissionJson: string) => void) {
        calls.push("request")
        callback(JSON.stringify("denied"))
      },
      subscribeVoiceState(callback: (eventJson: string) => void) {
        calls.push("subscribe")
        callback(JSON.stringify({ runId: "run-voice", phase: "listening" }))
      },
    } as unknown as NativeDaemonBridge
    const runtime = createDesktopRuntime(bridge)
    const permissions: string[] = []
    const phases: string[] = []

    observeVoice(
      runtime,
      (permission) => permissions.push(permission),
      (event) => phases.push(event.phase),
    )
    requestVoicePermission(runtime, (permission) => permissions.push(permission))

    expect(calls).toEqual(["read", "subscribe", "request"])
    expect(permissions).toEqual(["authorized", "denied"])
    expect(phases).toEqual(["listening"])

    const failedRuntime = createDesktopRuntime({
      voicePermissionState() {
        throw new Error("native permission unavailable")
      },
    } as unknown as NativeDaemonBridge)
    expect(() => readVoicePermission(failedRuntime)).toThrow("native permission unavailable")
  })
})
