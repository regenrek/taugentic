import { describe, expect, it } from "vite-plus/test";

import { createDesktopDaemonLocatorConfig } from "../../packages/main/src/desktop-locator-config.js";

describe("desktop-locator-config", () => {
  it("derives socket path from the locator owner without hidden globals", () => {
    const config = createDesktopDaemonLocatorConfig({
      env: {
        XDG_RUNTIME_DIR: "/run/user/501",
        TAUGENTIC_DAEMON_SOCKET_NAME: "desktop.sock",
      },
      platform: "linux",
    });

    expect(config).toEqual({
      socketPath: "/run/user/501/desktop.sock.sock",
    });
  });

  it("uses injected locator facts instead of hidden process globals", () => {
    const config = createDesktopDaemonLocatorConfig({
      env: {
        TAUGENTIC_DAEMON_SOCKET_NAME: "desktop.sock",
      },
      platform: "linux",
      tempDir: "/tmp/custom",
      userId: 42,
    });

    expect(config).toEqual({
      socketPath: "/tmp/custom/taugentic-uid-42/desktop.sock.sock",
    });
  });
});
