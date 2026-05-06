import { describe, expect, it, vi } from "vite-plus/test";

import type { ArtifactSummary } from "../../packages/shared/src/contracts.js";
import {
  DEFAULT_READ_ARTIFACT_MAX_BYTES,
  MAX_READ_ARTIFACT_MAX_BYTES,
} from "../../packages/shared/src/ipc.js";
import {
  handleReadArtifactContent,
  handleSaveArtifactAs,
  performReadArtifactContent,
  performSaveArtifactAs,
  resolveArtifactStoragePath,
  type DesktopArtifactFileStat,
  type DesktopArtifactIo,
  type DesktopArtifactReadResult,
  type DesktopArtifactSaveDialogResult,
} from "../../packages/main/src/desktop-artifact-service.js";

const IS_WINDOWS = process.platform === "win32";
const POSIX_ROOT = "/tmp/taugentic/artifacts";
const WIN_ROOT = "C:\\taugentic\\artifacts";
const ARTIFACT_ROOT = IS_WINDOWS ? WIN_ROOT : POSIX_ROOT;
const ABSOLUTE_IN_ROOT = IS_WINDOWS
  ? "C:\\taugentic\\artifacts\\run-1\\patch.diff"
  : "/tmp/taugentic/artifacts/run-1/patch.diff";

function patchSummary(overrides: Partial<ArtifactSummary> = {}): ArtifactSummary {
  return {
    id: overrides.id ?? "artifact-1",
    runId: overrides.runId ?? "run-1",
    kind: overrides.kind ?? "Patch",
    storagePath: overrides.storagePath ?? "run-1/patch.diff",
  };
}

function statFileMock(stat: DesktopArtifactFileStat): DesktopArtifactIo["statFile"] {
  return vi.fn<DesktopArtifactIo["statFile"]>(async () => stat);
}

function readFileMock(result: DesktopArtifactReadResult): DesktopArtifactIo["readFile"] {
  return vi.fn<DesktopArtifactIo["readFile"]>(async () => result);
}

function showSaveDialogMock(
  result: DesktopArtifactSaveDialogResult,
): DesktopArtifactIo["showSaveDialog"] {
  return vi.fn<DesktopArtifactIo["showSaveDialog"]>(async () => result);
}

function makeIo(overrides: Partial<DesktopArtifactIo> = {}): DesktopArtifactIo {
  return {
    statFile: overrides.statFile ?? statFileMock({ kind: "file", size: 16 }),
    readFile:
      overrides.readFile ??
      readFileMock({
        status: "ok",
        bytes: Buffer.from("hello-artifact-v1"),
        truncated: false,
      }),
    copyFile: overrides.copyFile ?? vi.fn(async () => undefined),
    showSaveDialog:
      overrides.showSaveDialog ??
      showSaveDialogMock({ status: "saved", path: "/tmp/pick/patch.diff" }),
    realpath: overrides.realpath,
  };
}

describe("resolveArtifactStoragePath", () => {
  it("resolves a relative storagePath against the main-owned artifact root", () => {
    const result = resolveArtifactStoragePath("run-1/patch.diff", ARTIFACT_ROOT);
    expect(result.kind).toBe("resolved");
    if (result.kind === "resolved") {
      expect(result.absolutePath.startsWith(ARTIFACT_ROOT)).toBe(true);
      expect(result.absolutePath.endsWith("patch.diff")).toBe(true);
    }
  });

  it("accepts the nested repo fixture shape 'artifacts/run-1/patch.diff'", () => {
    const result = resolveArtifactStoragePath("artifacts/run-1/patch.diff", ARTIFACT_ROOT);
    expect(result.kind).toBe("resolved");
  });

  it("accepts an absolute path that stays inside the artifact root", () => {
    const result = resolveArtifactStoragePath(ABSOLUTE_IN_ROOT, ARTIFACT_ROOT);
    expect(result.kind).toBe("resolved");
  });

  it("rejects an absolute path that escapes the artifact root", () => {
    const escape = IS_WINDOWS ? "C:\\etc\\passwd" : "/etc/passwd";
    const result = resolveArtifactStoragePath(escape, ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
    if (result.kind === "invalid") {
      expect(result.reason).toMatch(/escapes artifact root/);
    }
  });

  it("rejects '..' path traversal segments", () => {
    const result = resolveArtifactStoragePath("../escape.diff", ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
    if (result.kind === "invalid") {
      expect(result.reason).toMatch(/'\.\.'/);
    }
  });

  it("rejects backslash '..' path traversal on win32-style input", () => {
    const result = resolveArtifactStoragePath("run-1\\..\\..\\escape.diff", ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
  });

  it("rejects null bytes in storagePath", () => {
    const result = resolveArtifactStoragePath("run-1/evil\0patch.diff", ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
  });

  it("rejects empty storagePath", () => {
    const result = resolveArtifactStoragePath("", ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
  });

  it("rejects non-string storagePath", () => {
    const result = resolveArtifactStoragePath(7 as unknown as string, ARTIFACT_ROOT);
    expect(result.kind).toBe("invalid");
  });

  it("rejects relative storagePath when the main artifact root is unknown", () => {
    const result = resolveArtifactStoragePath("run-1/patch.diff", null);
    expect(result.kind).toBe("invalid");
    if (result.kind === "invalid") {
      expect(result.reason).toMatch(/artifact root/);
    }
  });

  it("accepts absolute storagePath when no artifact root is configured (bootstrap fallback)", () => {
    const result = resolveArtifactStoragePath(ABSOLUTE_IN_ROOT, null);
    expect(result.kind).toBe("resolved");
  });
});

describe("performReadArtifactContent", () => {
  it("reads a relative storagePath resolved against the artifact root", async () => {
    const bytes = Buffer.from("diff --git a/x b/x\n+hello\n");
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: bytes.byteLength }),
      readFile: readFileMock({ status: "ok", bytes, truncated: false }),
    });

    const result = await performReadArtifactContent(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      DEFAULT_READ_ARTIFACT_MAX_BYTES,
      io,
      ARTIFACT_ROOT,
    );

    expect(result.status).toBe("inline");
    if (result.status === "inline") {
      expect(result.storagePath).toBe("run-1/patch.diff");
      expect(result.content).toBe(bytes.toString("utf-8"));
    }
  });

  it("reads an absolute storagePath that lives inside the artifact root", async () => {
    const bytes = Buffer.from("absolute-content");
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: bytes.byteLength }),
      readFile: readFileMock({ status: "ok", bytes, truncated: false }),
    });
    const result = await performReadArtifactContent(
      patchSummary({ storagePath: ABSOLUTE_IN_ROOT }),
      DEFAULT_READ_ARTIFACT_MAX_BYTES,
      io,
      ARTIFACT_ROOT,
    );
    expect(result.status).toBe("inline");
  });

  it("returns tooLarge without reading when size exceeds the cap", async () => {
    const readFile = vi.fn<DesktopArtifactIo["readFile"]>();
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: 1024 }),
      readFile,
    });
    const result = await performReadArtifactContent(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      512,
      io,
      ARTIFACT_ROOT,
    );

    expect(result.status).toBe("tooLarge");
    if (result.status === "tooLarge") {
      expect(result.totalBytes).toBe(1024);
      expect(result.limitBytes).toBe(512);
    }
    expect(readFile).not.toHaveBeenCalled();
  });

  it("surfaces fileNotFound for missing / directory / symlink stat kinds", async () => {
    const notFound = await performReadArtifactContent(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      1024,
      makeIo({ statFile: statFileMock({ kind: "notFound", size: 0 }) }),
      ARTIFACT_ROOT,
    );
    expect(notFound).toEqual({ status: "missing", reason: "fileNotFound" });

    const dir = await performReadArtifactContent(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      1024,
      makeIo({ statFile: statFileMock({ kind: "directory", size: 0 }) }),
      ARTIFACT_ROOT,
    );
    expect(dir).toEqual({ status: "missing", reason: "fileNotFound" });

    const symlink = await performReadArtifactContent(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      1024,
      makeIo({ statFile: statFileMock({ kind: "symlink", size: 0 }) }),
      ARTIFACT_ROOT,
    );
    expect(symlink).toEqual({ status: "missing", reason: "fileNotFound" });
  });

  it("rejects absolute storagePath that escapes the artifact root", async () => {
    const escape = IS_WINDOWS ? "C:\\etc\\passwd" : "/etc/passwd";
    await expect(
      performReadArtifactContent(
        patchSummary({ storagePath: escape }),
        1024,
        makeIo(),
        ARTIFACT_ROOT,
      ),
    ).rejects.toThrow(/escapes artifact root/);
  });

  it("catches symlink-based escape via realpath containment", async () => {
    const outsideRealpath = IS_WINDOWS ? "C:\\etc\\passwd" : "/etc/passwd";
    const io = makeIo({
      realpath: vi.fn(async (path: string) =>
        path === ARTIFACT_ROOT ? ARTIFACT_ROOT : outsideRealpath,
      ),
    });
    await expect(
      performReadArtifactContent(
        patchSummary({ storagePath: "run-1/patch.diff" }),
        1024,
        io,
        ARTIFACT_ROOT,
      ),
    ).rejects.toThrow(/escapes artifact root/);
  });
});

describe("performSaveArtifactAs", () => {
  it("saves a relative storagePath artifact after user picks a destination", async () => {
    const copyFile = vi.fn(async () => undefined);
    const showSaveDialog = showSaveDialogMock({
      status: "saved",
      path: "/tmp/out/patch.diff",
    });
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: 42 }),
      copyFile,
      showSaveDialog,
    });

    const result = await performSaveArtifactAs(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      "patch.diff",
      null,
      io,
      ARTIFACT_ROOT,
    );

    expect(result).toEqual({
      status: "saved",
      savedPath: "/tmp/out/patch.diff",
      bytesCopied: 42,
    });
    expect(copyFile).toHaveBeenCalledTimes(1);
  });

  it("returns cancelled without copying on dialog dismissal", async () => {
    const copyFile = vi.fn(async () => undefined);
    const io = makeIo({
      copyFile,
      showSaveDialog: showSaveDialogMock({ status: "cancelled" }),
    });

    const result = await performSaveArtifactAs(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      "patch.diff",
      null,
      io,
      ARTIFACT_ROOT,
    );
    expect(result).toEqual({ status: "cancelled" });
    expect(copyFile).not.toHaveBeenCalled();
  });

  it("returns missing when the daemon-owned file is gone", async () => {
    const io = makeIo({
      statFile: statFileMock({ kind: "notFound", size: 0 }),
    });
    const result = await performSaveArtifactAs(
      patchSummary({ storagePath: "run-1/patch.diff" }),
      undefined,
      null,
      io,
      ARTIFACT_ROOT,
    );
    expect(result).toEqual({ status: "missing", reason: "fileNotFound" });
  });
});

describe("handleReadArtifactContent", () => {
  it("returns artifactNotFound when the daemon has no artifact summary", async () => {
    const result = await handleReadArtifactContent(
      { sessionId: "session-1", artifactId: "artifact-missing" },
      {
        io: makeIo(),
        artifactRoot: ARTIFACT_ROOT,
        getArtifact: vi.fn(async () => null),
      },
    );
    expect(result).toEqual({ status: "missing", reason: "artifactNotFound" });
  });

  it("resolves relative paths through the deps artifact root", async () => {
    const readFile = vi.fn<DesktopArtifactIo["readFile"]>(async () => ({
      status: "ok" as const,
      bytes: Buffer.from("hi"),
      truncated: false,
    }));
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: 2 }),
      readFile,
    });
    await handleReadArtifactContent(
      { sessionId: "session-1", artifactId: "artifact-1" },
      {
        io,
        artifactRoot: ARTIFACT_ROOT,
        getArtifact: vi.fn(async () => patchSummary({ storagePath: "run-1/patch.diff" })),
      },
    );
    const callArg = readFile.mock.calls[0]![0];
    expect(callArg.startsWith(ARTIFACT_ROOT)).toBe(true);
  });

  it("applies the default cap when maxBytes is omitted", async () => {
    const readFile = vi.fn<DesktopArtifactIo["readFile"]>(async () => ({
      status: "ok" as const,
      bytes: Buffer.alloc(10),
      truncated: false,
    }));
    const io = makeIo({
      statFile: statFileMock({ kind: "file", size: 10 }),
      readFile,
    });
    await handleReadArtifactContent(
      { sessionId: "session-1", artifactId: "artifact-1" },
      {
        io,
        artifactRoot: ARTIFACT_ROOT,
        getArtifact: vi.fn(async () => patchSummary({ storagePath: "run-1/patch.diff" })),
      },
    );
    const callArg = readFile.mock.calls[0]![1];
    expect(callArg).toBe(DEFAULT_READ_ARTIFACT_MAX_BYTES);
  });

  it("rejects invalid query shapes via DTO parsing", async () => {
    await expect(
      handleReadArtifactContent(
        { sessionId: "", artifactId: "artifact-1" },
        {
          io: makeIo(),
          artifactRoot: ARTIFACT_ROOT,
          getArtifact: vi.fn(async () => patchSummary()),
        },
      ),
    ).rejects.toThrow(/SessionId/);
    await expect(
      handleReadArtifactContent(
        {
          sessionId: "session-1",
          artifactId: "artifact-1",
          maxBytes: MAX_READ_ARTIFACT_MAX_BYTES + 1,
        },
        {
          io: makeIo(),
          artifactRoot: ARTIFACT_ROOT,
          getArtifact: vi.fn(async () => patchSummary()),
        },
      ),
    ).rejects.toThrow(/maxBytes/);
  });
});

describe("handleSaveArtifactAs", () => {
  it("returns artifactNotFound when the daemon has no summary", async () => {
    const result = await handleSaveArtifactAs(
      { sessionId: "session-1", artifactId: "artifact-missing" },
      null,
      {
        io: makeIo(),
        artifactRoot: ARTIFACT_ROOT,
        getArtifact: vi.fn(async () => null),
      },
    );
    expect(result).toEqual({ status: "missing", reason: "artifactNotFound" });
  });

  it("rejects filenames that include path separators", async () => {
    await expect(
      handleSaveArtifactAs(
        {
          sessionId: "session-1",
          artifactId: "artifact-1",
          suggestedFilename: "../escape.diff",
        },
        null,
        {
          io: makeIo(),
          artifactRoot: ARTIFACT_ROOT,
          getArtifact: vi.fn(async () => patchSummary()),
        },
      ),
    ).rejects.toThrow(/path separators/);
  });
});
