import { describe, expect, it } from "vite-plus/test";

import { queryKeys } from "../../packages/renderer/src/lib/queries/keys.js";
import { useSessionNativeRunsQuery } from "../../packages/renderer/src/lib/queries/session-queries.js";

describe("session query keys", () => {
  it("keeps generic activity and run-only activity in separate cache keys", () => {
    expect(queryKeys.sessionActivity("session-1", { limit: 12 })).not.toEqual(
      queryKeys.sessionActivity("session-1", {
        kinds: ["run"],
        limit: 12,
      }),
    );
  });

  it("normalizes native run list filters for stable cache keys", () => {
    expect(
      queryKeys.sessionNativeRuns("session-1", {
        filter: {
          harness: ["native", "acp"],
          status: ["running", "queued"],
          parentRunId: "run-parent",
        },
        limit: 25,
      }),
    ).toEqual(
      queryKeys.sessionNativeRuns("session-1", {
        filter: {
          harness: ["acp", "native"],
          status: ["queued", "running"],
          parentRunId: "run-parent",
        },
        limit: 25,
      }),
    );
    expect(useSessionNativeRunsQuery).toBeTypeOf("function");
  });
});
