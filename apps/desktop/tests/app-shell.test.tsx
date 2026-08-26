import { describe, expect, it } from "bun:test"

import { launchM1Desktop } from "./m1-runtime.js"

describe("native desktop shell", () => {
  it("runs the credentials-free native session lifecycle and native interactions", async () => {
    const desktop = await launchM1Desktop()
    const { app } = desktop

    try {
      await app.getByText("TAUGENTIC").waitFor({ timeoutMs: 8_000 })
      await app.getByTestId("workspace-shell").waitFor({ timeoutMs: 8_000 })
      await app.getByText("DAEMON READY").waitFor({ timeoutMs: 8_000 })
      await app.getByTestId("workspace-sidebar").waitFor({ timeoutMs: 8_000 })
      await app.getByTestId("open-project").waitFor({ timeoutMs: 8_000 })
      await app.getByTestId("sidebar-view-projects").click()
      await app.getByTestId("sidebar-open-project").waitFor({ timeoutMs: 8_000 })
      expect((await app.call("getAllText", {})).text).toContain("CONVERSATIONS")

      await app.getByTestId("close-daemon").click()
      await app.getByText("DAEMON CLOSED").waitFor({ timeoutMs: 8_000 })
    } finally {
      await desktop.close()
    }
  }, 90_000)
})
