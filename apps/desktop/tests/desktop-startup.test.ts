import { describe, expect, it } from "bun:test"

import { DesktopStartupPresentation } from "../src/desktop-startup.js"

describe("desktop startup", () => {
  it("renders the primary window before an unresolved workspace bootstrap", async () => {
    const startup = new DesktopStartupPresentation()
    let resolveBootstrap!: () => void
    const bootstrap = new Promise<void>((resolve) => { resolveBootstrap = resolve })
    const events: string[] = []

    const completion = startup.start({
      renderPrimaryWindow() { events.push("render") },
      bootstrapWorkspace() {
        events.push("bootstrap")
        return bootstrap
      },
    })

    expect(events).toEqual(["render", "bootstrap"])
    expect(startup.error()).toBeUndefined()
    resolveBootstrap()
    await completion
  })

  it("projects one actionable deferred bootstrap failure", async () => {
    const startup = new DesktopStartupPresentation()
    let notifications = 0
    const unsubscribe = startup.subscribe(() => { notifications += 1 })

    await startup.start({
      renderPrimaryWindow() {},
      bootstrapWorkspace: () => Promise.reject(new Error("unavailable")),
    })

    expect(startup.error()).toBe("Desktop startup could not be completed. Restart Taugentic and try again.")
    expect(notifications).toBe(1)
    unsubscribe()
  })

  it("returns one completion without rendering or bootstrapping twice", async () => {
    const startup = new DesktopStartupPresentation()
    let renders = 0
    let bootstraps = 0
    const dependencies = {
      renderPrimaryWindow() { renders += 1 },
      bootstrapWorkspace() {
        bootstraps += 1
        return Promise.resolve()
      },
    }

    const first = startup.start(dependencies)
    const second = startup.start(dependencies)

    expect(second).toBe(first)
    await first
    expect(renders).toBe(1)
    expect(bootstraps).toBe(1)
  })
})
