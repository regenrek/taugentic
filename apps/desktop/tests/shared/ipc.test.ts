import { describe, expect, it } from "vite-plus/test";

import {
  createDesktopStreamOpenErrorResponse,
  createDesktopStreamOpenRequest,
  createDesktopStreamOpenSuccessResponse,
  createDesktopWindowState,
  rendererOwnsWindowControls,
  resolveWindowChromeOptions,
  DESKTOP_INVOKE_METHODS,
  DESKTOP_IPC_SCHEMA,
  DESKTOP_WINDOW_CHANNELS,
  DESKTOP_STREAM_METHODS,
  createDesktopApi,
  getDesktopStreamResponseChannel,
  parseDesktopWindowState,
  parseDesktopStreamOpenRequest,
  parseDesktopStreamOpenResponse,
  resolveDesktopWindowPlatform,
} from "../../packages/shared/src/ipc.js";

describe("createDesktopApi", () => {
  it("routes invoke methods through the schema channel", async () => {
    const calls: Array<{ channel: string; args: unknown[] }> = [];
    const desktopApi = createDesktopApi({
      async invoke(channel, ...args) {
        calls.push({ channel, args });
        return { channel, args };
      },
    });

    await desktopApi.getDaemonStatus();
    await desktopApi.getAgentRuntime();
    await desktopApi.selectAgentRuntimeProfile({ runtimeProfileId: "runtime-codex-safe" });
    await desktopApi.patchAgentRuntimeProfile({
      runtimeProfileId: "runtime-codex-safe",
      patch: { policyMode: "allow" },
    });
    await desktopApi.loginAgentRuntimeAuthProfile({ authProfileId: "auth-codex-chatgpt" });
    await desktopApi.logoutAgentRuntimeAuthProfile({ authProfileId: "auth-codex-chatgpt" });
    await desktopApi.setAgentRuntimeExtensionEnabled({
      extensionId: "local-shell-tools",
      enabled: true,
    });
    await desktopApi.openSession("Build daemon app server");
    await desktopApi.getSessionOverview({ recentActivityLimit: 5 });
    await desktopApi.listRecipes();
    await desktopApi.getWorkflowStatus();
    await desktopApi.loadWorkflow({ path: "/Users/alice/.taugentic/workflow.yaml" });
    await desktopApi.reloadWorkflow();
    await desktopApi.validateWorkflow({ contents: "kind: taugentic.workflow/v1" });
    await desktopApi.startRun("session-1", { objective: "Ship app server hard cut" });
    await desktopApi.getSession("session-1");
    await desktopApi.getActivityPage("session-1", { limit: 25 });
    await desktopApi.listApprovals("session-1", {});
    await desktopApi.listArtifacts("session-1", {});
    await desktopApi.readArtifactContent({
      sessionId: "session-1",
      artifactId: "artifact-1",
    });
    await desktopApi.saveArtifactAs({
      sessionId: "session-1",
      artifactId: "artifact-1",
      suggestedFilename: "patch.diff",
    });
    await desktopApi.listRuns("session-1");
    await desktopApi.getRunDetail("session-1", "run-1");
    await desktopApi.listNativeRuns("session-1", { limit: 25 });
    await desktopApi.replayRunEvents("session-1", "run-1", 42n);

    expect(calls).toEqual([
      {
        channel: DESKTOP_IPC_SCHEMA.getDaemonStatus.channel,
        args: [],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getAgentRuntime.channel,
        args: [],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.selectAgentRuntimeProfile.channel,
        args: [{ runtimeProfileId: "runtime-codex-safe" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.patchAgentRuntimeProfile.channel,
        args: [{ runtimeProfileId: "runtime-codex-safe", patch: { policyMode: "allow" } }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.loginAgentRuntimeAuthProfile.channel,
        args: [{ authProfileId: "auth-codex-chatgpt" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.logoutAgentRuntimeAuthProfile.channel,
        args: [{ authProfileId: "auth-codex-chatgpt" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.setAgentRuntimeExtensionEnabled.channel,
        args: [{ extensionId: "local-shell-tools", enabled: true }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.openSession.channel,
        args: ["Build daemon app server"],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getSessionOverview.channel,
        args: [{ recentActivityLimit: 5 }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.listRecipes.channel,
        args: [],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getWorkflowStatus.channel,
        args: [],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.loadWorkflow.channel,
        args: [{ path: "/Users/alice/.taugentic/workflow.yaml" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.reloadWorkflow.channel,
        args: [],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.validateWorkflow.channel,
        args: [{ contents: "kind: taugentic.workflow/v1" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.startRun.channel,
        args: ["session-1", { objective: "Ship app server hard cut" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getSession.channel,
        args: ["session-1"],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getActivityPage.channel,
        args: ["session-1", { limit: 25 }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.listApprovals.channel,
        args: ["session-1", {}],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.listArtifacts.channel,
        args: ["session-1", {}],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.readArtifactContent.channel,
        args: [{ sessionId: "session-1", artifactId: "artifact-1" }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.saveArtifactAs.channel,
        args: [
          {
            sessionId: "session-1",
            artifactId: "artifact-1",
            suggestedFilename: "patch.diff",
          },
        ],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.listRuns.channel,
        args: ["session-1"],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.getRunDetail.channel,
        args: ["session-1", "run-1"],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.listNativeRuns.channel,
        args: ["session-1", { limit: 25 }],
      },
      {
        channel: DESKTOP_IPC_SCHEMA.replayRunEvents.channel,
        args: ["session-1", "run-1", 42n],
      },
    ]);
  });
});

describe("desktop IPC method classification", () => {
  it("keeps invoke and stream methods partitioned by schema kind", () => {
    const schemaEntries = Object.entries(DESKTOP_IPC_SCHEMA);
    const invokeMethods = schemaEntries
      .filter(([, spec]) => spec.kind === "invoke")
      .map(([method]) => method);
    const streamMethods = schemaEntries
      .filter(([, spec]) => spec.kind === "stream")
      .map(([method]) => method);

    expect(DESKTOP_INVOKE_METHODS).toEqual(invokeMethods);
    expect(DESKTOP_STREAM_METHODS).toEqual(streamMethods);
  });
});

describe("desktop stream request correlation helpers", () => {
  it("creates and parses request-correlated stream open payloads", () => {
    const request = createDesktopStreamOpenRequest(DESKTOP_IPC_SCHEMA.openRunStream, "req-1", [
      "session-1",
      null,
    ]);

    expect(request).toEqual({
      requestId: "req-1",
      args: ["session-1", null],
    });
    expect(parseDesktopStreamOpenRequest("openRunStream", request)).toEqual(request);
    expect(getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "req-1")).toBe(
      `${DESKTOP_IPC_SCHEMA.openRunStream.responseChannelPrefix}req-1`,
    );
    expect(
      parseDesktopStreamOpenResponse("openRunStream", createDesktopStreamOpenSuccessResponse()),
    ).toEqual({ status: "ok" });
    expect(
      parseDesktopStreamOpenResponse(
        "openRunStream",
        createDesktopStreamOpenErrorResponse("attach failed"),
      ),
    ).toEqual({ status: "error", message: "attach failed" });
  });

  it("rejects malformed request-correlated stream open payloads", () => {
    expect(() =>
      parseDesktopStreamOpenRequest("openRunStream", {
        requestId: " ",
        args: ["session-1", null],
      }),
    ).toThrow("desktop stream request id must be a non-empty string");

    expect(() =>
      parseDesktopStreamOpenRequest("openRunStream", {
        requestId: "req-1",
        args: ["session-1"],
      }),
    ).toThrow("desktop IPC method openRunStream expected 2 arg(s), got 1");

    expect(() => getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, " ")).toThrow(
      "desktop stream request id must be a non-empty string",
    );
    expect(() => parseDesktopStreamOpenResponse("openRunStream", { status: "nope" })).toThrow(
      'desktop stream response openRunStream status must be "ok" or "error"',
    );
  });
});

describe("desktop window chrome contracts", () => {
  it("defines dedicated window chrome channels outside the daemon invoke schema", () => {
    expect(DESKTOP_WINDOW_CHANNELS).toEqual({
      close: "desktop:window-close",
      getState: "desktop:get-window-state",
      minimize: "desktop:window-minimize",
      stateDidChange: "desktop:window-state-did-change",
      toggleMaximize: "desktop:window-toggle-maximize",
    });
    expect(DESKTOP_INVOKE_METHODS).not.toContain("getWindowState");
    expect(DESKTOP_INVOKE_METHODS).not.toContain("toggleMaximizeWindow");
  });

  it("normalizes window state defaults and platform mapping", () => {
    expect(resolveDesktopWindowPlatform("darwin")).toBe("macos");
    expect(resolveDesktopWindowPlatform("win32")).toBe("windows");
    expect(resolveDesktopWindowPlatform("linux")).toBe("linux");
    expect(createDesktopWindowState("macos")).toEqual({
      canClose: true,
      canMaximize: true,
      canMinimize: true,
      controlsAlignment: "leading",
      isFocused: true,
      isFullScreen: false,
      isMaximized: false,
      platform: "macos",
    });
  });

  it("parses window state payloads from native chrome events", () => {
    expect(
      parseDesktopWindowState({
        canClose: true,
        canMaximize: false,
        canMinimize: true,
        controlsAlignment: "trailing",
        isFocused: false,
        isFullScreen: true,
        isMaximized: false,
        platform: "windows",
      }),
    ).toEqual({
      canClose: true,
      canMaximize: false,
      canMinimize: true,
      controlsAlignment: "trailing",
      isFocused: false,
      isFullScreen: true,
      isMaximized: false,
      platform: "windows",
    });

    expect(() =>
      parseDesktopWindowState({
        canClose: true,
        canMaximize: true,
        canMinimize: "yes",
        isFocused: true,
        isFullScreen: false,
        isMaximized: false,
        platform: "linux",
      }),
    ).toThrow("desktop window state canMinimize must be a boolean");
  });

  it("resolves cross-OS window chrome options per canonical 2026 pattern", () => {
    const colors = { background: "#14171b", symbol: "#ededed" };

    expect(resolveWindowChromeOptions("macos", colors)).toEqual({
      frame: true,
      titleBarStyle: "hiddenInset",
      trafficLightPosition: { x: 12, y: 12 },
    });

    expect(resolveWindowChromeOptions("windows", colors)).toEqual({
      frame: true,
      titleBarStyle: "hidden",
      titleBarOverlay: { color: "#14171b", symbolColor: "#ededed", height: 36 },
    });

    expect(resolveWindowChromeOptions("linux", colors)).toEqual({
      frame: false,
      titleBarStyle: "default",
    });
  });

  it("only lets the renderer draw window controls on Linux", () => {
    expect(rendererOwnsWindowControls("macos")).toBe(false);
    expect(rendererOwnsWindowControls("windows")).toBe(false);
    expect(rendererOwnsWindowControls("linux")).toBe(true);
  });
});
