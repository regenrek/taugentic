import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import React from "react"

describe("M8 GPUIX canvas consumer contract", () => {
  it("mounts the generic bounded canvas intrinsic without a product bridge", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(
        <canvas
          testId="generic-canvas"
          style={{ width: 120, height: 60 }}
          motion="paused"
          commands={[
            { type: "line", id: "edge", from: { x: 0, y: 0 }, to: { x: 1, y: 1 }, width: 0.04, color: "#55ccff" },
            { type: "circle", id: "node", center: { x: 0.5, y: 0.5 }, radius: 0.12, color: "#ffffff" },
          ]}
        />,
      )

      const canvas = renderer.findByTestId("generic-canvas")
      expect(canvas).toBeDefined()
      expect(renderer.getElementBounds(canvas!.id)).toEqual([0, 0, 120, 60])
    } finally {
      unmount()
    }
  })
})
