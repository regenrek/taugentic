import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"

import { Sidebar } from "../src/features/sidebar/sidebar.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M2 Space and Project Organization", () => {
  it("creates named spaces and moves only the selected project through keyboard-operable controls", () => {
    const actions: string[] = []
    const root = createTestRoot()
    const snapshot = {
      spaces: [{ id: "space-product", title: "Product" }],
      projects: [
        { id: "project-desktop", title: "Desktop", workspaceIds: ["workspace-desktop"] },
        { id: "project-other", title: "Other", workspaceIds: ["workspace-other"] },
      ],
      agents: [],
      conversations: [],
    }
    try {
      root.render(<Sidebar
        snapshot={snapshot}
        state={{ view: "spaces", filter: "", expandedSpaceIds: [] }}
        selectedProjectId="project-desktop"
        spaceTitle="Design"
        conversationTitle=""
        canCreateSpace
        canCreateConversation={false}
        canOrganizeProjects
        dispatch={() => {}}
        onSpaceTitleChange={() => {}}
        onCreateSpace={() => actions.push("create:Design")}
        onSetProjectSpace={(projectId, spaceId) => actions.push(`move:${projectId}:${spaceId ?? "ungrouped"}`)}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
      />)
      click(root, "create-space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("new-space-title")!.id, "enter")
      expect(actions).toEqual(["create:Design", "create:Design"])
      expect(root.renderer.getAutomationTree()).toContain('"name":"Create space"')

      root.render(<Sidebar
        snapshot={snapshot}
        state={{ view: "projects", filter: "", expandedSpaceIds: [] }}
        selectedProjectId="project-desktop"
        conversationTitle=""
        canCreateConversation={false}
        canOrganizeProjects
        dispatch={() => {}}
        onSetProjectSpace={(projectId, spaceId) => actions.push(`move:${projectId}:${spaceId ?? "ungrouped"}`)}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
      />)
      click(root, "set-project-space-project-desktop-space-product")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("set-project-space-project-desktop-ungrouped")!.id, "space")
      expect(root.renderer.findByTestId("project-space-controls-project-other")).toBeUndefined()
      expect(actions).toEqual([
        "create:Design",
        "create:Design",
        "move:project-desktop:space-product",
        "move:project-desktop:ungrouped",
      ])
      expect(root.renderer.getAutomationTree()).toContain('"name":"Move project to Product"')
      expect(root.renderer.getAutomationTree()).toContain('"name":"Move project to Ungrouped"')
    } finally {
      root.unmount()
    }
  })

  it("makes organization controls unavailable without a ready shell capability", () => {
    const root = createTestRoot()
    try {
      root.render(<Sidebar
        snapshot={{ spaces: [{ id: "space-product", title: "Product" }], projects: [{ id: "project-desktop", title: "Desktop", workspaceIds: [] }], agents: [], conversations: [] }}
        state={{ view: "spaces", filter: "", expandedSpaceIds: [] }}
        selectedProjectId="project-desktop"
        spaceTitle="Design"
        conversationTitle=""
        canCreateSpace={false}
        canCreateConversation={false}
        canOrganizeProjects={false}
        dispatch={() => {}}
        onCreateSpace={() => { throw new Error("disabled create must not run") }}
        onSetProjectSpace={() => { throw new Error("disabled move must not run") }}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
      />)
      expect(root.renderer.getAutomationTree()).toContain('"name":"Create space","disabled":true')
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("create-space")!.id, "enter")
      root.render(<Sidebar
        snapshot={{ spaces: [{ id: "space-product", title: "Product" }], projects: [{ id: "project-desktop", title: "Desktop", workspaceIds: [] }], agents: [], conversations: [] }}
        state={{ view: "projects", filter: "", expandedSpaceIds: [] }}
        selectedProjectId="project-desktop"
        conversationTitle=""
        canCreateConversation={false}
        canOrganizeProjects={false}
        dispatch={() => {}}
        onSetProjectSpace={() => { throw new Error("disabled move must not run") }}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
      />)
      expect(root.renderer.getAutomationTree()).toContain('"name":"Move project to Product","disabled":true')
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("set-project-space-project-desktop-space-product")!.id, "enter")
    } finally {
      root.unmount()
    }
  })
})
