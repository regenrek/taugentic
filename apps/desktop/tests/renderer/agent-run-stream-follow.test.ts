import { describe, expect, it } from "vite-plus/test";

import {
  AGENT_RUN_STREAM_NEAR_BOTTOM_PX,
  computeFollowLiveTail,
} from "../../packages/renderer/src/features/agent-visualization/AgentRunStream.js";

describe("computeFollowLiveTail", () => {
  it("returns true when the viewport sits at the bottom", () => {
    expect(
      computeFollowLiveTail({
        scrollTop: 180,
        scrollHeight: 200,
        clientHeight: 20,
      }),
    ).toBe(true);
  });

  it("returns true within the default near-bottom threshold", () => {
    expect(
      computeFollowLiveTail({
        scrollTop: 200 - 20 - AGENT_RUN_STREAM_NEAR_BOTTOM_PX,
        scrollHeight: 200,
        clientHeight: 20,
      }),
    ).toBe(true);
  });

  it("returns false once the user scrolls above the threshold", () => {
    expect(
      computeFollowLiveTail({
        scrollTop: 200 - 20 - AGENT_RUN_STREAM_NEAR_BOTTOM_PX - 1,
        scrollHeight: 200,
        clientHeight: 20,
      }),
    ).toBe(false);
  });

  it("respects a custom near-bottom threshold", () => {
    expect(
      computeFollowLiveTail({
        scrollTop: 0,
        scrollHeight: 100,
        clientHeight: 50,
        nearBottomPx: 60,
      }),
    ).toBe(true);
    expect(
      computeFollowLiveTail({
        scrollTop: 0,
        scrollHeight: 100,
        clientHeight: 50,
        nearBottomPx: 10,
      }),
    ).toBe(false);
  });

  it("handles degenerate short-content case where there is nothing to scroll", () => {
    expect(
      computeFollowLiveTail({
        scrollTop: 0,
        scrollHeight: 10,
        clientHeight: 50,
      }),
    ).toBe(true);
  });
});
