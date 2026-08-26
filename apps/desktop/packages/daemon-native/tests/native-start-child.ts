import { NativeDaemonBridge } from "../index.js";

const EXIT_START = 1;
const EXIT_SUBSCRIBE = 2;
const EXIT_NAVIGATION = 3;
const EXIT_CALLBACK_PROJECTION = 4;
const EXIT_CLOSE = 5;
const EXIT_PROJECT_OPEN = 6;
const EXIT_SESSION_OPEN = 7;

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
