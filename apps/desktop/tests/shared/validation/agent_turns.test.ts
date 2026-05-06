import { describe, expect, it } from "vite-plus/test";

import {
  parseAgentTurnsPageQuery,
  parseAgentTurnsPageResult,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";
import { agentTurnAssistantItem, agentTurnToolCallItem } from "./helpers.js";

describe("parseAgentTurnsPageQuery", () => {
  it("accepts a transcript page query with an optional cursor", () => {
    expect(
      parseAgentTurnsPageQuery({
        limit: 25,
        before: {
          sequence: "44",
        },
      }),
    ).toEqual({
      limit: 25,
      before: {
        sequence: 44n,
      },
    });
  });
});

describe("parseAgentTurnsPageResult", () => {
  it("accepts committed assistant rows from the Rust contract", () => {
    expect(
      parseAgentTurnsPageResult({
        items: [agentTurnAssistantItem()],
        nextBefore: {
          sequence: "45",
        },
        latestCursor: {
          daemonInstanceId: "daemon-1",
          sessionId: "session-1",
          sequence: "52",
        },
      }),
    ).toEqual({
      items: [
        {
          kind: "assistant",
          cursor: {
            sequence: 45n,
          },
          sessionId: "session-1",
          runId: "run-1",
          turnId: "turn-1",
          startedAtMs: 100n,
          completedAtMs: 120n,
          text: "hello world",
        },
      ],
      nextBefore: {
        sequence: 45n,
      },
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 52n,
      },
    });
  });

  it("accepts committed tool call rows with input from the Rust contract", () => {
    expect(
      parseAgentTurnsPageResult({
        items: [agentTurnToolCallItem()],
        nextBefore: null,
        latestCursor: null,
      }),
    ).toEqual({
      items: [
        {
          kind: "toolCall",
          cursor: {
            sequence: 46n,
          },
          sessionId: "session-1",
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          toolName: "shell",
          input: '{"cmd":"echo hi"}',
          output: "echo hi",
          outcome: "completed",
          startedAtMs: 110n,
          completedAtMs: 120n,
        },
      ],
      nextBefore: null,
      latestCursor: null,
    });
  });

  it("rejects committed rows with malformed cursors", () => {
    expect(() =>
      parseAgentTurnsPageResult({
        items: [
          {
            ...agentTurnAssistantItem(),
            cursor: {
              sequence: "45",
              extra: true,
            },
          },
        ],
      }),
    ).toThrow(ProtocolValidationError);
  });
});
