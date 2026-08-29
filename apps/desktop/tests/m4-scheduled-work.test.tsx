import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "bun:test"

import type { ScheduledWorkOccurrence, SessionId } from "@taugentic/desktop-protocol"

import { ScheduledWorkPanel } from "../src/features/scheduled-work/scheduled-work-panel.js"
import { useScheduledWork } from "../src/features/scheduled-work/use-scheduled-work.js"
import type { DesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../src/platform/daemon/query-client.js"
import { scheduledWorkQueryKey } from "../src/platform/daemon/scheduled-work-query.js"

type ScheduledWorkRuntime = Pick<DesktopRuntime, "createScheduledWork" | "listScheduledWork" | "cancelScheduledWork">

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
}

function Harness(props: { runtime: ScheduledWorkRuntime; openRun(runId: string): void; sessionId?: SessionId }) {
  const scheduledWork = useScheduledWork({
    runtime: props.runtime,
    enabled: true,
    sessionId: props.sessionId,
    selection: { runtimeProfileId: "profile-one", authProfileId: "auth-one", modelId: "model-one" },
  })
  return <ScheduledWorkPanel scheduledWork={scheduledWork} onOpenRun={props.openRun} />
}

describe("M4 scheduled work", () => {
  it("creates with only user objective, explicit selection, and due time then refreshes the one projection", async () => {
    const requests: unknown[] = []
    const runtime: ScheduledWorkRuntime = {
      async listScheduledWork() { return { occurrences: [] } },
      async createScheduledWork(request) {
        requests.push(request)
        return {
          definition: {} as never,
          occurrence: { id: "occurrence-one", scheduledWorkId: "scheduled-one", dueAtMs: request.dueAtMs, state: { kind: "pending" } },
        }
      },
      async cancelScheduledWork() {},
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={"session-one" as SessionId} openRun={() => undefined} /></QueryClientProvider>)
      await settle()
      const objective = root.renderer.findByTestId("scheduled-work-objective")!
      const dueAt = root.renderer.findByTestId("scheduled-work-due-at-ms")!
      root.renderer.nativeSimulateInput(objective.id, "Review the pull request")
      root.renderer.nativeSimulateInput(dueAt.id, "1780000000000")
      click(root.renderer, "create-scheduled-work")
      await settle()

      expect(requests).toEqual([{
        objective: "Review the pull request",
        selection: { runtimeProfileId: "profile-one", authProfileId: "auth-one", modelId: "model-one" },
        dueAtMs: "1780000000000",
      }])
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })

  it("cancels only cancellable occurrences and opens linked runs through the supplied navigation", async () => {
    const occurrence: ScheduledWorkOccurrence = {
      id: "occurrence-one",
      scheduledWorkId: "scheduled-one",
      dueAtMs: "1780000000000",
      state: { kind: "claimed", run_id: "run-one" },
    }
    const cancelled: string[] = []
    const opened: string[] = []
    const runtime: ScheduledWorkRuntime = {
      async listScheduledWork() { return { occurrences: [occurrence] } },
      async createScheduledWork() { throw new Error("Create is outside this test.") },
      async cancelScheduledWork(request) { cancelled.push(request.occurrenceId) },
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={"session-one" as SessionId} openRun={(runId) => opened.push(runId)} /></QueryClientProvider>)
      await settle()
      await settle()
      click(root.renderer, "open-scheduled-work-run-occurrence-one")
      click(root.renderer, "cancel-scheduled-work-occurrence-one")
      await settle()

      expect(opened).toEqual(["run-one"])
      expect(cancelled).toEqual(["occurrence-one"])
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })

  it("isolates the sole Scheduled Work cache by selected session and clears its projection without a session", () => {
    const first: ScheduledWorkOccurrence = {
      id: "occurrence-first",
      scheduledWorkId: "scheduled-first",
      dueAtMs: "1780000000000",
      state: { kind: "pending" },
    }
    const second: ScheduledWorkOccurrence = {
      id: "occurrence-second",
      scheduledWorkId: "scheduled-second",
      dueAtMs: "1780000000001",
      state: { kind: "pending" },
    }
    const runtime: ScheduledWorkRuntime = {
      async listScheduledWork() { return { occurrences: [] } },
      async createScheduledWork() { throw new Error("Create is outside this test.") },
      async cancelScheduledWork() {},
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      desktopQueryClient.setQueryData(scheduledWorkQueryKey("session-first" as SessionId), { occurrences: [first] })
      desktopQueryClient.setQueryData(scheduledWorkQueryKey("session-second" as SessionId), { occurrences: [second] })
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={"session-first" as SessionId} openRun={() => undefined} /></QueryClientProvider>)
      expect(root.renderer.findByTestId("scheduled-work-occurrence-first")).toBeDefined()
      expect(root.renderer.findByTestId("scheduled-work-occurrence-second")).toBeUndefined()

      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={"session-second" as SessionId} openRun={() => undefined} /></QueryClientProvider>)
      expect(root.renderer.findByTestId("scheduled-work-occurrence-first")).toBeUndefined()
      expect(root.renderer.findByTestId("scheduled-work-occurrence-second")).toBeDefined()

      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={undefined} openRun={() => undefined} /></QueryClientProvider>)
      expect(root.renderer.findByTestId("scheduled-work-occurrence-second")).toBeUndefined()
      expect(root.renderer.findByTestId("scheduled-work-empty")).toBeDefined()
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })

  it("names the scheduling inputs for native accessibility", () => {
    const runtime: ScheduledWorkRuntime = {
      async listScheduledWork() { return { occurrences: [] } },
      async createScheduledWork() { throw new Error("Create is outside this test.") },
      async cancelScheduledWork() {},
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} sessionId={"session-one" as SessionId} openRun={() => undefined} /></QueryClientProvider>)
      const tree = root.renderer.getAutomationTree()
      expect(tree).toContain('"name":"Work objective"')
      expect(tree).toContain('"name":"Due time in Unix milliseconds"')
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })
})
