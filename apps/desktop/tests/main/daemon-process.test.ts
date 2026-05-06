import { describe, expect, it } from "vite-plus/test";

import { shouldTerminateManagedDaemonOnQuit } from "../../packages/main/src/daemon-process.js";

describe("shouldTerminateManagedDaemonOnQuit", () => {
  it("uses product mode rather than packaging heuristics", () => {
    expect(shouldTerminateManagedDaemonOnQuit("local")).toBe(true);
    expect(shouldTerminateManagedDaemonOnQuit("background")).toBe(false);
  });
});
