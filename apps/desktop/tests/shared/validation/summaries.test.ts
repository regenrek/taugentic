import { describe, expect, it } from "vite-plus/test";

import {
  parseApprovalRequestList,
  parseArtifactSummaryList,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";

describe("parseArtifactSummaryList", () => {
  it("accepts Rust-owned artifact summaries", () => {
    expect(
      parseArtifactSummaryList([
        {
          id: "artifact-7",
          runId: "run-1",
          kind: "Patch",
          storagePath: "artifacts/run-1/patch.diff",
        },
      ]),
    ).toEqual([
      {
        id: "artifact-7",
        runId: "run-1",
        kind: "Patch",
        storagePath: "artifacts/run-1/patch.diff",
      },
    ]);
  });

  it("rejects drifted artifact summaries", () => {
    expect(() =>
      parseArtifactSummaryList([
        {
          id: "artifact-7",
          kind: "patch",
          storagePath: "artifacts/run-1/patch.diff",
        },
      ]),
    ).toThrow(ProtocolValidationError);
  });
});

describe("parseApprovalRequestList", () => {
  it("accepts Rust-owned approval requests", () => {
    expect(
      parseApprovalRequestList([
        {
          id: "approval-7",
          runId: "run-1",
          scope: "processExec",
          requestedAtMs: "1000",
          expiresAtMs: "61000",
          target: { kind: "processExec", command: "pnpm test" },
          reason: "need shell",
        },
      ]),
    ).toEqual([
      {
        id: "approval-7",
        runId: "run-1",
        scope: "processExec",
        requestedAtMs: "1000",
        expiresAtMs: "61000",
        target: { kind: "processExec", command: "pnpm test" },
        reason: "need shell",
      },
    ]);
  });

  it("rejects drifted approval requests", () => {
    expect(() =>
      parseApprovalRequestList([
        {
          id: "approval-7",
          scope: "processExec",
          reason: "need shell",
        },
      ]),
    ).toThrow(ProtocolValidationError);
  });
});
