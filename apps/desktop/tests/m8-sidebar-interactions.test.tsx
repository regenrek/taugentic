import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { Sidebar } from "../src/features/sidebar/sidebar.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M8 sidebar interaction semantics", () => {
  it("uses one Pressable path for representative sidebar control classes", () => {
    const actions: string[] = []
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 360, height: 700 }}><Sidebar
        snapshot={{
          spaces: [{ id: "space-one", title: "Product" }],
          projects: [{ id: "project-one", title: "Desktop", spaceId: "space-one", workspaceIds: [] }],
          agents: [],
          conversations: [{ sessionId: "conversation-one", workspaceId: "workspace-one", title: "Design review", status: "idle", attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "project", projectId: "project-one" }, archived: false, pinned: false }],
        }}
        state={{ view: "spaces", filter: "", expandedSpaceIds: ["space-one"], selectedConversationId: "conversation-one" }}
        selectedProjectId="project-one"
        spaceTitle=""
        conversationTitle=""
        canCreateSpace={false}
        canCreateConversation={false}
        canOrganizeConversations
        dispatch={(action) => actions.push(`${action.type}:${"view" in action ? action.view : "spaceId" in action ? action.spaceId : "sessionId" in action ? action.sessionId : "projectId" in action ? action.projectId : ""}`)}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
        onSetPinnedConversation={(id, pinned) => actions.push(`${pinned ? "pin" : "unpin"}:${id}`)}
      /></div>)
      click(root, "sidebar-view-projects")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("space-space-one")!.id, "enter")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("project-project-one")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("conversation-entry-conversation-one")!.id, "enter")
      click(root, "pin-conversation-conversation-one")
      const disabled = root.renderer.findByTestId("create-space")!
      root.renderer.nativeSimulateKeystrokes(disabled.id, "enter space")
      click(root, "create-space")
      expect(actions).toEqual(["selectView:projects", "toggleSpace:space-one", "selectProject:project-one", "selectConversation:conversation-one", "pin:conversation-one"])
      expect(root.renderer.getAutomationTree()).toContain('"testId":"create-space","accessibility":{"role":"button","name":"Create space","disabled":true}')
      expect(root.renderer.getAutomationTree()).toContain('"testId":"space-space-one","accessibility":{"role":"button","name":"Product space","disabled":false,"expanded":true}')
      expect(root.renderer.getAutomationTree()).toContain('"testId":"conversation-entry-conversation-one","accessibility":{"role":"button","name":"Open conversation Design review","disabled":false,"selected":true}')
    } finally {
      root.unmount()
    }
  })
})
