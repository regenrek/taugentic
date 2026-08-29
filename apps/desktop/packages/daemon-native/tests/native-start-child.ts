import { NativeDaemonBridge } from "../index.js";

const EXIT_START = 1;
const EXIT_SUBSCRIBE = 2;
const EXIT_NAVIGATION = 3;
const EXIT_CALLBACK_PROJECTION = 4;
const EXIT_CLOSE = 5;
const EXIT_PROJECT_OPEN = 6;
const EXIT_SESSION_OPEN = 7;
const EXIT_TRANSCRIPT = 8;
const EXIT_TERMINAL = 9;
const EXIT_GIT_SNAPSHOT = 10;
const EXIT_GIT_CHECKPOINTS = 11;
const EXIT_GIT_REQUEST = 12;
const EXIT_CODE_HOST_PROJECTION = 13;
const EXIT_SESSION_ATTACH = 14;
const EXIT_THREAD_WORKSPACE = 15;
const EXIT_LINEAGE_GRAPH = 16;

type JsonObject = Record<string, unknown>;

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseJsonObject(value: unknown): JsonObject | null {
  if (typeof value !== "string") {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(value);
    return isJsonObject(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function isLifecycleProjection(value: unknown): boolean {
  const projection = parseJsonObject(value);
  return (
    projection !== null &&
    (projection.status === "ready" ||
      projection.status === "snapshotRehydrationRequired" ||
      projection.status === "disconnected") &&
    typeof projection.invalidated === "boolean" &&
    typeof projection.foreignRuntimeRestricted === "boolean"
  );
}

function isNavigationSnapshot(value: unknown): boolean {
  const snapshot = parseJsonObject(value);
  if (snapshot === null) {
    return false;
  }
  return ["spaces", "projects", "conversations", "agents"].every(
    (key) => snapshot[key] === undefined || Array.isArray(snapshot[key]),
  );
}

function projectOpenResult(value: unknown): JsonObject | null {
  const result = parseJsonObject(value);
  return result !== null && typeof result.projectId === "string" && isJsonObject(result.snapshot)
    ? result
    : null;
}

function firstProject(result: JsonObject): { projectId: string; workspaceId: string } | null {
  if (!isJsonObject(result.snapshot) || !Array.isArray(result.snapshot.projects)) {
    return null;
  }
  const project = result.snapshot.projects.find((candidate) => (
    isJsonObject(candidate)
    && candidate.id === result.projectId
    && Array.isArray(candidate.workspaceIds)
    && typeof candidate.workspaceIds[0] === "string"
  ));
  return isJsonObject(project)
    ? { projectId: project.id as string, workspaceId: (project.workspaceIds as string[])[0] }
    : null;
}

function isSessionSummary(value: unknown): boolean {
  const session = parseJsonObject(value);
  return session !== null
    && typeof session.id === "string"
    && session.title === "Native project conversation"
    && session.status === "idle";
}

function isEmptyThreadWorkspace(value: unknown, sessionId: string): value is JsonObject {
  const workspace = parseJsonObject(value);
  return workspace !== null
    && workspace.sessionId === sessionId
    && workspace.goal === ""
    && workspace.plan === ""
    && workspace.recap === ""
    && workspace.notes === ""
    && Array.isArray(workspace.pins)
    && workspace.pins.length === 0
    && Array.isArray(workspace.workLog)
    && workspace.workLog.length === 0;
}

function isGoalThreadWorkspace(value: unknown, sessionId: string): boolean {
  const workspace = parseJsonObject(value);
  if (
    workspace === null
    || workspace.sessionId !== sessionId
    || workspace.goal !== "Ship the workspace"
    || workspace.plan !== ""
    || workspace.recap !== ""
    || workspace.notes !== ""
    || !Array.isArray(workspace.pins)
    || workspace.pins.length !== 0
    || !Array.isArray(workspace.workLog)
    || workspace.workLog.length !== 1
  ) {
    return false;
  }
  const entry = workspace.workLog[0];
  return isJsonObject(entry)
    && entry.sequence === "1"
    && typeof entry.occurredAtMs === "string"
    && entry.kind === "goalSet";
}

function isLineageGraph(value: unknown): boolean {
  const graph = parseJsonObject(value);
  return graph !== null
    && Array.isArray(graph.nodes)
    && Array.isArray(graph.edges)
    && Array.isArray(graph.orphanRunIds)
    && typeof graph.totalCount === "number"
    && typeof graph.omittedCount === "number"
    && typeof graph.truncated === "boolean"
    && typeof graph.cycleBroken === "boolean";
}

let currentStage = EXIT_START;
let bridge: NativeDaemonBridge;
try {
  bridge = new NativeDaemonBridge();
} catch {
  process.exit(currentStage);
}

let exitCode = 0;
let started = false;
let callbackProjectionInvalid = false;
let selectedProject: { projectId: string; workspaceId: string } | null = null;
let openedSessionId: string | null = null;
let resolveLifecycleCallback!: () => void;
const lifecycleCallback = new Promise<void>((resolve) => {
  resolveLifecycleCallback = resolve;
});

try {
  const result = parseJsonObject(await bridge.start());
  started = result?.started === true;
  if (!started) {
    exitCode = EXIT_START;
  }
} catch {
  exitCode = currentStage;
}

if (exitCode === 0) {
  currentStage = EXIT_SUBSCRIBE;
  try {
    const subscription = await bridge.subscribeLifecycle((...args: unknown[]) => {
      if (args.length !== 1 || typeof args[0] !== "string" || !isLifecycleProjection(args[0])) {
        callbackProjectionInvalid = true;
      }
      resolveLifecycleCallback();
    });
    if (!isLifecycleProjection(subscription)) {
      exitCode = EXIT_SUBSCRIBE;
    } else if (callbackProjectionInvalid) {
      exitCode = EXIT_CALLBACK_PROJECTION;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0) {
  currentStage = EXIT_CODE_HOST_PROJECTION;
  try {
    const accounts = parseJsonObject(await bridge.codeHostAccounts());
    if (accounts === null || !Array.isArray(accounts.accounts)) {
      exitCode = EXIT_CODE_HOST_PROJECTION;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0) {
  currentStage = EXIT_PROJECT_OPEN;
  try {
    const workspacePath = process.env.TAUGENTIC_WORKSPACE_PATH;
    if (!workspacePath) {
      exitCode = EXIT_PROJECT_OPEN;
    } else {
      const first = projectOpenResult(await bridge.openProject(workspacePath, true));
      const reopened = projectOpenResult(await bridge.openProject(workspacePath, false));
      if (first === null || reopened === null || first.projectId !== reopened.projectId) {
        exitCode = EXIT_PROJECT_OPEN;
      } else {
        selectedProject = firstProject(reopened);
        if (selectedProject === null) {
          exitCode = EXIT_PROJECT_OPEN;
        }
        await lifecycleCallback;
        if (callbackProjectionInvalid) {
          exitCode = EXIT_CALLBACK_PROJECTION;
        }
      }
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && selectedProject !== null) {
  currentStage = EXIT_GIT_REQUEST;
  try {
    const params = JSON.stringify(selectedProject);
    const result = parseJsonObject(await bridge.gitSnapshot(params));
    const snapshot = result !== null && isJsonObject(result.snapshot) ? result.snapshot : null;
    if (
      snapshot === null
      || !Array.isArray(snapshot.files)
      || !Array.isArray(snapshot.worktrees)
      || typeof snapshot.fingerprint !== "string"
    ) {
      exitCode = EXIT_GIT_SNAPSHOT;
    } else {
      const checkpoints = parseJsonObject(await bridge.gitCheckpointList(params));
      if (checkpoints === null || !Array.isArray(checkpoints.checkpoints)) {
        exitCode = EXIT_GIT_CHECKPOINTS;
      }
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && selectedProject !== null) {
  currentStage = EXIT_SESSION_OPEN;
  try {
    const opened = await bridge.openSession(JSON.stringify({
      title: "Native project conversation",
      workspace: {
        kind: "byProject",
        projectId: selectedProject.projectId,
        workspaceId: selectedProject.workspaceId,
      },
    }));
    if (!isSessionSummary(opened)) {
      exitCode = EXIT_SESSION_OPEN;
    } else {
      openedSessionId = parseJsonObject(opened)?.id as string;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && openedSessionId !== null) {
  currentStage = EXIT_SESSION_ATTACH;
  try {
    const attached = await bridge.attachSession(openedSessionId);
    if (!isSessionSummary(attached)) {
      exitCode = EXIT_SESSION_ATTACH;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && openedSessionId !== null) {
  currentStage = EXIT_THREAD_WORKSPACE;
  try {
    const empty = await bridge.threadWorkspace();
    if (!isEmptyThreadWorkspace(empty, openedSessionId)) {
      exitCode = EXIT_THREAD_WORKSPACE;
    } else {
      const updated = await bridge.updateThreadWorkspace(JSON.stringify({
        mutation: { kind: "goalSet", value: "Ship the workspace" },
      }));
      if (!isGoalThreadWorkspace(updated, openedSessionId)) {
        exitCode = EXIT_THREAD_WORKSPACE;
      } else {
        const reread = await bridge.threadWorkspace();
        if (reread !== updated || !isGoalThreadWorkspace(reread, openedSessionId)) {
          exitCode = EXIT_THREAD_WORKSPACE;
        }
      }
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && openedSessionId !== null) {
  currentStage = EXIT_TRANSCRIPT;
  try {
    const page = parseJsonObject(await bridge.agentTurnsPage(
      openedSessionId,
      JSON.stringify({ limit: 100 }),
    ));
    if (page === null || (page.items !== undefined && !Array.isArray(page.items))) {
      exitCode = EXIT_TRANSCRIPT;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && openedSessionId !== null) {
  currentStage = EXIT_LINEAGE_GRAPH;
  try {
    if (!isLineageGraph(await bridge.runLineageGraph(openedSessionId, "{}"))) {
      exitCode = EXIT_LINEAGE_GRAPH;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0 && selectedProject !== null) {
  currentStage = EXIT_TERMINAL;
  try {
    const spawned = parseJsonObject(await bridge.spawnTerminal(JSON.stringify({
      projectId: selectedProject.projectId,
      workspaceId: selectedProject.workspaceId,
      rows: 24,
      cols: 80,
      userApproved: true,
    })));
    const terminal = spawned !== null && isJsonObject(spawned.terminal)
      ? spawned.terminal
      : null;
    if (terminal === null || typeof terminal.id !== "string") {
      exitCode = EXIT_TERMINAL;
    } else {
      let output = "";
      let resolveMarker!: () => void;
      const markerSeen = new Promise<void>((resolve) => {
        resolveMarker = resolve;
      });
      const attached = parseJsonObject(await bridge.subscribeTerminalEvents(
        terminal.id,
        (eventJson: string) => {
          const params = parseJsonObject(eventJson);
          const event = params !== null && isJsonObject(params.event) ? params.event : null;
          if (event?.kind !== "output" || typeof event.dataBase64 !== "string") {
            return;
          }
          output += Buffer.from(event.dataBase64, "base64").toString("utf8");
          if (output.includes("__TA_NATIVE_TERMINAL__")) {
            resolveMarker();
          }
        },
      ));
      if (attached === null || !isJsonObject(attached.terminal)) {
        exitCode = EXIT_TERMINAL;
      } else {
        const input = parseJsonObject(await bridge.terminalInput(JSON.stringify({
          terminalId: terminal.id,
          dataBase64: Buffer.from("printf '__TA_NATIVE_TERMINAL__\\n'\n").toString("base64"),
        })));
        if (input === null || typeof input.acceptedBytes !== "number") {
          exitCode = EXIT_TERMINAL;
        } else {
          await Promise.race([
            markerSeen,
            new Promise<never>((_, reject) => {
              setTimeout(() => reject(new Error("terminal output timeout")), 5_000);
            }),
          ]);
        }
      }
      bridge.releaseTerminalEventSubscription();
      await bridge.closeTerminal(JSON.stringify({ terminalId: terminal.id }));
    }
  } catch {
    exitCode = currentStage;
  }
}

if (exitCode === 0) {
  currentStage = EXIT_NAVIGATION;
  try {
    const navigation = parseJsonObject(await bridge.navigationSnapshot());
    if (navigation === null || !isNavigationSnapshot(JSON.stringify(navigation))) {
      exitCode = EXIT_NAVIGATION;
    } else if (selectedProject !== null && !Array.isArray(navigation.conversations)) {
      exitCode = EXIT_NAVIGATION;
    } else if (selectedProject !== null && !(navigation.conversations as unknown[]).some((item) => (
      isJsonObject(item)
      && item.title === "Native project conversation"
      && isJsonObject(item.placement)
      && item.placement.kind === "project"
      && item.placement.projectId === selectedProject?.projectId
    ))) {
      exitCode = EXIT_NAVIGATION;
    } else if (callbackProjectionInvalid) {
      exitCode = EXIT_CALLBACK_PROJECTION;
    }
  } catch {
    exitCode = currentStage;
  }
}

if (started) {
  currentStage = EXIT_CLOSE;
  try {
    const closed = parseJsonObject(await bridge.close());
    if (exitCode === 0 && (closed === null || Object.keys(closed).length !== 0)) {
      exitCode = EXIT_CLOSE;
    }
  } catch {
    if (exitCode === 0) {
      exitCode = currentStage;
    }
  }
}

if (exitCode === 0 && callbackProjectionInvalid) {
  exitCode = EXIT_CALLBACK_PROJECTION;
}

process.exitCode = exitCode;
