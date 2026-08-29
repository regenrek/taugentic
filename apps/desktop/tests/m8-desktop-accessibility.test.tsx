import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { ProductState } from "../src/app/product-state.js"
import { ProjectTrustConfirmation } from "../src/app/project-trust-confirmation.js"
import { RuntimeRoutePicker } from "../src/features/auth-profiles/auth-profiles.js"
import { Sidebar } from "../src/features/sidebar/sidebar.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"

describe("M8 desktop accessibility", () => {
  it("renders canonical product facts with native semantic roles and actionable detail", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<div><ProductState kind="empty" title="No conversations" detail="Create a project conversation first." /><ProductState kind="loading" title="Loading activity" detail="Waiting for the daemon query." /><ProductState kind="offline" title="Daemon unavailable" detail="Start the daemon and reopen this window." /><ProductState kind="error" title="Transcript query failed" detail="The daemon returned an invalid cursor." /><ProductState kind="destructive" title="Trust this folder" detail="This grants file access." /></div>)
      const tree = renderer.getAutomationTree()
      expect(tree).toContain("No conversations")
      expect(tree).toContain("Transcript query failed")
      expect(tree).toContain("The daemon returned an invalid cursor.")
      expect(tree).toContain("Trust this folder")
    } finally { unmount() }
  })

  it("keeps settings malformed data actionable without silently replacing it", async () => {
    const settings = new DesktopSettings()
    let reads = 0
    const persistence = { read: async () => { reads += 1; return "{" }, write: async () => {} }
    const first = settings.initialize(persistence)
    const second = settings.initialize({ read: async () => { throw new Error("second initialization must not own loading") }, write: async () => {} })
    expect(second).toBe(first)
    await first
    expect(reads).toBe(1)
    expect(settings.error()).toBe("Desktop settings could not be loaded. Fix or remove the local settings document before changing preferences.")
    expect(settings.appearance().theme).toBe("dark")
  })

  it("does not persist an unconfigured workspace read", async () => {
    const writes: string[] = []
    const settings = new DesktopSettings()
    await settings.initialize({ read: async () => null, write: async (documentJson) => { writes.push(documentJson) } })

    expect(settings.presentation("workspace-unconfigured-read")).toBeUndefined()
    expect(writes).toEqual([])
  })

  it("activates trust, route and sidebar controls with Space through GPUIX native input", () => {
    const decisions: boolean[] = []
    const actions: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<div>
        <ProjectTrustConfirmation onDecision={(accepted) => decisions.push(accepted)} />
        <RuntimeRoutePicker snapshot={undefined} draft={undefined} pendingAuthMethodIds={[]} onDraft={() => {}} onLogin={() => {}} onLogout={() => {}} onPreferences={() => {}} />
        <Sidebar snapshot={{ spaces: [{ id: "space-a", title: "Alpha" }], projects: [], conversations: [], agents: [] }} state={{ view: "spaces", filter: "", expandedSpaceIds: [] }} conversationTitle="" canCreateConversation={false} onConversationTitleChange={() => {}} onCreateConversation={() => {}} dispatch={(action) => actions.push(action.type)} />
      </div>)
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("confirm-project-trust")!.id, "space")
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("runtime-route-toggle")!.id, "space")
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("space-space-a")!.id, "space")
      expect(decisions).toEqual([true])
      expect(renderer.findByTestId("runtime-route-options")).toBeDefined()
      expect(actions).toEqual(["toggleSpace"])
    } finally { unmount() }
  })
})
