import { mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

import {
  METHOD_DAEMON_INITIALIZE,
  METHOD_DAEMON_SESSION_ATTACH,
  METHOD_DAEMON_SESSION_OPEN,
} from "../../packages/shared/generated/index.js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const hoisted = vi.hoisted(() => ({
  createDesktopDaemonLocatorConfig: vi.fn(() => ({ socketPath: "/tmp/ta-daemon.sock" })),
  initializeResponse: {
    daemonInstanceId: "daemon-1",
    clientCredential: "credential-1credential-1credential-1",
    daemonVersion: "0.0.1",
    protocolVersion: "2026-04-stage3",
    capabilities: {
      notifications: true,
      eventSubscriptions: true,
    },
  },
  requestCalls: [] as Array<{ method: string; params: Record<string, unknown> }>,
  requestHandlers: new Map<string, (params: Record<string, unknown>) => Promise<unknown>>(),
}));

vi.mock("../../packages/main/src/desktop-locator-config.js", () => hoisted);

vi.mock("../../packages/main/src/daemon-rpc-connection.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/main/src/daemon-rpc-connection.js")>();

  class FakeDaemonRpcConnection {
    constructor(
      private readonly options: {
        initializeConnection: () => Promise<void>;
      },
    ) {}

    async ensureConnected(): Promise<void> {
      await this.options.initializeConnection();
    }

    enqueueOperation<Result>(operation: () => Promise<Result>): Promise<Result> {
      return operation();
    }

    async request<Result>(
      method: string,
      params: Record<string, unknown>,
      parseResult: (value: unknown) => Result,
    ): Promise<Result> {
      hoisted.requestCalls.push({ method, params });
      if (method === METHOD_DAEMON_INITIALIZE) {
        return parseResult(hoisted.initializeResponse);
      }

      const handler = hoisted.requestHandlers.get(method);
      if (handler == null) {
        throw new Error(`unconfigured fake daemon request for ${method}`);
      }
      return parseResult(await handler(params));
    }

    dispose(): void {}
  }

  return {
    ...actual,
    DaemonRpcConnection: FakeDaemonRpcConnection,
  };
});

async function collectRelativeFiles(root: string, current = root): Promise<string[]> {
  const entries = await readdir(current, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        return collectRelativeFiles(root, path);
      }
      return [path.slice(root.length + 1)];
    }),
  );
  return files.flat();
}

function setProcessPlatform(platform: NodeJS.Platform): void {
  Object.defineProperty(process, "platform", {
    configurable: true,
    value: platform,
  });
}

function restoreProcessEnv(snapshot: NodeJS.ProcessEnv): void {
  for (const key of Object.keys(process.env)) {
    if (!(key in snapshot)) {
      delete process.env[key];
    }
  }
  for (const [key, value] of Object.entries(snapshot)) {
    if (value == null) {
      delete process.env[key];
      continue;
    }
    process.env[key] = value;
  }
}

describe("daemon local storage keys", () => {
  let tempHome = "";
  let previousEnv: NodeJS.ProcessEnv;
  let previousCwd = "";
  let originalPlatformDescriptor: PropertyDescriptor | undefined;

  beforeEach(async () => {
    vi.resetModules();
    tempHome = await mkdtemp(join(tmpdir(), "taugentic-daemon-storage-"));
    previousEnv = { ...process.env };
    previousCwd = process.cwd();
    originalPlatformDescriptor = Object.getOwnPropertyDescriptor(process, "platform");
    setProcessPlatform("darwin");
    process.env.HOME = tempHome;
    delete process.env.APPDATA;
    delete process.env.USERPROFILE;
    delete process.env.XDG_CONFIG_HOME;
    hoisted.initializeResponse = {
      daemonInstanceId: "daemon-1",
      clientCredential: "credential-1credential-1credential-1",
      daemonVersion: "0.0.1",
      protocolVersion: "2026-04-stage3",
      capabilities: {
        notifications: true,
        eventSubscriptions: true,
      },
    };
    hoisted.requestCalls.length = 0;
    hoisted.requestHandlers.clear();
  });

  afterEach(async () => {
    restoreProcessEnv(previousEnv);
    process.chdir(previousCwd);
    if (originalPlatformDescriptor) {
      Object.defineProperty(process, "platform", originalPlatformDescriptor);
    }
    await rm(tempHome, { recursive: true, force: true });
  });

  it("hashes clientName in persisted credential paths", async () => {
    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");

    await storeDesktopClientCredential("../evil/client", "credential-secretcredential-secret");

    const files = await collectRelativeFiles(
      join(tempHome, "Library", "Application Support", "taugentic", "desktop-daemon-clients"),
    );

    expect(await loadDesktopClientCredential("../evil/client")).toBe(
      "credential-secretcredential-secret",
    );
    expect(files).toHaveLength(1);
    expect(files[0]).toMatch(/^ta-daemon\/[a-f0-9]{64}\.credential$/u);
    expect(files[0]).not.toContain("../evil/client");
    if (process.platform !== "win32") {
      const storedPath = join(
        tempHome,
        "Library",
        "Application Support",
        "taugentic",
        "desktop-daemon-clients",
        files[0],
      );
      expect((await stat(dirname(storedPath))).mode & 0o777).toBe(0o700);
      expect((await stat(storedPath)).mode & 0o777).toBe(0o600);
    }
  });

  it("purges invalid persisted client credentials", async () => {
    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");

    await storeDesktopClientCredential("../evil/client", "credential-secretcredential-secret");

    const files = await collectRelativeFiles(
      join(tempHome, "Library", "Application Support", "taugentic", "desktop-daemon-clients"),
    );
    expect(files).toHaveLength(1);

    const storedPath = join(
      tempHome,
      "Library",
      "Application Support",
      "taugentic",
      "desktop-daemon-clients",
      files[0],
    );
    await writeFile(storedPath, "short", "utf8");

    expect(await loadDesktopClientCredential("../evil/client")).toBeNull();
    expect(
      await collectRelativeFiles(
        join(tempHome, "Library", "Application Support", "taugentic", "desktop-daemon-clients"),
      ),
    ).toHaveLength(0);
  });

  it("hashes clientName and sessionId in persisted authority paths", async () => {
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopSessionAuthority(
      "../evil/client",
      "../session/../../owned" as never,
      "session-authority-1session-authority-1" as never,
    );

    const files = await collectRelativeFiles(
      join(
        tempHome,
        "Library",
        "Application Support",
        "taugentic",
        "desktop-daemon-session-authorities",
      ),
    );

    expect(
      await loadDesktopSessionAuthority("../evil/client", "../session/../../owned" as never),
    ).toBe("session-authority-1session-authority-1");
    expect(files).toHaveLength(1);
    expect(files[0]).toMatch(/^ta-daemon\/[a-f0-9]{64}\/[a-f0-9]{64}\.authority$/u);
    expect(files[0]).not.toContain("../evil/client");
    expect(files[0]).not.toContain("../session/../../owned");
    if (process.platform !== "win32") {
      const storedPath = join(
        tempHome,
        "Library",
        "Application Support",
        "taugentic",
        "desktop-daemon-session-authorities",
        files[0],
      );
      expect((await stat(dirname(storedPath))).mode & 0o777).toBe(0o700);
      expect((await stat(storedPath)).mode & 0o777).toBe(0o600);
    }
  });

  it("purges invalid persisted session authorities", async () => {
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopSessionAuthority(
      "../evil/client",
      "../session/../../owned" as never,
      "session-authority-1session-authority-1" as never,
    );

    const files = await collectRelativeFiles(
      join(
        tempHome,
        "Library",
        "Application Support",
        "taugentic",
        "desktop-daemon-session-authorities",
      ),
    );
    expect(files).toHaveLength(1);

    const storedPath = join(
      tempHome,
      "Library",
      "Application Support",
      "taugentic",
      "desktop-daemon-session-authorities",
      files[0],
    );
    await writeFile(storedPath, "   ", "utf8");

    expect(
      await loadDesktopSessionAuthority("../evil/client", "../session/../../owned" as never),
    ).toBeNull();
    expect(
      await collectRelativeFiles(
        join(
          tempHome,
          "Library",
          "Application Support",
          "taugentic",
          "desktop-daemon-session-authorities",
        ),
      ),
    ).toHaveLength(0);
  });

  it("persists open-session authority and reloads it from disk on a fresh connection", async () => {
    const workspace = {
      kind: "byPath",
      path: "/tmp/taugentic-storage-key-workspace",
      trustAcknowledged: true,
    } as const;
    hoisted.requestHandlers.set(METHOD_DAEMON_SESSION_OPEN, async (params) => {
      expect(params).toEqual({
        title: "Build daemon app server",
        workspace,
      });
      return {
        session: {
          id: "session-1",
          title: "Build daemon app server",
          status: "idle",
        },
        latestCursor: null,
        sessionAuthority: "session-authority-1session-authority-1",
      };
    });
    hoisted.requestHandlers.set(METHOD_DAEMON_SESSION_ATTACH, async (params) => {
      expect(params).toEqual({
        sessionId: "session-1",
        sessionAuthority: "session-authority-1session-authority-1",
      });
      return {
        session: {
          id: "session-1",
          title: "Build daemon app server",
          status: "idle",
        },
        latestCursor: null,
        sessionAuthority: "session-authority-2session-authority-2",
      };
    });

    const { DaemonSessionRequestClient } =
      await import("../../packages/main/src/daemon-session-request-client.js");
    const { DaemonSessionConnection } =
      await import("../../packages/main/src/daemon-session-connection.js");
    const { DAEMON_REQUEST_TIMEOUT_DISABLED } =
      await import("../../packages/main/src/daemon-rpc-connection.js");
    const { loadDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    const client = new DaemonSessionRequestClient(null, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
    });
    const opened = await client.openSession("Build daemon app server", workspace);
    expect(opened.id).toBe("session-1");
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-1session-authority-1",
    );

    const freshConnection = new DaemonSessionConnection("session-1" as never, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
    });
    await freshConnection.initializeConnection();

    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-2session-authority-2",
    );
    expect(
      hoisted.requestCalls.filter(({ method }) => method === METHOD_DAEMON_SESSION_ATTACH),
    ).toHaveLength(1);
  });

  it("credential rotation purges all cached session authorities from disk before attach", async () => {
    const { storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");
    const { DaemonSessionConnection } =
      await import("../../packages/main/src/daemon-session-connection.js");
    const { DAEMON_REQUEST_TIMEOUT_DISABLED, DaemonProtocolError } =
      await import("../../packages/main/src/daemon-rpc-connection.js");

    await storeDesktopClientCredential(
      "desktop-main",
      "credential-oldcredential-oldcredential-old",
    );
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-2" as never,
      "session-authority-2session-authority-2" as never,
    );
    hoisted.initializeResponse = {
      ...hoisted.initializeResponse,
      clientCredential: "credential-newcredential-newcredential-new",
    };

    const freshConnection = new DaemonSessionConnection("session-1" as never, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
    });

    const initializePromise = freshConnection.initializeConnection();
    await expect(initializePromise).rejects.toThrow(DaemonProtocolError);
    await expect(initializePromise).rejects.toThrow(
      "missing local session authority for session-1",
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBeNull();
    expect(await loadDesktopSessionAuthority("desktop-main", "session-2" as never)).toBeNull();
    expect(
      hoisted.requestCalls.filter(({ method }) => method === METHOD_DAEMON_SESSION_ATTACH),
    ).toHaveLength(0);
  });

  it("purges a stale persisted session authority from disk after terminal attach denial", async () => {
    const { storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");
    const { DaemonSessionConnection } =
      await import("../../packages/main/src/daemon-session-connection.js");
    const { DAEMON_REQUEST_TIMEOUT_DISABLED, DaemonJsonRpcError, DaemonProtocolError } =
      await import("../../packages/main/src/daemon-rpc-connection.js");

    await storeDesktopClientCredential("desktop-main", "credential-1credential-1credential-1");
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );
    hoisted.requestHandlers.set(METHOD_DAEMON_SESSION_ATTACH, async (params) => {
      expect(params).toEqual({
        sessionId: "session-1",
        sessionAuthority: "session-authority-1session-authority-1",
      });
      throw new DaemonJsonRpcError(-32_602, "session does not exist: session-1");
    });

    const freshConnection = new DaemonSessionConnection("session-1" as never, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
    });

    await expect(freshConnection.initializeConnection()).rejects.toThrow(
      new DaemonJsonRpcError(-32_602, "session does not exist: session-1"),
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBeNull();
    expect(
      await collectRelativeFiles(
        join(
          tempHome,
          "Library",
          "Application Support",
          "taugentic",
          "desktop-daemon-session-authorities",
        ),
      ),
    ).toHaveLength(0);

    await expect(freshConnection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError("missing local session authority for session-1"),
    );
    expect(
      hoisted.requestCalls.filter(({ method }) => method === METHOD_DAEMON_SESSION_ATTACH),
    ).toHaveLength(1);
  });

  it("keeps persisted daemon storage rooted in APPDATA across packaged path changes on win32", async () => {
    setProcessPlatform("win32");
    process.env.APPDATA = join(tempHome, "AppData", "Roaming");
    process.env.USERPROFILE = join(tempHome, "UserProfile");

    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopClientCredential("desktop-main", "credential-1credential-1credential-1");
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );

    const persistedRoot = join(process.env.APPDATA, "taugentic");
    process.chdir(await mkdtemp(join(tmpdir(), "taugentic-packaged-win32-")));

    expect(await loadDesktopClientCredential("desktop-main")).toBe(
      "credential-1credential-1credential-1",
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-1session-authority-1",
    );
    await expect(stat(join(persistedRoot, "desktop-daemon-clients"))).resolves.toBeTruthy();
    await expect(
      stat(join(persistedRoot, "desktop-daemon-session-authorities")),
    ).resolves.toBeTruthy();
  });

  it("keeps persisted daemon storage rooted in XDG_CONFIG_HOME across packaged path changes on linux", async () => {
    setProcessPlatform("linux");
    process.env.HOME = join(tempHome, "home");
    process.env.XDG_CONFIG_HOME = join(tempHome, "xdg-config");

    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopClientCredential("desktop-main", "credential-1credential-1credential-1");
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );

    const persistedRoot = join(process.env.XDG_CONFIG_HOME, "taugentic");
    process.chdir(await mkdtemp(join(tmpdir(), "taugentic-packaged-linux-")));

    expect(await loadDesktopClientCredential("desktop-main")).toBe(
      "credential-1credential-1credential-1",
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-1session-authority-1",
    );
    await expect(stat(join(persistedRoot, "desktop-daemon-clients"))).resolves.toBeTruthy();
    await expect(
      stat(join(persistedRoot, "desktop-daemon-session-authorities")),
    ).resolves.toBeTruthy();
  });

  it("falls back to USERPROFILE AppData Roaming for persisted daemon storage on win32", async () => {
    setProcessPlatform("win32");
    delete process.env.APPDATA;
    process.env.USERPROFILE = join(tempHome, "UserProfile");

    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopClientCredential("desktop-main", "credential-1credential-1credential-1");
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );

    const fallbackRoot = join(process.env.USERPROFILE, "AppData", "Roaming", "taugentic");
    const packagedCwd = await mkdtemp(join(tmpdir(), "taugentic-packaged-win32-fallback-"));
    process.chdir(packagedCwd);

    expect(await loadDesktopClientCredential("desktop-main")).toBe(
      "credential-1credential-1credential-1",
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-1session-authority-1",
    );
    await expect(stat(join(fallbackRoot, "desktop-daemon-clients"))).resolves.toBeTruthy();
    await expect(
      stat(join(fallbackRoot, "desktop-daemon-session-authorities")),
    ).resolves.toBeTruthy();
    await expect(stat(join(packagedCwd, "taugentic"))).rejects.toThrow();
  });

  it("falls back to HOME .config for persisted daemon storage on linux", async () => {
    setProcessPlatform("linux");
    process.env.HOME = join(tempHome, "home");
    delete process.env.XDG_CONFIG_HOME;

    const { loadDesktopClientCredential, storeDesktopClientCredential } =
      await import("../../packages/main/src/daemon-client-credential.js");
    const { loadDesktopSessionAuthority, storeDesktopSessionAuthority } =
      await import("../../packages/main/src/daemon-session-authority.js");

    await storeDesktopClientCredential("desktop-main", "credential-1credential-1credential-1");
    await storeDesktopSessionAuthority(
      "desktop-main",
      "session-1" as never,
      "session-authority-1session-authority-1" as never,
    );

    const fallbackRoot = join(process.env.HOME, ".config", "taugentic");
    const packagedCwd = await mkdtemp(join(tmpdir(), "taugentic-packaged-linux-fallback-"));
    process.chdir(packagedCwd);

    expect(await loadDesktopClientCredential("desktop-main")).toBe(
      "credential-1credential-1credential-1",
    );
    expect(await loadDesktopSessionAuthority("desktop-main", "session-1" as never)).toBe(
      "session-authority-1session-authority-1",
    );
    await expect(stat(join(fallbackRoot, "desktop-daemon-clients"))).resolves.toBeTruthy();
    await expect(
      stat(join(fallbackRoot, "desktop-daemon-session-authorities")),
    ).resolves.toBeTruthy();
    await expect(stat(join(packagedCwd, "taugentic"))).rejects.toThrow();
  });
});
