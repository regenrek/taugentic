import { NativeDaemonBridge } from "../index.js";

const EXIT_CONSTRUCT = 1;
const EXIT_START = 2;
const EXIT_SUBSCRIBE = 3;
const EXIT_CLOSE = 4;

function isLifecycleProjection(value: unknown): boolean {
  if (typeof value !== "string") {
    return false;
  }

  try {
    const projection: unknown = JSON.parse(value);
    return typeof projection === "object"
      && projection !== null
      && !Array.isArray(projection)
      && (projection.status === "ready"
        || projection.status === "snapshotRehydrationRequired"
        || projection.status === "disconnected")
      && typeof projection.invalidated === "boolean"
      && typeof projection.foreignRuntimeRestricted === "boolean";
  } catch {
    return false;
  }
}

let bridge: NativeDaemonBridge;
try {
  bridge = new NativeDaemonBridge();
} catch {
  process.exitCode = EXIT_CONSTRUCT;
}

let exitCode = process.exitCode ?? 0;
let started = false;

if (exitCode === 0) {
  try {
    const result: unknown = JSON.parse(await bridge!.start());
    started = typeof result === "object"
      && result !== null
      && !Array.isArray(result)
      && (result as Record<string, unknown>).started === true;
    if (!started) {
      exitCode = EXIT_START;
    }
  } catch {
    exitCode = EXIT_START;
  }
}

if (exitCode === 0) {
  try {
    const subscription = await bridge!.subscribeLifecycle(() => {});
    if (!isLifecycleProjection(subscription)) {
      exitCode = EXIT_SUBSCRIBE;
    }
  } catch {
    exitCode = EXIT_SUBSCRIBE;
  }
}

if (started) {
  try {
    const closed: unknown = JSON.parse(await bridge!.close());
    if (exitCode === 0
      && (typeof closed !== "object" || closed === null || Array.isArray(closed)
        || Object.keys(closed).length !== 0)) {
      exitCode = EXIT_CLOSE;
    }
  } catch {
    if (exitCode === 0) {
      exitCode = EXIT_CLOSE;
    }
  }
}

process.exitCode = exitCode;
