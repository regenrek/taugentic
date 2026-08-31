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
  const executablePath = process.env.PATH;
  if (!executablePath) {
    throw new Error("native bridge test requires PATH");
  }
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
    PATH: executablePath,
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
  expect(declaration).toContain("readDesktopSettings(): Promise<string | null>");
  expect(declaration).toContain("writeDesktopSettings(documentJson: string): Promise<void>");
  expect(declaration).toContain("attachSession(sessionId: string)");
  expect(declaration).toContain("navigationSnapshot(search?: string | null)");
  expect(declaration).toContain("navigationIntent(intentJson: string)");
  expect(declaration).toContain("openProject(path: string, trustAcknowledged: boolean)");
  expect(declaration).toContain("materializeWorkspaceImage(paramsJson: string)");
  expect(declaration).toContain("materializeArtifactImage(sessionId: string, queryJson: string)");
  expect(declaration).not.toContain("savedPath");
  expect(declaration).not.toContain("dataUri");
  expect(declaration).toContain("completeAuthProfileLogin(paramsJson: string)");
  expect(declaration).toContain("listApprovals(queryJson: string)");
  expect(declaration).toContain("diagnosticsSnapshot(): Promise<string>");
  expect(declaration).toContain("threadWorkspace(): Promise<string>");
  expect(declaration).toContain("updateThreadWorkspace(commandJson: string): Promise<string>");
  expect(declaration).not.toContain("threadWorkspace(sessionId:");
  expect(declaration).not.toContain("updateThreadWorkspace(sessionId:");
  expect(declaration).toContain("agentTurnsPage(sessionId: string, queryJson: string)");
  expect(declaration).toContain("forkRun(requestJson: string)");
  expect(declaration).toContain("switchRouteAndResume(requestJson: string)");
  expect(declaration).toContain("listNativeRuns(sessionId: string, requestJson: string)");
  expect(declaration).toContain("runLineageGraph(sessionId: string, requestJson: string)");
  expect(declaration).toContain("getRun(sessionId: string, queryJson: string)");
  expect(declaration).toContain("runTimeline(sessionId: string, queryJson: string)");
  expect(declaration).toContain("activityPage(sessionId: string, queryJson: string)");
  expect(declaration).toContain("replayRunEvents(sessionId: string, queryJson: string)");
  expect(declaration).toContain("codeHostAccounts(): Promise<string>");
  expect(declaration).toContain("connectCodeHostAccount(paramsJson: string)");
  expect(declaration).toContain("prepareCodeHostPush(paramsJson: string)");
  expect(declaration).toContain("codeHostPullRequests(paramsJson: string)");
  expect(declaration).toContain("createCodeHostPullRequestComment(paramsJson: string)");
  expect(declaration).toContain("decideApproval(paramsJson: string)");
  expect(declaration).toContain("listRecipes(): Promise<string>");
  expect(declaration).toContain("listWorkItems(queryJson: string)");
  expect(declaration).toContain("refreshWorkItems(paramsJson: string)");
  expect(declaration).toContain("dismissWorkItem(paramsJson: string)");
  expect(declaration).toContain("triggerWorkItem(sessionId: string, paramsJson: string)");
  expect(declaration).toContain("createScheduledWork(paramsJson: string)");
  expect(declaration).toContain("listScheduledWork(): Promise<string>");
  expect(declaration).toContain("cancelScheduledWork(paramsJson: string)");
  expect(declaration).toContain("inspectPluginPackage(paramsJson: string)");
  expect(declaration).toContain("installPluginPackage(paramsJson: string)");
  expect(declaration).toContain("listPluginInstallations(): Promise<string>");
  expect(declaration).toContain("uninstallPlugin(paramsJson: string)");
  expect(declaration).toContain("spawnTerminal(paramsJson: string)");
  expect(declaration).toContain("terminalInput(paramsJson: string)");
  expect(declaration).toContain("gitSnapshot(paramsJson: string)");
  expect(declaration).toContain("gitDiff(paramsJson: string)");
  expect(declaration).toContain("gitStage(paramsJson: string)");
  expect(declaration).toContain("gitCheckpointPrepareRevert(paramsJson: string)");
  expect(declaration).toContain("gitCheckpointApplyRevert(paramsJson: string)");
  expect(declaration).toContain("subscribeTerminalEvents(terminalId: string");
  expect(declaration).toContain("subscribeLifecycle(callback: (projectionJson: string) => void)");
  expect(declaration).toContain("voicePermissionState(): string");
  expect(declaration).toContain("requestVoicePermission(callback: (permissionJson: string) => void): string");
  expect(declaration).toContain("subscribeVoiceState(callback: (eventJson: string) => void): string");
  expect(declaration).not.toContain("audioBase64");
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
  expect(typeof bridge.readDesktopSettings).toBe("function");
  expect(typeof bridge.writeDesktopSettings).toBe("function");
  expect(typeof bridge.navigationSnapshot).toBe("function");
  expect(typeof bridge.navigationIntent).toBe("function");
  expect(typeof bridge.openProject).toBe("function");
  expect(typeof bridge.materializeWorkspaceImage).toBe("function");
  expect(typeof bridge.completeAuthProfileLogin).toBe("function");
  expect(typeof bridge.listApprovals).toBe("function");
  expect(typeof bridge.diagnosticsSnapshot).toBe("function");
  expect(typeof bridge.threadWorkspace).toBe("function");
  expect(typeof bridge.updateThreadWorkspace).toBe("function");
  expect(typeof bridge.agentTurnsPage).toBe("function");
  expect(typeof bridge.forkRun).toBe("function");
  expect(typeof bridge.switchRouteAndResume).toBe("function");
  expect(typeof bridge.listNativeRuns).toBe("function");
  expect(typeof bridge.runLineageGraph).toBe("function");
  expect(typeof bridge.getRun).toBe("function");
  expect(typeof bridge.runTimeline).toBe("function");
  expect(typeof bridge.activityPage).toBe("function");
  expect(typeof bridge.materializeArtifactImage).toBe("function");
  expect(typeof bridge.replayRunEvents).toBe("function");
  expect(typeof bridge.codeHostAccounts).toBe("function");
  expect(typeof bridge.connectCodeHostAccount).toBe("function");
  expect(typeof bridge.prepareCodeHostPush).toBe("function");
  expect(typeof bridge.codeHostPullRequests).toBe("function");
  expect(typeof bridge.createCodeHostPullRequestComment).toBe("function");
  expect(typeof bridge.decideApproval).toBe("function");
  expect(typeof bridge.listRecipes).toBe("function");
  expect(typeof bridge.listWorkItems).toBe("function");
  expect(typeof bridge.refreshWorkItems).toBe("function");
  expect(typeof bridge.dismissWorkItem).toBe("function");
  expect(typeof bridge.triggerWorkItem).toBe("function");
  expect(typeof bridge.inspectPluginPackage).toBe("function");
  expect(typeof bridge.installPluginPackage).toBe("function");
  expect(typeof bridge.listPluginInstallations).toBe("function");
  expect(typeof bridge.uninstallPlugin).toBe("function");
  expect(typeof bridge.spawnTerminal).toBe("function");
  expect(typeof bridge.listTerminals).toBe("function");
  expect(typeof bridge.terminalInput).toBe("function");
  expect(typeof bridge.resizeTerminal).toBe("function");
  expect(typeof bridge.closeTerminal).toBe("function");
  expect(typeof bridge.subscribeTerminalEvents).toBe("function");
  expect(typeof bridge.releaseTerminalEventSubscription).toBe("function");
  expect(typeof bridge.gitSnapshot).toBe("function");
  expect(typeof bridge.gitDiff).toBe("function");
  expect(typeof bridge.gitStage).toBe("function");
  expect(typeof bridge.gitUnstage).toBe("function");
  expect(typeof bridge.gitCommit).toBe("function");
  expect(typeof bridge.gitCheckpointList).toBe("function");
  expect(typeof bridge.gitCheckpointPrepareRevert).toBe("function");
  expect(typeof bridge.gitCheckpointApplyRevert).toBe("function");
  await expect(bridge.close()).resolves.toBe("{}");
});

test("native bridge keeps malformed operation input redacted", async () => {
  const unstarted = new NativeDaemonBridge();
  const malformed = "not-json-secret-input";
  const expectRedacted = async (operation: Promise<unknown>) => {
    try {
      await operation;
      throw new Error("malformed native bridge input should fail");
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).toBe("native daemon operation failed");
      expect((error as Error).message).not.toContain(malformed);
    }
  };

  await expectRedacted(unstarted.updateThreadWorkspace(malformed));
  await expectRedacted(unstarted.getRun("session-redaction", malformed));
  await expectRedacted(unstarted.runLineageGraph("session-redaction", malformed));
  await expectRedacted(unstarted.runTimeline("session-redaction", malformed));
  await expectRedacted(unstarted.activityPage("session-redaction", malformed));
  await expectRedacted(unstarted.replayRunEvents("session-redaction", malformed));
});

test("Plugin N-API invokes every staged operation with public-safe failures", async () => {
  const bridge = new NativeDaemonBridge();
  const secret = "plugin-params-must-never-escape";
  const expectPublicFailure = async (operation: Promise<unknown>) => {
    try {
      await operation;
      throw new Error("unstarted Plugin operation should fail");
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).not.toContain(secret);
      expect(["native daemon operation failed", "native daemon bridge is not started"])
        .toContain((error as Error).message);
    }
  };

  await expectPublicFailure(bridge.inspectPluginPackage(secret));
  await expectPublicFailure(bridge.installPluginPackage(secret));
  await expectPublicFailure(bridge.listPluginInstallations());
  await expectPublicFailure(bridge.uninstallPlugin(secret));
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
