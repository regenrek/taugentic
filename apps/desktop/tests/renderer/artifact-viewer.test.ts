import { describe, expect, it } from "vite-plus/test";

import type { ArtifactSummary } from "../../packages/shared/generated/index.js";
import {
  ARTIFACT_VIEWER_PATCH_KINDS,
  formatBytes,
  patchLineKind,
} from "../../packages/renderer/src/features/session-detail/ArtifactViewer.js";
import { classifySaveArtifactResult } from "../../packages/renderer/src/features/session-detail/ArtifactsSection.js";
import { defaultArtifactFilename } from "../../packages/renderer/src/features/session-detail/useArtifactContent.js";

describe("patchLineKind", () => {
  it("detects addition, deletion, hunk, and header lines", () => {
    expect(patchLineKind("+added")).toBe("addition");
    expect(patchLineKind("-removed")).toBe("deletion");
    expect(patchLineKind("@@ -1,2 +3,4 @@")).toBe("hunk");
    expect(patchLineKind("--- a/file")).toBe("header");
    expect(patchLineKind("+++ b/file")).toBe("header");
    expect(patchLineKind("diff --git a/x b/x")).toBe("header");
  });

  it("treats anything else as context", () => {
    expect(patchLineKind("unchanged")).toBe("context");
    expect(patchLineKind("")).toBe("context");
    expect(patchLineKind(" leading space")).toBe("context");
  });

  it("keeps every reachable patch classification explicit", () => {
    expect(ARTIFACT_VIEWER_PATCH_KINDS).toEqual([
      "addition",
      "deletion",
      "hunk",
      "header",
      "context",
    ]);
  });
});

describe("formatBytes", () => {
  it("formats small values as bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("formats KiB values with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KiB");
    expect(formatBytes(1536)).toBe("1.5 KiB");
  });

  it("formats MiB values with one decimal", () => {
    expect(formatBytes(2 * 1024 * 1024)).toBe("2.0 MiB");
    expect(formatBytes(1024 * 1024 * 15.25)).toBe("15.3 MiB");
  });
});

describe("defaultArtifactFilename", () => {
  function summary(overrides: Partial<ArtifactSummary>): ArtifactSummary {
    return {
      id: overrides.id ?? "artifact-1",
      runId: overrides.runId ?? "run-1",
      kind: overrides.kind ?? "Patch",
      storagePath: overrides.storagePath ?? "/tmp/artifacts/patch.diff",
    };
  }

  it("picks a reasonable extension per kind", () => {
    expect(defaultArtifactFilename(summary({ kind: "Patch" }))).toBe("artifact-1.diff");
    expect(defaultArtifactFilename(summary({ kind: "CommandLog" }))).toBe("artifact-1.log");
    expect(defaultArtifactFilename(summary({ kind: "Transcript" }))).toBe("artifact-1.md");
    expect(defaultArtifactFilename(summary({ kind: "FileSnapshot" }))).toBe("artifact-1.txt");
  });
});

describe("classifySaveArtifactResult", () => {
  it("saved → clears errors and does not invalidate the artifact list", () => {
    expect(
      classifySaveArtifactResult({
        status: "saved",
        savedPath: "/tmp/out/patch.diff",
        bytesCopied: 42,
      }),
    ).toEqual({
      errorMessage: null,
      invalidateArtifactList: false,
    });
  });

  it("cancelled → stays silent and does not invalidate the artifact list", () => {
    expect(classifySaveArtifactResult({ status: "cancelled" })).toEqual({
      errorMessage: null,
      invalidateArtifactList: false,
    });
  });

  it("missing.artifactNotFound → shows shared missing copy and invalidates the artifact list", () => {
    expect(classifySaveArtifactResult({ status: "missing", reason: "artifactNotFound" })).toEqual({
      errorMessage: "artifact no longer exists",
      invalidateArtifactList: true,
    });
  });

  it("missing.fileNotFound → shows shared missing copy and invalidates the artifact list", () => {
    expect(classifySaveArtifactResult({ status: "missing", reason: "fileNotFound" })).toEqual({
      errorMessage: "artifact file is missing on disk",
      invalidateArtifactList: true,
    });
  });
});
