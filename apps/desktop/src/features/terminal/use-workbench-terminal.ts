import type { NativeRenderer } from "@regenrek/gpuix-react"
import type {
  ProjectId,
  TerminalSessionId,
  TerminalSessionSummary,
  WorkspaceId,
} from "@taugentic/desktop-protocol"
import { useCallback, useEffect, useRef, useState } from "react"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"

type TerminalViewport = { rows: number; cols: number }

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

function validViewport(rows: unknown, cols: unknown): TerminalViewport | undefined {
  if (!Number.isInteger(rows) || !Number.isInteger(cols)) return undefined
  if (Number(rows) < 2 || Number(cols) < 2) return undefined
  return { rows: Number(rows), cols: Number(cols) }
}

export function useWorkbenchTerminal(input: {
  runtime: DesktopRuntime
  renderer: NativeRenderer
  projectId?: ProjectId
  workspaceId?: WorkspaceId
  enabled: boolean
}) {
  const [terminals, setTerminals] = useState<readonly TerminalSessionSummary[]>([])
  const [selectedTerminalId, setSelectedTerminalId] = useState<TerminalSessionId>()
  const [surfaceElementId, setSurfaceElementId] = useState<number>()
  const [viewport, setViewport] = useState<TerminalViewport>()
  const [snapshotTruncated, setSnapshotTruncated] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const scopeRef = useRef("")
  const attachmentGeneration = useRef(0)
  const subscriptionQueue = useRef(Promise.resolve())
  const inputQueue = useRef(Promise.resolve())
  const pendingResize = useRef<{ terminalId: TerminalSessionId; viewport: TerminalViewport } | undefined>(undefined)
  const resizeRunning = useRef(false)

  const scope = input.projectId && input.workspaceId
    ? `${input.projectId}:${input.workspaceId}`
    : ""
  scopeRef.current = scope

  const refresh = useCallback(async () => {
    if (!input.enabled || !input.projectId || !input.workspaceId) return
    const requestedScope = `${input.projectId}:${input.workspaceId}`
    try {
      const result = await input.runtime.listTerminals({
        projectId: input.projectId,
        workspaceId: input.workspaceId,
      })
      if (scopeRef.current !== requestedScope) return
      setTerminals(result.terminals)
      setSelectedTerminalId((current) => (
        current && result.terminals.some((terminal) => terminal.id === current)
          ? current
          : undefined
      ))
    } catch (cause) {
      if (scopeRef.current === requestedScope) {
        setError(errorMessage(cause, "Terminal sessions could not be loaded."))
      }
    }
  }, [input.enabled, input.projectId, input.runtime, input.workspaceId])

  useEffect(() => {
    setSelectedTerminalId(undefined)
    setTerminals([])
    setSnapshotTruncated(false)
    setError(undefined)
    if (scope) void refresh()
  }, [refresh, scope])

  useEffect(() => {
    const generation = ++attachmentGeneration.current
    let cancelled = false
    const write = input.renderer.terminalWrite
    const reset = input.renderer.terminalReset
    const canAttach = input.enabled && selectedTerminalId && surfaceElementId

    const switchSubscription = async (): Promise<void> => {
      input.runtime.releaseTerminalSubscription()
      if (cancelled || attachmentGeneration.current !== generation || !canAttach) return
      if (!write || !reset) {
        setError("The native terminal renderer is unavailable.")
        return
      }
      reset.call(input.renderer, surfaceElementId)
      setSnapshotTruncated(false)
      try {
        await input.runtime.subscribeTerminal(selectedTerminalId, {
          attached(initial) {
            if (cancelled || attachmentGeneration.current !== generation) return
            setSnapshotTruncated(initial.snapshotTruncated)
            if (initial.snapshotBase64) {
              write.call(input.renderer, surfaceElementId, initial.snapshotBase64)
            }
          },
          event(update) {
            if (cancelled || attachmentGeneration.current !== generation) return
            if (update.event.kind === "output") {
              write.call(input.renderer, surfaceElementId, update.event.dataBase64)
              return
            }
            setTerminals((current) => current.map((terminal) => (
              terminal.id === update.terminalId
                ? { ...terminal, status: "exited" }
                : terminal
            )))
          },
          failed(message) {
            if (!cancelled && attachmentGeneration.current === generation) setError(message)
          },
        })
      } catch (cause) {
        if (!cancelled && attachmentGeneration.current === generation) {
          setError(errorMessage(cause, "The terminal session could not be attached."))
        }
      }
    }

    subscriptionQueue.current = subscriptionQueue.current.then(switchSubscription, switchSubscription)
    return () => {
      cancelled = true
      subscriptionQueue.current = subscriptionQueue.current.then(() => {
        if (attachmentGeneration.current === generation) input.runtime.releaseTerminalSubscription()
      })
    }
  }, [input.enabled, input.renderer, input.runtime, selectedTerminalId, surfaceElementId])

  const setTerminalSurface = useCallback((elementId?: number) => {
    setSurfaceElementId(elementId)
  }, [])

  const selectTerminal = useCallback((terminalId: TerminalSessionId) => {
    setError(undefined)
    setSelectedTerminalId(terminalId)
  }, [])

  const spawn = useCallback(async () => {
    if (!input.enabled || !input.projectId || !input.workspaceId || !viewport) return
    setBusy(true)
    setError(undefined)
    try {
      const result = await input.runtime.spawnTerminal({
        projectId: input.projectId,
        workspaceId: input.workspaceId,
        rows: viewport.rows,
        cols: viewport.cols,
        userApproved: true,
      })
      setTerminals((current) => [
        ...current.filter((terminal) => terminal.id !== result.terminal.id),
        result.terminal,
      ])
      setSelectedTerminalId(result.terminal.id)
    } catch (cause) {
      setError(errorMessage(cause, "The terminal session could not be created."))
    } finally {
      setBusy(false)
    }
  }, [input.enabled, input.projectId, input.runtime, input.workspaceId, viewport])

  const close = useCallback(async (terminalId: TerminalSessionId) => {
    setBusy(true)
    setError(undefined)
    try {
      const result = await input.runtime.closeTerminal({ terminalId })
      setTerminals((current) => current.map((terminal) => (
        terminal.id === terminalId ? result.terminal : terminal
      )))
      setSelectedTerminalId((current) => current === terminalId ? undefined : current)
    } catch (cause) {
      setError(errorMessage(cause, "The terminal session could not be closed."))
    } finally {
      setBusy(false)
    }
  }, [input.runtime])

  const sendInput = useCallback((dataBase64?: string) => {
    if (!selectedTerminalId || !dataBase64) return
    const terminalId = selectedTerminalId
    const send = async (): Promise<void> => {
      try {
        await input.runtime.terminalInput({ terminalId, dataBase64 })
      } catch (cause) {
        setError(errorMessage(cause, "Terminal input could not be sent."))
      }
    }
    inputQueue.current = inputQueue.current.then(send, send)
  }, [input.runtime, selectedTerminalId])

  const flushResize = useCallback(async () => {
    if (resizeRunning.current) return
    resizeRunning.current = true
    try {
      while (pendingResize.current) {
        const request = pendingResize.current
        pendingResize.current = undefined
        try {
          const result = await input.runtime.resizeTerminal({
            terminalId: request.terminalId,
            rows: request.viewport.rows,
            cols: request.viewport.cols,
          })
          setTerminals((current) => current.map((terminal) => (
            terminal.id === request.terminalId ? result.terminal : terminal
          )))
        } catch (cause) {
          setError(errorMessage(cause, "The terminal could not be resized."))
        }
      }
    } finally {
      resizeRunning.current = false
    }
  }, [input.runtime])

  const resize = useCallback((rows: unknown, cols: unknown) => {
    const measured = validViewport(rows, cols)
    if (!measured) return
    setViewport(measured)
    if (!selectedTerminalId) return
    pendingResize.current = { terminalId: selectedTerminalId, viewport: measured }
    void flushResize()
  }, [flushResize, selectedTerminalId])

  return {
    terminals,
    selectedTerminalId,
    selectedTerminal: terminals.find((terminal) => terminal.id === selectedTerminalId),
    viewport,
    snapshotTruncated,
    busy,
    error,
    canSpawn: input.enabled && Boolean(input.projectId) && Boolean(input.workspaceId) && Boolean(viewport) && !busy,
    setTerminalSurface,
    selectTerminal,
    spawn,
    close,
    sendInput,
    resize,
    refresh,
  }
}

export type WorkbenchTerminalState = ReturnType<typeof useWorkbenchTerminal>
