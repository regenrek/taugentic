export type DesktopStartupDependencies = {
  renderPrimaryWindow(): void
  bootstrapWorkspace(): Promise<void>
}

/** Desktop presentation owns window creation and the visible deferred-bootstrap failure. */
export class DesktopStartupPresentation {
  #completion: Promise<void> | undefined
  #error: string | undefined
  #listeners = new Set<() => void>()

  error(): string | undefined {
    return this.#error
  }

  subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }

  start({ renderPrimaryWindow, bootstrapWorkspace }: DesktopStartupDependencies): Promise<void> {
    if (this.#completion) return this.#completion
    let complete!: () => void
    this.#completion = new Promise<void>((resolve) => { complete = resolve })
    renderPrimaryWindow()
    bootstrapWorkspace().then(complete, () => {
      this.#error = "Desktop startup could not be completed. Restart Taugentic and try again."
      for (const listener of this.#listeners) listener()
      complete()
    })
    return this.#completion
  }
}
