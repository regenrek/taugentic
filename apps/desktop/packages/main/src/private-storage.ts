import { chmod, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const PRIVATE_DIRECTORY_MODE = 0o700;
const PRIVATE_FILE_MODE = 0o600;

export async function writePrivateStorageFile(
  baseDirectory: string,
  pathSegments: readonly string[],
  contents: string,
): Promise<string> {
  if (pathSegments.length === 0) {
    throw new Error("private storage path must not be empty");
  }

  let currentDirectory = baseDirectory;
  await ensurePrivateDirectory(currentDirectory);
  for (const segment of pathSegments.slice(0, -1)) {
    currentDirectory = join(currentDirectory, segment);
    await ensurePrivateDirectory(currentDirectory);
  }

  const filePath = join(baseDirectory, ...pathSegments);
  await writeFile(filePath, contents, { encoding: "utf8", mode: PRIVATE_FILE_MODE });
  await enforcePrivateMode(filePath, PRIVATE_FILE_MODE);
  return filePath;
}

export async function deletePrivateStorageFile(
  baseDirectory: string,
  pathSegments: readonly string[],
): Promise<void> {
  if (pathSegments.length === 0) {
    throw new Error("private storage path must not be empty");
  }

  await rm(join(baseDirectory, ...pathSegments), { force: true });
}

export async function deletePrivateStorageDirectory(
  baseDirectory: string,
  pathSegments: readonly string[],
): Promise<void> {
  if (pathSegments.length === 0) {
    throw new Error("private storage path must not be empty");
  }

  await rm(join(baseDirectory, ...pathSegments), { recursive: true, force: true });
}

async function ensurePrivateDirectory(path: string): Promise<void> {
  await mkdir(path, { recursive: true, mode: PRIVATE_DIRECTORY_MODE });
  await enforcePrivateMode(path, PRIVATE_DIRECTORY_MODE);
}

async function enforcePrivateMode(path: string, mode: number): Promise<void> {
  if (process.platform === "win32") {
    return;
  }
  try {
    await chmod(path, mode);
  } catch (error: unknown) {
    if (isMissingPathError(error)) {
      return;
    }
    throw error;
  }
}

function isMissingPathError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
