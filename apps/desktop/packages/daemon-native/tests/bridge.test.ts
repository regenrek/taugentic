import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { join, resolve } from "node:path";
import { NativeDaemonBridge } from "../index.js";

function releaseDaemonBinary(): string {
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version=1", "--no-deps"], {
      encoding: "utf8",
    }),
  ) as { target_directory: string };
  return resolve(
    metadata.target_directory,
    "release",
    process.platform === "win32" ? "ta-daemon.exe" : "ta-daemon",
  );
}

async function withIsolatedNativeRuntime(
  run: (environment: Record<string, string>) => Promise<void>,
): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "tg-native-"));
  const home = join(root, "home");
  const config = join(root, "config");
  const runtime = join(root, "runtime");
  const logs = join(root, "logs");
  const socketName = `tg-n-${process.pid}`;
  const environment = {
    HOME: home,
    USERPROFILE: home,
    XDG_CONFIG_HOME: config,
    XDG_RUNTIME_DIR: runtime,
    APPDATA: join(home, "AppData", "Roaming"),
    TAUGENTIC_DAEMON_BINARY: releaseDaemonBinary(),
    TAUGENTIC_DAEMON_SOCKET_NAME: socketName,
    TAUGENTIC_DAEMON_RUNTIME_MODE: "local",
    TAUGENTIC_LOG_DIR: logs,
    TAUGENTIC_LOG_STDERR: "0",
    TAUGENTIC_WORKSPACE_PATH: fileURLToPath(new URL("../../../", import.meta.url)),
  } as const;
  await Promise.all([mkdir(home), mkdir(config), mkdir(runtime), mkdir(logs)]);
  try {
    await run(environment);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("native bridge declaration never accepts or returns secret arguments", async () => {
  const declaration = await Bun.file(new URL("../index.d.ts", import.meta.url)).text();

  expect(declaration).not.toContain("credential");
  expect(declaration).not.toContain("authority");
  expect(declaration).not.toContain("socketPath");
  expect(declaration).not.toContain("daemonInstanceId");
  expect(declaration).toContain("openSession(paramsJson: string)");
  expect(declaration).toContain("attachSession(sessionId: string)");
  expect(declaration).toContain("navigationSnapshot(search?: string | null)");
  expect(declaration).toContain("navigationIntent(intentJson: string)");
  expect(declaration).toContain("openProject(path: string, trustAcknowledged: boolean)");
  expect(declaration).toContain("completeAuthProfileLogin(paramsJson: string)");
  expect(declaration).toContain("listApprovals(queryJson: string)");
  expect(declaration).toContain("decideApproval(paramsJson: string)");
  expect(declaration).toContain("subscribeLifecycle(callback: (projectionJson: string) => void)");
});

test("generated navigation contract keeps ungrouped membership and session-derived rows", async () => {
  const project = await Bun.file(
    new URL("../../shared/generated/NavigationProject.ts", import.meta.url),
  ).text();
  const conversation = await Bun.file(
    new URL("../../shared/generated/NavigationConversation.ts", import.meta.url),
  ).text();

  expect(project).toContain("spaceId?: SpaceId | null");
  expect(conversation).toContain("title: string");
  expect(conversation).toContain("status: SessionStatus");
});

test("generated session-open contract includes daemon-owned project placement", async () => {
  const selector = await Bun.file(
    new URL("../../shared/generated/WorkspaceSelector.ts", import.meta.url),
  ).text();

  expect(selector).toContain('"kind": "byProject"');
  expect(selector).toContain("projectId: ProjectId");
  expect(selector).toContain("workspaceId: WorkspaceId");
});

test("loads and invokes the real native module", async () => {
  const bridge = new NativeDaemonBridge();
  expect(typeof bridge.subscribeLifecycle).toBe("function");
  expect(typeof bridge.navigationSnapshot).toBe("function");
  expect(typeof bridge.navigationIntent).toBe("function");
  expect(typeof bridge.openProject).toBe("function");
  expect(typeof bridge.completeAuthProfileLogin).toBe("function");
  expect(typeof bridge.listApprovals).toBe("function");
  expect(typeof bridge.decideApproval).toBe("function");
  await expect(bridge.close()).resolves.toBe("{}");
});

test("starts and releases the current release daemon through the native bridge", async () => {
  await withIsolatedNativeRuntime(async (environment) => {
    const child = Bun.spawn(
      [process.execPath, fileURLToPath(new URL("./native-start-child.ts", import.meta.url))],
      {
        env: environment,
        stdin: "ignore",
        stdout: "ignore",
        stderr: "ignore",
      },
    );
    const termination = setTimeout(() => child.kill(), 20_000);
    try {
      expect(await child.exited).toBe(0);
    } finally {
      clearTimeout(termination);
      child.kill();
      await child.exited;
    }
  });
}, 25_000);
