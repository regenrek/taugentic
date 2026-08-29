import { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import type {
  DesktopDaemonLifecycleProjection,
  DaemonNavigationIntent,
  ForkRunRequest,
  ForkRunResult,
  ContinueRunRequest,
  ContinueRunResult,
  SwitchAccountAndResumeRequest,
  SwitchAccountAndResumeResult,
  JoinRunRequest,
  JoinRunResult,
  ListNativeRunsRequest,
  ListNativeRunsResult,
  RunLineageGraphRequest,
  RunLineageGraphResult,
  SpawnRunRequest,
  SpawnRunResult,
  TerminalAttachResult,
  TerminalCloseParams,
  TerminalCloseResult,
  TerminalEventParams,
  TerminalInputParams,
  TerminalInputResult,
  TerminalListParams,
  TerminalListResult,
  TerminalResizeParams,
  TerminalResizeResult,
  TerminalSessionId,
  TerminalSpawnParams,
  TerminalSpawnResult,
  VoiceEvent,
  VoicePermissionState,
  WorkItemDismissParams,
  WorkItemDismissResult,
  WorkItemListQuery,
  WorkItemListResult,
  WorkItemRefreshParams,
  WorkItemTriggerParams,
  WorkItemTriggerResult,
  RecipeListResponse,
  NavigationSnapshot,
  CancelScheduledWorkRequest,
  CreateScheduledWorkRequest,
  CreateScheduledWorkResult,
  ListScheduledWorkResult,
  InspectPluginPackageRequest,
  InstallPluginPackageRequest,
  InstallPluginPackageResult,
  ListPluginInstallationsResult,
  PluginInspection,
  UninstallPluginRequest,
  DaemonDiagnostics,
} from "@taugentic/desktop-protocol"

import { decodeProtocolJson } from "./protocol-json.js"

export type DesktopRuntime = {
  start(): Promise<void>
  close(): Promise<void>
  bridge: NativeDaemonBridge
  subscribeLifecycle(listener: (projection: DesktopDaemonLifecycleProjection) => void): Promise<DesktopDaemonLifecycleProjection>
  forkRun(request: ForkRunRequest): Promise<ForkRunResult>
  continueRun(request: ContinueRunRequest): Promise<ContinueRunResult>
  switchAccountAndResume(request: SwitchAccountAndResumeRequest): Promise<SwitchAccountAndResumeResult>
  spawnRun(request: SpawnRunRequest): Promise<SpawnRunResult>
  joinRun(request: JoinRunRequest): Promise<JoinRunResult>
  navigationIntent(intent: DaemonNavigationIntent): Promise<NavigationSnapshot>
  diagnosticsSnapshot(): Promise<DaemonDiagnostics>
  listRecipes(): Promise<RecipeListResponse>
  listWorkItems(query?: WorkItemListQuery): Promise<WorkItemListResult>
  refreshWorkItems(params?: WorkItemRefreshParams): Promise<WorkItemListResult>
  dismissWorkItem(params: WorkItemDismissParams): Promise<WorkItemDismissResult>
  triggerWorkItem(sessionId: string, params: WorkItemTriggerParams): Promise<WorkItemTriggerResult>
  createScheduledWork(request: CreateScheduledWorkRequest): Promise<CreateScheduledWorkResult>
  listScheduledWork(): Promise<ListScheduledWorkResult>
  cancelScheduledWork(request: CancelScheduledWorkRequest): Promise<void>
  inspectPluginPackage?(request: InspectPluginPackageRequest): Promise<PluginInspection>
  installPluginPackage?(request: InstallPluginPackageRequest): Promise<InstallPluginPackageResult>
  listPluginInstallations?(): Promise<ListPluginInstallationsResult>
  uninstallPlugin?(request: UninstallPluginRequest): Promise<void>
  listNativeRuns(sessionId: string, request: ListNativeRunsRequest): Promise<ListNativeRunsResult>
  runLineageGraph(sessionId: string, request?: RunLineageGraphRequest): Promise<RunLineageGraphResult>
  spawnTerminal(params: TerminalSpawnParams): Promise<TerminalSpawnResult>
  listTerminals(params: TerminalListParams): Promise<TerminalListResult>
  terminalInput(params: TerminalInputParams): Promise<TerminalInputResult>
  resizeTerminal(params: TerminalResizeParams): Promise<TerminalResizeResult>
  closeTerminal(params: TerminalCloseParams): Promise<TerminalCloseResult>
  subscribeTerminal(
    terminalId: TerminalSessionId,
    listener: {
      attached(initial: TerminalAttachResult): void
      event(event: TerminalEventParams): void
      failed(message: string): void
    },
  ): Promise<TerminalAttachResult>
  releaseTerminalSubscription(): void
  voicePermissionState(): VoicePermissionState
  requestVoicePermission(listener: (permission: VoicePermissionState) => void): void
  subscribeVoiceState(listener: (event: VoiceEvent) => void): void
}

/** Plugin calls are available only from the real native desktop runtime. */
export type PluginDesktopRuntime = Required<Pick<DesktopRuntime,
  "inspectPluginPackage" | "installPluginPackage" | "listPluginInstallations" | "uninstallPlugin"
>>

/** The sole desktop owner of the redacted Rust bridge instance. */
export function createDesktopRuntime(bridge: NativeDaemonBridge = new NativeDaemonBridge()): DesktopRuntime & PluginDesktopRuntime {

  return {
    bridge,
    async start() {
      await bridge.start()
    },
    async close() {
      await bridge.close()
    },
    async subscribeLifecycle(listener) {
      const bufferedProjections: DesktopDaemonLifecycleProjection[] = []
      let initialDelivered = false
      const initialProjectionJson = await bridge.subscribeLifecycle((projectionJson) => {
        const projection = decodeProtocolJson<DesktopDaemonLifecycleProjection>(projectionJson)
        if (!initialDelivered) {
          bufferedProjections.push(projection)
          return
        }
        listener(projection)
      })
      const initialProjection = decodeProtocolJson<DesktopDaemonLifecycleProjection>(initialProjectionJson)
      listener(initialProjection)
      for (const projection of bufferedProjections) listener(projection)
      initialDelivered = true
      return initialProjection
    },
    async forkRun(request) {
      return decodeProtocolJson<ForkRunResult>(await bridge.forkRun(JSON.stringify(request)))
    },
    async continueRun(request) {
      return decodeProtocolJson<ContinueRunResult>(await bridge.continueRun(JSON.stringify(request)))
    },
    async switchAccountAndResume(request) {
      return decodeProtocolJson<SwitchAccountAndResumeResult>(await bridge.switchAccountAndResume(JSON.stringify(request)))
    },
    async spawnRun(request) {
      return decodeProtocolJson<SpawnRunResult>(await bridge.spawnRun(JSON.stringify(request)))
    },
    async joinRun(request) {
      return decodeProtocolJson<JoinRunResult>(await bridge.joinRun(JSON.stringify(request)))
    },
    async navigationIntent(intent) {
      return decodeProtocolJson<NavigationSnapshot>(await bridge.navigationIntent(JSON.stringify(intent)))
    },
    async diagnosticsSnapshot() {
      return decodeProtocolJson<DaemonDiagnostics>(await bridge.diagnosticsSnapshot())
    },
    async listRecipes() {
      return decodeProtocolJson<RecipeListResponse>(await bridge.listRecipes())
    },
    async listWorkItems(query = {}) {
      return decodeProtocolJson<WorkItemListResult>(await bridge.listWorkItems(JSON.stringify(query)))
    },
    async refreshWorkItems(params = {}) {
      return decodeProtocolJson<WorkItemListResult>(await bridge.refreshWorkItems(JSON.stringify(params)))
    },
    async dismissWorkItem(params) {
      return decodeProtocolJson<WorkItemDismissResult>(await bridge.dismissWorkItem(JSON.stringify(params)))
    },
    async triggerWorkItem(sessionId, params) {
      return decodeProtocolJson<WorkItemTriggerResult>(
        await bridge.triggerWorkItem(sessionId, JSON.stringify(params)),
      )
    },
    async createScheduledWork(request) {
      return decodeProtocolJson<CreateScheduledWorkResult>(await bridge.createScheduledWork(JSON.stringify(request)))
    },
    async listScheduledWork() {
      return decodeProtocolJson<ListScheduledWorkResult>(await bridge.listScheduledWork())
    },
    async cancelScheduledWork(request) {
      await bridge.cancelScheduledWork(JSON.stringify(request))
    },
    async inspectPluginPackage(request) {
      return decodeProtocolJson<PluginInspection>(await bridge.inspectPluginPackage(JSON.stringify(request)))
    },
    async installPluginPackage(request) {
      return decodeProtocolJson<InstallPluginPackageResult>(await bridge.installPluginPackage(JSON.stringify(request)))
    },
    async listPluginInstallations() {
      return decodeProtocolJson<ListPluginInstallationsResult>(await bridge.listPluginInstallations())
    },
    async uninstallPlugin(request) {
      await bridge.uninstallPlugin(JSON.stringify(request))
    },
    async listNativeRuns(sessionId, request) {
      return decodeProtocolJson<ListNativeRunsResult>(
        await bridge.listNativeRuns(sessionId, JSON.stringify(request)),
      )
    },
    async runLineageGraph(sessionId, request = {}) {
      return decodeProtocolJson<RunLineageGraphResult>(await bridge.runLineageGraph(sessionId, JSON.stringify(request)))
    },
    async spawnTerminal(params) {
      return decodeProtocolJson<TerminalSpawnResult>(await bridge.spawnTerminal(JSON.stringify(params)))
    },
    async listTerminals(params) {
      return decodeProtocolJson<TerminalListResult>(await bridge.listTerminals(JSON.stringify(params)))
    },
    async terminalInput(params) {
      return decodeProtocolJson<TerminalInputResult>(await bridge.terminalInput(JSON.stringify(params)))
    },
    async resizeTerminal(params) {
      return decodeProtocolJson<TerminalResizeResult>(await bridge.resizeTerminal(JSON.stringify(params)))
    },
    async closeTerminal(params) {
      return decodeProtocolJson<TerminalCloseResult>(await bridge.closeTerminal(JSON.stringify(params)))
    },
    async subscribeTerminal(terminalId, listener) {
      type Delivery =
        | { kind: "event"; event: TerminalEventParams }
        | { kind: "failed"; message: string }
      const bufferedDeliveries: Delivery[] = []
      let initialDelivered = false
      const initialJson = await bridge.subscribeTerminalEvents(terminalId, (eventJson) => {
        let delivery: Delivery
        try {
          delivery = { kind: "event", event: decodeProtocolJson<TerminalEventParams>(eventJson) }
        } catch {
          delivery = { kind: "failed", message: "The terminal event stream ended unexpectedly." }
        }
        if (!initialDelivered) {
          bufferedDeliveries.push(delivery)
          return
        }
        if (delivery.kind === "event") listener.event(delivery.event)
        else listener.failed(delivery.message)
      })
      const initial = decodeProtocolJson<TerminalAttachResult>(initialJson)
      listener.attached(initial)
      initialDelivered = true
      for (const delivery of bufferedDeliveries) {
        if (delivery.kind === "event") listener.event(delivery.event)
        else listener.failed(delivery.message)
      }
      return initial
    },
    releaseTerminalSubscription() {
      bridge.releaseTerminalEventSubscription()
    },
    voicePermissionState() {
      return decodeProtocolJson<VoicePermissionState>(bridge.voicePermissionState())
    },
    requestVoicePermission(listener) {
      bridge.requestVoicePermission((permissionJson) => {
        listener(decodeProtocolJson<VoicePermissionState>(permissionJson))
      })
    },
    subscribeVoiceState(listener) {
      bridge.subscribeVoiceState((eventJson) => {
        listener(decodeProtocolJson<VoiceEvent>(eventJson))
      })
    },
  }
}
