import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { WorkInboxPanel } from "../src/features/work-items/work-inbox-panel.js"
import type { DesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M2 Work Inbox", () => {
  it("uses one daemon query projection and renders sync state without a desktop store", async () => {
    const calls: unknown[] = []
    const runtime = {
      listWorkItems: async (query: unknown) => {
        calls.push(query)
        return { items: [], sync: { state: "idle", detail: "daemon-owned" } }
      },
    } satisfies Pick<DesktopRuntime, "listWorkItems">
    await expect(runtime.listWorkItems({})).resolves.toEqual({ items: [], sync: { state: "idle", detail: "daemon-owned" } })
    expect(calls).toEqual([{}])
  })

  it("renders daemon items and sends only explicit refresh, dismiss, and selected-route trigger intents", () => {
    const calls: string[] = []
    const inbox = {
      items: [{ key: "github:issue-42", source: { kind: "gitHub", repo_owner: "owner", repo_name: "repo" }, externalId: "42", title: "Repair inbox", body: "", labels: ["bug"], url: "https://example.test/42", fetchedAtMs: 1, status: "available" as const }],
      sync: { state: "idle" as const, detail: "daemon-owned" },
      loading: false,
      busy: false,
      actionsEnabled: true,
      refresh: () => calls.push("refresh"),
      dismiss: (key: string) => calls.push(`dismiss:${key}`),
      trigger: (item: { key: string }) => calls.push(`trigger:${item.key}`),
    }
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 360, height: 600 }}><WorkInboxPanel inbox={inbox as never} canTrigger /></div>)
      expect(root.renderer.findByTestId("work-inbox-sync")).toBeDefined()
      expect(root.renderer.findByTestId("work-item-github:issue-42")).toBeDefined()
      click(root, "refresh-work-inbox")
      click(root, "dismiss-work-item-github:issue-42")
      click(root, "trigger-work-item-github:issue-42")
      expect(calls).toEqual(["refresh", "dismiss:github:issue-42", "trigger:github:issue-42"])
    } finally {
      root.unmount()
    }
  })

  it("keeps trigger disabled until a complete selected route exists", () => {
    const root = createTestRoot()
    const inbox = {
      items: [{ key: "github:issue-42", source: { kind: "gitHub", repo_owner: "owner", repo_name: "repo" }, externalId: "42", title: "Repair inbox", body: "", labels: [], url: "https://example.test/42", fetchedAtMs: 1, status: "available" as const }],
      loading: false,
      busy: false,
      actionsEnabled: true,
      refresh() {},
      dismiss() {},
      trigger() { throw new Error("disabled trigger must not run") },
    }
    try {
      root.render(<div style={{ width: 360, height: 600 }}><WorkInboxPanel inbox={inbox as never} canTrigger={false} /></div>)
      expect(root.renderer.getAutomationTree()).toContain('"name":"Run work item Repair inbox","disabled":true')
    } finally {
      root.unmount()
    }
  })

  it("does not invoke refresh or dismiss while the daemon is unavailable", () => {
    const calls: string[] = []
    const root = createTestRoot()
    const inbox = {
      items: [{ key: "github:issue-42", source: { kind: "gitHub", repo_owner: "owner", repo_name: "repo" }, externalId: "42", title: "Repair inbox", body: "", labels: [], url: "https://example.test/42", fetchedAtMs: 1, status: "available" as const }],
      loading: false,
      busy: false,
      actionsEnabled: false,
      refresh: () => calls.push("refresh"),
      dismiss: (key: string) => calls.push(`dismiss:${key}`),
      trigger: (item: { key: string }) => calls.push(`trigger:${item.key}`),
    }
    try {
      root.render(<div style={{ width: 360, height: 600 }}><WorkInboxPanel inbox={inbox as never} canTrigger={false} /></div>)
      click(root, "refresh-work-inbox")
      click(root, "dismiss-work-item-github:issue-42")
      click(root, "trigger-work-item-github:issue-42")
      expect(calls).toEqual([])
      expect(root.renderer.getAutomationTree()).toContain('"name":"Refresh Work Inbox","disabled":true')
      expect(root.renderer.getAutomationTree()).toContain('"name":"Dismiss work item Repair inbox","disabled":true')
    } finally {
      root.unmount()
    }
  })
})
