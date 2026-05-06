import { describe, expect, it } from "vite-plus/test";

import {
  parseActivityPageQuery,
  parseActivityPageResult,
  parseClientCredential,
  parseSessionId,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";
import { agentStreamActivityItem } from "./helpers.js";

describe("parseActivityPageResult", () => {
  it("accepts artifact activity items from the Rust contract", () => {
    expect(
      parseActivityPageResult({
        items: [
          {
            cursor: {
              sequence: "7",
            },
            occurredAtMs: "90",
            event: {
              artifact: {
                artifact: {
                  id: "artifact-7",
                  runId: "run-1",
                  kind: "Patch",
                  storagePath: "artifacts/run-1/patch.diff",
                },
              },
            },
          },
        ],
        nextBefore: null,
        latestActivityCursor: {
          sequence: "7",
        },
      }),
    ).toEqual({
      items: [
        {
          cursor: {
            sequence: 7n,
          },
          occurredAtMs: 90n,
          event: {
            artifact: {
              artifact: {
                id: "artifact-7",
                runId: "run-1",
                kind: "Patch",
                storagePath: "artifacts/run-1/patch.diff",
              },
            },
          },
        },
      ],
      nextBefore: null,
      latestActivityCursor: {
        sequence: 7n,
      },
    });
  });

  it("rejects cursors that carry extra fields", () => {
    expect(() =>
      parseActivityPageResult({
        items: [
          {
            cursor: {
              sequence: "1",
              daemonInstanceId: "daemon-1",
            },
            occurredAtMs: "9",
            event: {
              run: {
                runId: "run-1",
                status: "queued",
                detail: "queued",
              },
            },
          },
        ],
        nextBefore: null,
        latestActivityCursor: null,
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("accepts resolved approval activity items without public actor or commentary", () => {
    expect(
      parseActivityPageResult({
        items: [
          {
            cursor: {
              sequence: "8",
            },
            occurredAtMs: "91",
            event: {
              approval: {
                phase: "resolved",
                resolution: {
                  approvalId: "approval-1",
                  runId: "run-1",
                  decision: "approved",
                  reason: "user",
                },
              },
            },
          },
        ],
        nextBefore: null,
        latestActivityCursor: {
          sequence: "8",
        },
      }),
    ).toEqual({
      items: [
        {
          cursor: {
            sequence: 8n,
          },
          occurredAtMs: 91n,
          event: {
            approval: {
              phase: "resolved",
              resolution: {
                approvalId: "approval-1",
                runId: "run-1",
                decision: "approved",
                reason: "user",
              },
            },
          },
        },
      ],
      nextBefore: null,
      latestActivityCursor: {
        sequence: 8n,
      },
    });
  });

  it("rejects resolved approval activity items that leak public actor or commentary", () => {
    expect(() =>
      parseActivityPageResult({
        items: [
          {
            cursor: {
              sequence: "8",
            },
            occurredAtMs: "91",
            event: {
              approval: {
                phase: "resolved",
                resolution: {
                  approvalId: "approval-1",
                  runId: "run-1",
                  decision: "approved",
                  reason: "user",
                  actor: {
                    principalId: "principal-1",
                  },
                  commentary: "looks safe",
                },
              },
            },
          },
        ],
        nextBefore: null,
        latestActivityCursor: {
          sequence: "8",
        },
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("accepts agent stream activity items from the Rust contract", () => {
    expect(
      parseActivityPageResult({
        items: [
          agentStreamActivityItem({
            kind: "toolCallStarted",
            toolName: "shell",
            input: '{"cmd":"echo hi"}',
          }),
        ],
        nextBefore: null,
        latestActivityCursor: {
          sequence: "44",
        },
      }),
    ).toEqual({
      items: [
        {
          cursor: {
            sequence: 44n,
          },
          occurredAtMs: 101n,
          event: {
            agentStream: {
              runId: "run-1",
              turnId: "turn-1",
              itemId: "item-1",
              fragmentSequence: 3,
              frame: {
                kind: "toolCallStarted",
                toolName: "shell",
                input: '{"cmd":"echo hi"}',
              },
            },
          },
        },
      ],
      nextBefore: null,
      latestActivityCursor: {
        sequence: 44n,
      },
    });
  });
});

describe("request-side desktop parsers", () => {
  it("accepts activity page queries and normalizes before cursors", () => {
    expect(
      parseActivityPageQuery({
        limit: 25,
        before: { sequence: "9" },
        kinds: ["run", "approval"],
      }),
    ).toEqual({
      limit: 25,
      before: { sequence: 9n },
      kinds: ["run", "approval"],
    });
  });

  it("rejects empty session identifiers even when the schema is only string-typed", () => {
    expect(() => parseSessionId("   ")).toThrow("SessionId must be a non-empty string");
  });

  it("accepts trimmed client credentials with the Rust-owned initialize invariant", () => {
    expect(parseClientCredential("  credential-1credential-1credential-1  ")).toBe(
      "credential-1credential-1credential-1",
    );
  });

  it("rejects short client credentials", () => {
    expect(() => parseClientCredential("short-credential")).toThrow(
      "clientCredential must be at least 32 non-whitespace ASCII characters",
    );
  });
});
