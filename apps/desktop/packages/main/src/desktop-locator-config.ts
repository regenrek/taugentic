import { tmpdir } from "node:os";

import {
  DAEMON_DEFAULT_SOCKET_NAME,
  DAEMON_SOCKET_NAME_ENV_VAR,
  resolveDaemonSocketName,
  resolveDaemonSocketPathForPlatform,
} from "@taugentic/desktop-shared";

export interface DesktopDaemonLocatorConfig {
  socketPath: string;
}

export interface CreateDesktopDaemonLocatorConfigOptions {
  env?: NodeJS.ProcessEnv;
  platform?: NodeJS.Platform;
  tempDir?: string;
  userId?: number;
}

export function createDesktopDaemonLocatorConfig(
  options: CreateDesktopDaemonLocatorConfigOptions = {},
): DesktopDaemonLocatorConfig {
  const env = options.env ?? process.env;
  const platform = options.platform ?? process.platform;
  const resolvedAppName = resolveDaemonSocketName(
    DAEMON_DEFAULT_SOCKET_NAME,
    env[DAEMON_SOCKET_NAME_ENV_VAR],
  );
  const uid = options.userId ?? (platform === "win32" ? undefined : process.getuid?.());
  const resolvedTempDir = options.tempDir ?? tmpdir();

  return {
    socketPath: resolveDaemonSocketPathForPlatform(
      platform,
      resolvedAppName,
      env.XDG_RUNTIME_DIR,
      resolvedTempDir,
      uid,
      env.HOME,
    ),
  };
}
