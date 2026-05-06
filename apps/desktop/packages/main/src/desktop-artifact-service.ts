import { BrowserWindow, dialog } from "electron";
import { copyFile, realpath } from "node:fs/promises";
import { basename, isAbsolute, normalize, resolve as resolvePath, sep } from "node:path";

import {
  DEFAULT_READ_ARTIFACT_MAX_BYTES,
  MAX_READ_ARTIFACT_MAX_BYTES,
} from "@taugentic/desktop-shared";
import type {
  ArtifactId,
  ArtifactSummary,
  ReadArtifactContentQuery,
  ReadArtifactContentResult,
  SaveArtifactAsQuery,
  SaveArtifactAsResult,
} from "@taugentic/desktop-shared";
import { parseSessionId } from "@taugentic/desktop-shared/validation";

/**
 * Dependency surface for artifact IO. Extracted so unit tests can supply an
 * in-memory filesystem without touching the real disk, and so the production
 * implementation stays the sole place that touches `node:fs` and `electron`.
 */
export interface DesktopArtifactIo {
  statFile(path: string): Promise<DesktopArtifactFileStat>;
  readFile(path: string, maxBytes: number): Promise<DesktopArtifactReadResult>;
  copyFile(source: string, destination: string): Promise<void>;
  showSaveDialog(
    options: DesktopArtifactSaveDialogOptions,
  ): Promise<DesktopArtifactSaveDialogResult>;
  /**
   * Resolves any symlinks in the supplied absolute path. Used for
   * containment checks so that a symlinked escape is caught at read time.
   */
  realpath?(path: string): Promise<string>;
}

export interface DesktopArtifactFileStat {
  readonly kind: "file" | "directory" | "symlink" | "other" | "notFound";
  readonly size: number;
}

export type DesktopArtifactReadResult =
  | {
      readonly status: "ok";
      readonly bytes: Buffer;
      readonly truncated: boolean;
    }
  | {
      readonly status: "notFound";
    };

export interface DesktopArtifactSaveDialogOptions {
  readonly defaultPath: string;
  readonly parentWindow: BrowserWindow | null;
}

export type DesktopArtifactSaveDialogResult =
  | {
      readonly status: "saved";
      readonly path: string;
    }
  | {
      readonly status: "cancelled";
    };

export interface DesktopArtifactServiceDeps {
  readonly io: DesktopArtifactIo;
  readonly getArtifact: (
    query: SaveArtifactAsQuery | ReadArtifactContentQuery,
  ) => Promise<ArtifactSummary | null>;
  /**
   * Main-owned artifact storage root. Relative `ArtifactSummary.storagePath`
   * values are resolved against this directory; absolute paths must stay
   * inside it to satisfy containment. When `null`, only absolute paths are
   * accepted and containment is enforced via `..` rejection + symlink
   * resolution only (backward-compatible path for early bootstrap before
   * the daemon has been queried for its artifact root).
   */
  readonly artifactRoot: string | null;
}

/**
 * Outcome of resolving a daemon-supplied `ArtifactSummary.storagePath` into
 * a safe absolute path on the main process side.
 *
 * The resolution is deliberately tolerant of both contracts that exist in
 * the repo today: the Rust `ArtifactWriter` canonicalizes to an absolute
 * path at write time, while shared validation fixtures and store tests use
 * relative paths like `artifacts/run-1/patch.diff`. Both are accepted; in
 * either case, the final resolved path is containment-checked against
 * `artifactRoot` (when known).
 */
export type DesktopArtifactPathResolution =
  | { readonly kind: "resolved"; readonly absolutePath: string }
  | { readonly kind: "invalid"; readonly reason: string };

/**
 * Pure path-resolution helper. Extracted for unit testing so we can cover
 * relative/absolute/escape/windows cases without touching the real
 * filesystem.
 */
export function resolveArtifactStoragePath(
  storagePath: unknown,
  artifactRoot: string | null,
): DesktopArtifactPathResolution {
  if (typeof storagePath !== "string" || storagePath.length === 0) {
    return { kind: "invalid", reason: "storagePath must be a non-empty string" };
  }
  if (storagePath.includes("\0")) {
    return { kind: "invalid", reason: "storagePath must not contain null bytes" };
  }
  if (hasParentTraversalSegment(storagePath)) {
    return {
      kind: "invalid",
      reason: "storagePath must not contain '..' path segments",
    };
  }

  const absoluteInput = isAbsolute(storagePath);

  if (!absoluteInput && artifactRoot === null) {
    return {
      kind: "invalid",
      reason:
        "relative storagePath requires an artifact root on the main process; daemon provided a logical path but the root is unknown",
    };
  }

  const root = artifactRoot === null ? null : normalize(stripTrailingSeparator(artifactRoot));

  const joined = absoluteInput
    ? normalize(storagePath)
    : normalize(resolvePath(root as string, storagePath));

  if (root !== null && !pathIsInside(joined, root)) {
    return {
      kind: "invalid",
      reason: `resolved artifact path ${joined} escapes artifact root ${root}`,
    };
  }

  return { kind: "resolved", absolutePath: joined };
}

/**
 * Core artifact content read. Pure with respect to the injected `io` surface;
 * callers supply daemon-authenticated `summary.storagePath` values.
 */
export async function performReadArtifactContent(
  summary: ArtifactSummary,
  maxBytes: number,
  io: DesktopArtifactIo,
  artifactRoot: string | null,
): Promise<ReadArtifactContentResult> {
  const resolution = resolveArtifactStoragePath(summary.storagePath, artifactRoot);
  if (resolution.kind === "invalid") {
    throw new Error(resolution.reason);
  }
  const safePath = await enforceRealpathContainment(resolution.absolutePath, artifactRoot, io);
  if (safePath.kind === "invalid") {
    throw new Error(safePath.reason);
  }

  const stat = await io.statFile(safePath.absolutePath);

  if (stat.kind === "notFound") {
    return { status: "missing", reason: "fileNotFound" };
  }
  if (stat.kind !== "file") {
    return { status: "missing", reason: "fileNotFound" };
  }

  if (stat.size > maxBytes) {
    return {
      status: "tooLarge",
      kind: summary.kind,
      storagePath: summary.storagePath,
      totalBytes: stat.size,
      limitBytes: maxBytes,
    };
  }

  const read = await io.readFile(safePath.absolutePath, maxBytes);
  if (read.status === "notFound") {
    return { status: "missing", reason: "fileNotFound" };
  }

  return {
    status: "inline",
    kind: summary.kind,
    storagePath: summary.storagePath,
    totalBytes: stat.size,
    readBytes: read.bytes.byteLength,
    truncated: read.truncated,
    encoding: "utf-8",
    content: read.bytes.toString("utf-8"),
  };
}

/**
 * Core artifact save-to-disk flow. Prompts the user for a destination then
 * copies the daemon-owned file via the injected IO surface.
 */
export async function performSaveArtifactAs(
  summary: ArtifactSummary,
  suggestedFilename: string | undefined,
  parentWindow: BrowserWindow | null,
  io: DesktopArtifactIo,
  artifactRoot: string | null,
): Promise<SaveArtifactAsResult> {
  const resolution = resolveArtifactStoragePath(summary.storagePath, artifactRoot);
  if (resolution.kind === "invalid") {
    throw new Error(resolution.reason);
  }
  const safePath = await enforceRealpathContainment(resolution.absolutePath, artifactRoot, io);
  if (safePath.kind === "invalid") {
    throw new Error(safePath.reason);
  }

  const stat = await io.statFile(safePath.absolutePath);

  if (stat.kind === "notFound" || stat.kind !== "file") {
    return { status: "missing", reason: "fileNotFound" };
  }

  const defaultFilename =
    suggestedFilename ?? defaultArtifactFilename(summary, safePath.absolutePath);
  const dialogResult = await io.showSaveDialog({
    defaultPath: defaultFilename,
    parentWindow,
  });

  if (dialogResult.status === "cancelled") {
    return { status: "cancelled" };
  }

  await io.copyFile(safePath.absolutePath, dialogResult.path);
  return {
    status: "saved",
    savedPath: dialogResult.path,
    bytesCopied: stat.size,
  };
}

/**
 * High-level read handler used by the main RPC layer.
 */
export async function handleReadArtifactContent(
  query: unknown,
  deps: DesktopArtifactServiceDeps,
): Promise<ReadArtifactContentResult> {
  const parsed = parseReadArtifactContentQuery(query);
  const summary = await deps.getArtifact(parsed);
  if (summary === null) {
    return { status: "missing", reason: "artifactNotFound" };
  }
  const maxBytes = resolveReadArtifactMaxBytes(parsed.maxBytes);
  return performReadArtifactContent(summary, maxBytes, deps.io, deps.artifactRoot);
}

/**
 * High-level save handler used by the main RPC layer.
 */
export async function handleSaveArtifactAs(
  query: unknown,
  parentWindow: BrowserWindow | null,
  deps: DesktopArtifactServiceDeps,
): Promise<SaveArtifactAsResult> {
  const parsed = parseSaveArtifactAsQuery(query);
  const summary = await deps.getArtifact(parsed);
  if (summary === null) {
    return { status: "missing", reason: "artifactNotFound" };
  }
  return performSaveArtifactAs(
    summary,
    parsed.suggestedFilename,
    parentWindow,
    deps.io,
    deps.artifactRoot,
  );
}

/**
 * Production IO surface. Isolated here so tests can build fakes without
 * linking real `fs` / `electron` behavior.
 */
export function createProductionDesktopArtifactIo(): DesktopArtifactIo {
  return {
    async statFile(path) {
      const { stat, lstat } = await import("node:fs/promises");
      try {
        const linkStat = await lstat(path);
        if (linkStat.isSymbolicLink()) {
          return { kind: "symlink", size: 0 };
        }
        const s = await stat(path);
        if (s.isDirectory()) {
          return { kind: "directory", size: s.size };
        }
        if (s.isFile()) {
          return { kind: "file", size: s.size };
        }
        return { kind: "other", size: s.size };
      } catch (error) {
        if (isNodeErrnoException(error) && error.code === "ENOENT") {
          return { kind: "notFound", size: 0 };
        }
        throw error;
      }
    },
    async readFile(path, maxBytes) {
      const { open } = await import("node:fs/promises");
      let handle;
      try {
        handle = await open(path, "r");
      } catch (error) {
        if (isNodeErrnoException(error) && error.code === "ENOENT") {
          return { status: "notFound" };
        }
        throw error;
      }
      try {
        const buffer = Buffer.alloc(maxBytes);
        const { bytesRead } = await handle.read(buffer, 0, maxBytes, 0);
        const slice = buffer.subarray(0, bytesRead);
        const stat = await handle.stat();
        const truncated = stat.size > bytesRead;
        return { status: "ok", bytes: slice, truncated };
      } finally {
        await handle.close();
      }
    },
    async copyFile(source, destination) {
      await copyFile(source, destination);
    },
    async showSaveDialog({ defaultPath, parentWindow }) {
      const result = parentWindow
        ? await dialog.showSaveDialog(parentWindow, { defaultPath })
        : await dialog.showSaveDialog({ defaultPath });
      if (result.canceled || !result.filePath) {
        return { status: "cancelled" };
      }
      return { status: "saved", path: result.filePath };
    },
    async realpath(path) {
      try {
        return await realpath(path);
      } catch (error) {
        if (isNodeErrnoException(error) && error.code === "ENOENT") {
          return path;
        }
        throw error;
      }
    },
  };
}

/**
 * Resolve the focused BrowserWindow, if any, for sheet-attachment on macOS.
 */
export function resolveFocusedBrowserWindow(): BrowserWindow | null {
  return BrowserWindow.getFocusedWindow?.() ?? null;
}

/**
 * Strongest containment check: canonicalize the resolved path on disk (if
 * the io surface provides `realpath`) and verify it still lives under
 * `artifactRoot`. Catches symlink-based escapes.
 *
 * When no artifact root is configured or the io does not expose realpath,
 * the passthrough resolution is returned unchanged; the callers will still
 * apply regular-file / symlink stat checks before reading.
 */
async function enforceRealpathContainment(
  absolutePath: string,
  artifactRoot: string | null,
  io: DesktopArtifactIo,
): Promise<DesktopArtifactPathResolution> {
  if (artifactRoot === null || io.realpath === undefined) {
    return { kind: "resolved", absolutePath };
  }
  const canonicalFile = await io.realpath(absolutePath);
  const canonicalRoot = await io.realpath(artifactRoot).catch(() => artifactRoot);
  if (!pathIsInside(canonicalFile, canonicalRoot)) {
    return {
      kind: "invalid",
      reason: `artifact canonical path ${canonicalFile} escapes artifact root ${canonicalRoot}`,
    };
  }
  return { kind: "resolved", absolutePath: canonicalFile };
}

function hasParentTraversalSegment(inputPath: string): boolean {
  // Reject `..` segments regardless of path separator. Normalize internally
  // on forward slashes so both posix and win32 inputs are covered.
  const normalizedForCheck = inputPath.replace(/\\/g, "/");
  return normalizedForCheck.split("/").some((segment) => segment === "..");
}

function stripTrailingSeparator(value: string): string {
  if (value.length <= 1) {
    return value;
  }
  if (value.endsWith(sep) || value.endsWith("/")) {
    return value.slice(0, -1);
  }
  return value;
}

function pathIsInside(candidate: string, root: string): boolean {
  const normalizedCandidate = normalize(candidate);
  const normalizedRoot = normalize(stripTrailingSeparator(root));
  if (normalizedCandidate === normalizedRoot) {
    return true;
  }
  return (
    normalizedCandidate.startsWith(normalizedRoot + sep) ||
    normalizedCandidate.startsWith(normalizedRoot + "/")
  );
}

function defaultArtifactFilename(summary: ArtifactSummary, resolvedPath: string): string {
  const base = basename(resolvedPath);
  if (base.length > 0) {
    return base;
  }
  const extension = suggestExtensionForKind(summary.kind);
  return `${summary.id}${extension}`;
}

function suggestExtensionForKind(kind: ArtifactSummary["kind"]): string {
  switch (kind) {
    case "Patch":
      return ".diff";
    case "CommandLog":
      return ".log";
    case "Transcript":
      return ".md";
    case "FileSnapshot":
      return ".txt";
    default: {
      const _exhaustive: never = kind;
      return "";
    }
  }
}

function parseReadArtifactContentQuery(value: unknown): ReadArtifactContentQuery {
  const record = parseRecord(value, "ReadArtifactContentQuery");
  return {
    sessionId: parseSessionId(record.sessionId),
    artifactId: parseArtifactId(record.artifactId),
    maxBytes: parseOptionalMaxBytes(record.maxBytes),
  };
}

function parseSaveArtifactAsQuery(value: unknown): SaveArtifactAsQuery {
  const record = parseRecord(value, "SaveArtifactAsQuery");
  return {
    sessionId: parseSessionId(record.sessionId),
    artifactId: parseArtifactId(record.artifactId),
    suggestedFilename: parseOptionalSuggestedFilename(record.suggestedFilename),
  };
}

function resolveReadArtifactMaxBytes(maxBytes: number | undefined): number {
  return maxBytes ?? DEFAULT_READ_ARTIFACT_MAX_BYTES;
}

function parseRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  throw new Error(`${label} must be an object`);
}

function parseArtifactId(value: unknown): ArtifactId {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error("ArtifactId must be a non-empty string");
  }
  return value as ArtifactId;
}

function parseOptionalMaxBytes(value: unknown): number | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (
    typeof value !== "number" ||
    !Number.isInteger(value) ||
    value <= 0 ||
    value > MAX_READ_ARTIFACT_MAX_BYTES
  ) {
    throw new Error(`maxBytes must be an integer between 1 and ${MAX_READ_ARTIFACT_MAX_BYTES}`);
  }
  return value;
}

function parseOptionalSuggestedFilename(value: unknown): string | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("suggestedFilename must be a non-empty string");
  }
  if (value.includes("/") || value.includes("\\") || value.includes("\0")) {
    throw new Error("suggestedFilename must not include path separators");
  }
  return value;
}

function isNodeErrnoException(error: unknown): error is NodeJS.ErrnoException {
  return (
    typeof error === "object" && error !== null && "code" in (error as Record<string, unknown>)
  );
}
