import { execFileSync } from "node:child_process"
import { writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { launchM1Desktop } from "./m1-runtime.js"

const readyBudgetMs = 10_000
const closeBudgetMs = 5_000
const startedAt = performance.now()
const desktop = await launchM1Desktop()

try {
  await desktop.app.getByText("DAEMON READY").waitFor({ timeoutMs: readyBudgetMs })
  const readyMs = performance.now() - startedAt
  const closedAt = performance.now()
  await desktop.app.getByTestId("close-daemon").click()
  await desktop.app.getByText("DAEMON CLOSED").waitFor({ timeoutMs: closeBudgetMs })
  const closeMs = performance.now() - closedAt

  if (readyMs > readyBudgetMs || closeMs > closeBudgetMs) {
    throw new Error("M1 release measurement exceeded its declared budget")
  }

  const result = {
    schemaVersion: 1,
    build: "release",
    fixture: "credentials-free-isolated-local-daemon",
    sampleCount: 1,
    commit: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
    device: { osClass: `${process.platform}-${process.arch}` },
    budgetsMs: { ready: readyBudgetMs, close: closeBudgetMs },
    measurementsMs: { ready: readyMs, close: closeMs },
    authenticatedProviderMetrics: "N/A: manual authenticated-profile walkthrough only",
  }
  await writeFile(join(tmpdir(), `taugentic-m1-measure-${process.pid}.json`), JSON.stringify(result))
  console.log("M1 release measurement passed; result stored in ignored temporary storage.")
} finally {
  await desktop.close()
}
