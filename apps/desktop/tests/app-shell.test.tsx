import { launch } from "@gpuix/react/automation"
import { describe, it } from "bun:test"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

describe("native desktop shell", () => {
  it("launches the production host and accepts native interaction", async () => {
    const desktopRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
    const app = await launch({ command: "bun", args: ["src/main.tsx"], cwd: desktopRoot })

    try {
      await app.getByText("TAUGENTIC").waitFor({ timeoutMs: 30_000 })
      await app.getByTestId("nav-approvals").click()
    } finally {
      await app.close()
    }
  })
})
