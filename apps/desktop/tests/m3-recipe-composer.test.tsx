import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"
import { useMemo, useState } from "react"

import { ConversationPanel } from "../src/features/workspace-layout/panels.js"
import { createCommandDispatcher } from "../src/features/commands/registry.js"
import { desktopRuntime, startSelectedRun, workspaceShell } from "../src/features/runtime/workspace-shell.js"
import { recipesQuery, recipesQueryKey } from "../src/platform/daemon/recipes-query.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M3 Recipe Composer", () => {
  it("uses the daemon catalog as its one typed query projection", async () => {
    const calls: string[] = []
    const runtime = {
      listRecipes: async () => {
        calls.push("list")
        return { recipes: [] }
      },
    }
    const query = recipesQuery(runtime)
    expect([...query.queryKey]).toEqual([...recipesQueryKey])
    await expect(runtime.listRecipes()).resolves.toEqual({ recipes: [] })
    expect(calls).toEqual(["list"])
  })

  it("keeps recipe choice transient, supports explicit clear, and dispatches its canonical ID", () => {
    const starts: Array<string | undefined> = []
    const root = createTestRoot()
    try {
      function Harness() {
        const [recipeId, setRecipeId] = useState<string>()
        const dispatcher = useMemo(() => createCommandDispatcher(new DesktopSettings(), () => ({ canStart: true, canCancel: false }), {
          openSettings() {}, focusPanel() {}, toggleTheme() {}, startRun() { starts.push(recipeId) }, cancelRun() {},
        }), [recipeId])
        return <div style={{ width: 900, height: 700 }}><ConversationPanel
          title="Recipe test"
          selectedConversationId="session-recipe-test"
          transcriptRows={[]}
          transcriptLoading={false}
          hasOlderTranscript={false}
          loadingOlderTranscript={false}
          onLoadOlderTranscript={() => {}}
          messages={[]}
          objective="Run the recipe"
          attachments={[]}
          onObjectiveChange={() => {}}
          onRemoveAttachment={() => {}}
          recipes={[{ id: "review", name: "Review", description: "Review this change", contract: "debug", promptTemplate: "ignored" }]}
          recipesLoading={false}
          selectedRecipeId={recipeId}
          onSelectRecipe={setRecipeId}
          commands={dispatcher}
        /></div>
      }
      root.render(<Harness />)
      click(root, "recipe-picker")
      expect(root.renderer.findByTestId("recipe-option-review")).toBeDefined()
      click(root, "recipe-option-review")
      expect(root.renderer.getPaintedText()).toContain("Review this change")
      click(root, "start-run")
      expect(starts).toEqual(["review"])
      click(root, "clear-recipe")
      click(root, "start-run")
      expect(starts).toEqual(["review", undefined])
      expect(root.renderer.getAutomationTree()).toContain('"name":"Clear recipe","disabled":true')
    } finally {
      root.unmount()
    }
  })

  it("sends only the transient selected recipe ID in the canonical start command", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      startRun(commandJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalStartRun = bridge.startRun
    const originalSubscribeRunEvents = bridge.subscribeRunEvents
    const commands: Array<Record<string, unknown>> = []
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: { status: "ready", invalidated: false, foreignRuntimeRestricted: false } })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-recipe-command" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-recipe" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth-recipe", modelId: "model-recipe" } })
      bridge.startRun = async (commandJson) => {
        commands.push(JSON.parse(commandJson) as Record<string, unknown>)
        return JSON.stringify({ id: `run-recipe-${commands.length}` })
      }
      bridge.subscribeRunEvents = async () => JSON.stringify({ events: [] })

      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Run the selected recipe" })
      await startSelectedRun(desktopRuntime, workspaceShell, "review")
      workspaceShell.send({ type: "RUN_CANCELLED", runId: "run-recipe-1" })

      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Run without a recipe" })
      await startSelectedRun(desktopRuntime, workspaceShell)

      expect(commands).toEqual([
        {
          objective: "Run the selected recipe",
          selection: { runtimeProfileId: "runtime-recipe", authProfileId: "auth-recipe", modelId: "model-recipe" },
          attachments: [],
          recipeId: "review",
        },
        {
          objective: "Run without a recipe",
          selection: { runtimeProfileId: "runtime-recipe", authProfileId: "auth-recipe", modelId: "model-recipe" },
          attachments: [],
        },
      ])
    } finally {
      bridge.startRun = originalStartRun
      bridge.subscribeRunEvents = originalSubscribeRunEvents
      workspaceShell.stop()
    }
  })

  it("does not silently clear a selected recipe absent from the latest daemon catalog", () => {
    const root = createTestRoot()
    try {
      const dispatcher = createCommandDispatcher(new DesktopSettings(), () => ({ canStart: false, canCancel: false }), {
        openSettings() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {},
      })
      root.render(<div style={{ width: 900, height: 700 }}><ConversationPanel title="Unavailable recipe" selectedConversationId="session-recipe-unavailable" transcriptRows={[]} transcriptLoading={false} hasOlderTranscript={false} loadingOlderTranscript={false} onLoadOlderTranscript={() => {}} messages={[]} objective="" attachments={[]} onObjectiveChange={() => {}} onRemoveAttachment={() => {}} recipes={[]} recipesLoading={false} selectedRecipeId="removed-by-daemon" onSelectRecipe={() => {}} commands={dispatcher} /></div>)
      expect(root.renderer.findByTestId("recipe-unavailable")).toBeDefined()
      expect(root.renderer.getPaintedText()).toContain("Unavailable recipe: removed-by-daemon")
    } finally {
      root.unmount()
    }
  })
})
