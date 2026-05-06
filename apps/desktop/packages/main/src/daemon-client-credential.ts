import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";

import { DAEMON_DEFAULT_SOCKET_NAME } from "@taugentic/desktop-shared";
import {
  ProtocolValidationError,
  parseClientCredential,
} from "@taugentic/desktop-shared/validation";

import { createDesktopDaemonLocatorConfig } from "./desktop-locator-config.js";
import { clientStorageKey } from "./daemon-storage-key.js";
import { deletePrivateStorageFile, writePrivateStorageFile } from "./private-storage.js";

export async function loadDesktopClientCredential(clientName: string): Promise<string | null> {
  const { baseDirectory, pathSegments } = clientCredentialStorageLocation(clientName);
  try {
    const stored = await readFile(join(baseDirectory, ...pathSegments), "utf8");
    return parseClientCredential(stored);
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

export async function storeDesktopClientCredential(
  clientName: string,
  clientCredential: string,
): Promise<void> {
  const { baseDirectory, pathSegments } = clientCredentialStorageLocation(clientName);
  await writePrivateStorageFile(baseDirectory, pathSegments, clientCredential);
}

function clientCredentialStorageLocation(clientName: string): {
  baseDirectory: string;
  pathSegments: string[];
} {
  const { socketPath } = createDesktopDaemonLocatorConfig();
  const socketName = socketPathName(socketPath);
  return {
    baseDirectory: clientCredentialBaseDir(socketPath, socketName),
    pathSegments: [socketName, `${clientStorageKey(clientName)}.credential`],
  };
}

function clientCredentialBaseDir(socketPath: string, socketName: string): string {
  if (socketName === DAEMON_DEFAULT_SOCKET_NAME) {
    return join(defaultConfigBaseDir(), "taugentic", "desktop-daemon-clients");
  }
  return join(dirname(socketPath), "taugentic-client-credentials");
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
