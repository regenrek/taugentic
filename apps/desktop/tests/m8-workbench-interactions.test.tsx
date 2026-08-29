import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { Pressable } from "../src/ui/pressable.js"

describe("M8 workbench interaction semantics", () => {
  it("owns native click, Enter, and Space activation while exposing the current semantic state", () => {
    const root = createTestRoot()
    let activations = 0
    try {
      root.render(<Pressable testId="workbench-control" name="Workbench control" role="checkbox" checked selected expanded onPress={() => { activations += 1 }}><text>Control</text></Pressable>)
      const control = root.renderer.findByTestId("workbench-control")!
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(control.id) ?? []
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      root.renderer.nativeSimulateKeystrokes(control.id, "enter space")
      expect(activations).toBe(3)
      expect(root.renderer.getAutomationTree()).toContain('"role":"checkbox"')
      expect(root.renderer.getAutomationTree()).toContain('"name":"Workbench control"')
      expect(root.renderer.getAutomationTree()).toContain('"checked":true')
      expect(root.renderer.getAutomationTree()).toContain('"selected":true')
      expect(root.renderer.getAutomationTree()).toContain('"expanded":true')
    } finally {
      root.unmount()
    }
  })

  it("does not activate a disabled control", () => {
    const root = createTestRoot()
    let activations = 0
    try {
      root.render(<Pressable testId="disabled-workbench-control" name="Disabled workbench control" disabled onPress={() => { activations += 1 }}><text>Disabled</text></Pressable>)
      const control = root.renderer.findByTestId("disabled-workbench-control")!
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(control.id) ?? []
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      root.renderer.nativeSimulateKeystrokes(control.id, "enter space")
      expect(activations).toBe(0)
      expect(root.renderer.getAutomationTree()).toContain('"disabled":true')
    } finally {
      root.unmount()
    }
  })
})
