import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";

import {
  DAEMON_DEFAULT_SOCKET_NAME,
  type SessionAuthority,
  type SessionId,
} from "@taugentic/desktop-shared";
import {
  ProtocolValidationError,
  parseSessionAuthority,
} from "@taugentic/desktop-shared/validation";

import { createDesktopDaemonLocatorConfig } from "./desktop-locator-config.js";
import { clientStorageKey, sessionStorageKey } from "./daemon-storage-key.js";
import {
  deletePrivateStorageDirectory,
  deletePrivateStorageFile,
  writePrivateStorageFile,
} from "./private-storage.js";

export async function loadDesktopSessionAuthority(
  clientName: string,
  sessionId: SessionId,
): Promise<SessionAuthority | null> {
  const { baseDirectory, pathSegments } = sessionAuthorityStorageLocation(clientName, sessionId);
  try {
    const stored = await readFile(join(baseDirectory, ...pathSegments), "utf8");
    return parseSessionAuthority(stored.trim());
  } catch (error: unknown) {
    if (isMissingFileError(error)) {
      return null;
    }
    if (error instanceof ProtocolValidationError) {
      await deletePrivateStorageFile(baseDirectory, pathSegments);
      return null;
    }
    throw error;
  }
}

export async function storeDesktopSessionAuthority(
  clientName: string,
  sessionId: SessionId,
  sessionAuthority: SessionAuthority,
): Promise<void> {
  const { baseDirectory, pathSegments } = sessionAuthorityStorageLocation(clientName, sessionId);
  await writePrivateStorageFile(baseDirectory, pathSegments, sessionAuthority);
}

export async function removeDesktopSessionAuthority(
  clientName: string,
  sessionId: SessionId,
): Promise<void> {
  const { baseDirectory, pathSegments } = sessionAuthorityStorageLocation(clientName, sessionId);
  await deletePrivateStorageFile(baseDirectory, pathSegments);
}

export async function removeDesktopClientSessionAuthorities(clientName: string): Promise<void> {
  const { baseDirectory, pathSegments } = sessionAuthorityClientStorageLocation(clientName);
  await deletePrivateStorageDirectory(baseDirectory, pathSegments);
}

function sessionAuthorityStorageLocation(
  clientName: string,
  sessionId: SessionId,
): { baseDirectory: string; pathSegments: string[] } {
  const clientLocation = sessionAuthorityClientStorageLocation(clientName);
  return {
    baseDirectory: clientLocation.baseDirectory,
    pathSegments: [...clientLocation.pathSegments, `${sessionStorageKey(sessionId)}.authority`],
  };
}

function sessionAuthorityClientStorageLocation(clientName: string): {
  baseDirectory: string;
  pathSegments: string[];
} {
  const { socketPath } = createDesktopDaemonLocatorConfig();
  const socketName = socketPathName(socketPath);
  return {
    baseDirectory: sessionAuthorityBaseDir(socketPath, socketName),
    pathSegments: [socketName, clientStorageKey(clientName)],
  };
}

function sessionAuthorityBaseDir(socketPath: string, socketName: string): string {
  if (socketName === DAEMON_DEFAULT_SOCKET_NAME) {
    return join(defaultConfigBaseDir(), "taugentic", "desktop-daemon-session-authorities");
  }
  return join(dirname(socketPath), "taugentic-session-authorities");
}

function defaultConfigBaseDir(): string {
  if (process.platform === "darwin") {
    return join(
      normalizedEnvPath(process.env.HOME) ?? process.cwd(),
      "Library",
      "Application Support",
    );
  }
  if (process.platform === "win32") {
    return (
      normalizedEnvPath(process.env.APPDATA) ??
      join(normalizedEnvPath(process.env.USERPROFILE) ?? process.cwd(), "AppData", "Roaming")
    );
  }
  return (
    normalizedEnvPath(process.env.XDG_CONFIG_HOME) ??
    join(normalizedEnvPath(process.env.HOME) ?? process.cwd(), ".config")
  );
}

function normalizedEnvPath(value: string | undefined): string | null {
  const normalized = value?.trim();
  return normalized == null || normalized.length === 0 ? null : normalized;
}

function socketPathName(socketPath: string): string {
  const fileName = socketPath.split(/[\\/]/).pop() ?? DAEMON_DEFAULT_SOCKET_NAME;
  return fileName.replace(/\.sock$/u, "");
}

function isMissingFileError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
