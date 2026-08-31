import { handleGpuixEvent, type NativeRenderer } from "@regenrek/gpuix-react"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import type {
  DesktopDaemonLifecycleProjection,
  TerminalInputParams,
  TerminalResizeParams,
  TerminalSessionSummary,
  TerminalSpawnParams,
} from "@taugentic/desktop-protocol"

import { TerminalPanel } from "../src/features/terminal/terminal-panel.js"
import { useWorkbenchTerminal } from "../src/features/terminal/use-workbench-terminal.js"
import type { DesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"

const terminal: TerminalSessionSummary = {
  id: "terminal-one",
  projectId: "project-one",
  workspaceId: "workspace-one",
  status: "running",
  rows: 24,
  cols: 80,
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
}

function attachmentSignal(): { settled: Promise<void>; resolve(): void } {
  let resolve!: () => void
  const settled = new Promise<void>((complete) => {
    resolve = complete
  })
  return { settled, resolve }
}

function fakeRuntime(record: {
  spawns: TerminalSpawnParams[]
  inputs: TerminalInputParams[]
  resizes: TerminalResizeParams[]
  attachments: number
  releases: number
  attachmentSignals?: Array<() => void>
}): DesktopRuntime {
  return {
    bridge: {} as DesktopRuntime["bridge"],
    async start() {},
    async close() {},
    async subscribeLifecycle(listener) {
      const projection: DesktopDaemonLifecycleProjection = { status: "ready", invalidated: false, foreignRuntimeRestricted: false }
      listener(projection)
      return projection
    },
    async forkRun() {
      throw new Error("Forking is outside the terminal fixture.")
    },
    async continueRun() {
      throw new Error("Continuing runs is outside the terminal fixture.")
    },
    async switchRouteAndResume() {
      throw new Error("Switching routes is outside the terminal fixture.")
    },
    async spawnRun() {
      throw new Error("Fresh spawning is outside the terminal fixture.")
    },
    async joinRun() {
      throw new Error("Joining fresh runs is outside the terminal fixture.")
    },
    async navigationIntent() {
      throw new Error("Navigation is outside the terminal fixture.")
    },
    async listRecipes() {
      return { recipes: [] }
    },
    async diagnosticsSnapshot() {
      throw new Error("Diagnostics are outside the terminal fixture.")
    },
    async browserProfile() {
      throw new Error("Browser profiles are outside the terminal fixture.")
    },
    async browserAction() {
      throw new Error("Browser actions are outside the terminal fixture.")
    },
    async clearBrowserData() {
      throw new Error("Browser data clearing is outside the terminal fixture.")
    },
    async listWorkItems() {
      throw new Error("Work Inbox is outside the terminal fixture.")
    },
    async refreshWorkItems() {
      throw new Error("Work Inbox is outside the terminal fixture.")
    },
    async dismissWorkItem() {
      throw new Error("Work Inbox is outside the terminal fixture.")
    },
    async triggerWorkItem() {
      throw new Error("Work Inbox is outside the terminal fixture.")
    },
    async createScheduledWork() {
      throw new Error("Scheduled Work is outside the terminal fixture.")
    },
    async listScheduledWork() {
      throw new Error("Scheduled Work is outside the terminal fixture.")
    },
    async cancelScheduledWork() {
      throw new Error("Scheduled Work is outside the terminal fixture.")
    },
    async listNativeRuns() {
      return { runs: [] }
    },
    async runLineageGraph() {
      return { nodes: [], edges: [], orphanRunIds: [], totalCount: 0, omittedCount: 0, truncated: false, cycleBroken: false }
    },
    async spawnTerminal(params) {
      record.spawns.push(params)
      return { terminal: { ...terminal, rows: params.rows, cols: params.cols } }
    },
    async listTerminals() {
      return { terminals: [] }
    },
    async terminalInput(params) {
      record.inputs.push(params)
      return { acceptedBytes: Buffer.from(params.dataBase64, "base64").byteLength }
    },
    async resizeTerminal(params) {
      record.resizes.push(params)
      return { terminal: { ...terminal, rows: params.rows, cols: params.cols } }
    },
    async closeTerminal() {
      return { terminal: { ...terminal, status: "exited" } }
    },
    async subscribeTerminal(_terminalId, listener) {
      record.attachments += 1
      record.attachmentSignals?.shift()?.()
      const initial = {
        terminal,
        snapshotBase64: Buffer.from("snapshot ").toString("base64"),
        snapshotTruncated: false,
        latestSequence: "1",
      }
      listener.attached(initial)
      listener.event({
        terminalId: terminal.id,
        event: { kind: "output", sequence: "2", dataBase64: Buffer.from("live").toString("base64") },
      })
      return initial
    },
    releaseTerminalSubscription() {
      record.releases += 1
    },
    voicePermissionState() {
      throw new Error("Voice is outside the terminal fixture.")
    },
    requestVoicePermission() {
      throw new Error("Voice is outside the terminal fixture.")
    },
    subscribeVoiceState() {
      throw new Error("Voice is outside the terminal fixture.")
    },
  }
}

function Harness(props: { runtime: DesktopRuntime; renderer: NativeRenderer; show: boolean }) {
  const state = useWorkbenchTerminal({
    runtime: props.runtime,
    renderer: props.renderer,
    projectId: "project-one",
    workspaceId: "workspace-one",
    enabled: true,
  })
  return props.show ? <TerminalPanel terminal={state} /> : <div testId="terminal-hidden" />
}

describe("M4 native terminal workbench", () => {
  it("spawns only after explicit user action with the native measured viewport", async () => {
    const record = { spawns: [], inputs: [], resizes: [], attachments: 0, releases: 0 } as {
      spawns: TerminalSpawnParams[]
      inputs: TerminalInputParams[]
      resizes: TerminalResizeParams[]
      attachments: number
      releases: number
    }
    const runtime = fakeRuntime(record)
    const root = createTestRoot()
    try {
      root.render(<Harness runtime={runtime} renderer={root.renderer} show />)
      await settle()
      expect(record.spawns).toHaveLength(0)

      click(root.renderer, "new-terminal")
      await settle()
      await settle()

      expect(record.spawns).toHaveLength(1)
      expect(record.spawns[0]?.userApproved).toBe(true)
      expect(record.spawns[0]?.rows).toBeGreaterThan(2)
      expect(record.spawns[0]?.cols).toBeGreaterThan(2)
      expect(root.renderer.getPaintedText().join("\n")).toContain("snapshot live")
    } finally {
      root.unmount()
    }
  })

  it("routes native keyboard bytes and resize measurements to the selected daemon terminal", async () => {
    const attached = attachmentSignal()
    const record = { spawns: [], inputs: [], resizes: [], attachments: 0, releases: 0 } as {
      spawns: TerminalSpawnParams[]
      inputs: TerminalInputParams[]
      resizes: TerminalResizeParams[]
      attachments: number
      releases: number
      attachmentSignals?: Array<() => void>
    }
    record.attachmentSignals = [attached.resolve]
    const runtime = fakeRuntime(record)
    const root = createTestRoot()
    try {
      root.render(<Harness runtime={runtime} renderer={root.renderer} show />)
      await settle()
      click(root.renderer, "new-terminal")
      await attached.settled
      const surface = root.renderer.findByTestId("terminal-surface")!

      root.renderer.nativeSimulateKeystrokes(surface.id, "h i enter")
      handleGpuixEvent({ elementId: surface.id, eventType: "terminalResize", rows: 30, cols: 100 }, root.renderer)
      await settle()

      expect(Buffer.concat(record.inputs.map((input) => Buffer.from(input.dataBase64, "base64"))).toString()).toBe("hi\r")
      expect(record.resizes).toContainEqual({ terminalId: terminal.id, rows: 30, cols: 100 })
    } finally {
      root.unmount()
    }
  })

  it("reattaches the daemon-owned terminal when its native panel remounts", async () => {
    const firstAttachment = attachmentSignal()
    const secondAttachment = attachmentSignal()
    const record = { spawns: [], inputs: [], resizes: [], attachments: 0, releases: 0 } as {
      spawns: TerminalSpawnParams[]
      inputs: TerminalInputParams[]
      resizes: TerminalResizeParams[]
      attachments: number
      releases: number
      attachmentSignals?: Array<() => void>
    }
    record.attachmentSignals = [firstAttachment.resolve, secondAttachment.resolve]
    const runtime = fakeRuntime(record)
    const root = createTestRoot()
    try {
      root.render(<Harness runtime={runtime} renderer={root.renderer} show />)
      await settle()
      click(root.renderer, "new-terminal")
      await firstAttachment.settled
      expect(record.attachments).toBe(1)

      root.render(<Harness runtime={runtime} renderer={root.renderer} show={false} />)
      root.render(<Harness runtime={runtime} renderer={root.renderer} show />)
      await secondAttachment.settled

      expect(record.attachments).toBe(2)
      expect(record.releases).toBeGreaterThan(0)
      expect(root.renderer.getPaintedText().join("\n")).toContain("snapshot live")
    } finally {
      root.unmount()
    }
  })
})
