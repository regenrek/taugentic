import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const hoisted = vi.hoisted(() => ({
  clients: [] as Array<{
    attachedSessionId: string | null | undefined;
    options: Record<string, unknown>;
    getAgentRuntime: ReturnType<typeof vi.fn>;
    selectAgentRuntimeProfile: ReturnType<typeof vi.fn>;
    patchAgentRuntimeProfile: ReturnType<typeof vi.fn>;
    loginAgentRuntimeAuthProfile: ReturnType<typeof vi.fn>;
    logoutAgentRuntimeAuthProfile: ReturnType<typeof vi.fn>;
    setAgentRuntimeExtensionEnabled: ReturnType<typeof vi.fn>;
  }>,
}));

vi.mock("../../packages/main/src/daemon-session-request-client.js", () => ({
  DaemonSessionRequestClient: class FakeDaemonSessionRequestClient {
    private readonly record;

    constructor(attachedSessionId?: string | null, options: Record<string, unknown> = {}) {
      this.record = {
        attachedSessionId,
        options,
        getAgentRuntime: vi.fn(async () => ({ owner: "standard" })),
        selectAgentRuntimeProfile: vi.fn(async (params: unknown) => ({
          owner: "standard",
          params,
        })),
        patchAgentRuntimeProfile: vi.fn(async (params: unknown) => ({ owner: "standard", params })),
        loginAgentRuntimeAuthProfile: vi.fn(async (params: unknown) => ({ owner: "auth", params })),
        logoutAgentRuntimeAuthProfile: vi.fn(async (params: unknown) => ({
          owner: "auth",
          params,
        })),
        setAgentRuntimeExtensionEnabled: vi.fn(async (params: unknown) => ({
          owner: "standard",
          params,
        })),
      };
      hoisted.clients.push(this.record);
    }

    getAgentRuntime() {
      return this.record.getAgentRuntime();
    }

    selectAgentRuntimeProfile(params: unknown) {
      return this.record.selectAgentRuntimeProfile(params);
    }

    patchAgentRuntimeProfile(params: unknown) {
      return this.record.patchAgentRuntimeProfile(params);
    }

    loginAgentRuntimeAuthProfile(params: unknown) {
      return this.record.loginAgentRuntimeAuthProfile(params);
    }

    logoutAgentRuntimeAuthProfile(params: unknown) {
      return this.record.logoutAgentRuntimeAuthProfile(params);
    }

    setAgentRuntimeExtensionEnabled(params: unknown) {
      return this.record.setAgentRuntimeExtensionEnabled(params);
    }
  },
}));

describe("daemon-agent-runtime", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.clients.length = 0;
  });

  it("uses a longer timeout policy only for interactive auth mutations", async () => {
    const {
      DAEMON_REQUEST_TIMEOUT_AGENT_RUNTIME,
      DAEMON_REQUEST_TIMEOUT_DISABLED,
      DAEMON_REQUEST_TIMEOUT_INTERACTIVE_AUTH,
    } = await import("../../packages/main/src/daemon-rpc-connection.js");
    const { desktopAgentRuntimeInvokeHandlers } =
      await import("../../packages/main/src/daemon-agent-runtime.js");

    expect(hoisted.clients).toHaveLength(3);
    expect(hoisted.clients[0]).toMatchObject({
      attachedSessionId: null,
      options: { requestTimeout: DAEMON_REQUEST_TIMEOUT_AGENT_RUNTIME },
    });
    expect(hoisted.clients[1]).toMatchObject({
      attachedSessionId: null,
      options: { requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED },
    });
    expect(hoisted.clients[2]).toMatchObject({
      attachedSessionId: null,
      options: { requestTimeout: DAEMON_REQUEST_TIMEOUT_INTERACTIVE_AUTH },
    });

    await expect(desktopAgentRuntimeInvokeHandlers.getAgentRuntime()).resolves.toEqual({
      owner: "standard",
    });
    await expect(
      desktopAgentRuntimeInvokeHandlers.loginAgentRuntimeAuthProfile({
        authProfileId: "auth-codex-chatgpt",
      }),
    ).resolves.toEqual({
      owner: "auth",
      params: { authProfileId: "auth-codex-chatgpt" },
    });
    await expect(
      desktopAgentRuntimeInvokeHandlers.logoutAgentRuntimeAuthProfile({
        authProfileId: "auth-codex-chatgpt",
      }),
    ).resolves.toEqual({
      owner: "auth",
      params: { authProfileId: "auth-codex-chatgpt" },
    });

    expect(hoisted.clients[0].getAgentRuntime).toHaveBeenCalledTimes(1);
    expect(hoisted.clients[0].loginAgentRuntimeAuthProfile).not.toHaveBeenCalled();
    expect(hoisted.clients[1].getAgentRuntime).not.toHaveBeenCalled();
    expect(hoisted.clients[1].loginAgentRuntimeAuthProfile).toHaveBeenCalledTimes(1);
    expect(hoisted.clients[1].logoutAgentRuntimeAuthProfile).not.toHaveBeenCalled();
    expect(hoisted.clients[2].loginAgentRuntimeAuthProfile).not.toHaveBeenCalled();
    expect(hoisted.clients[2].logoutAgentRuntimeAuthProfile).toHaveBeenCalledTimes(1);
  });
});
