import { describe, expect, it } from "vite-plus/test";

import { isDaemonCommand, parseDaemonProcessEntries } from "../../../../scripts/daemon-cleanup.mjs";

describe("parseDaemonProcessEntries", () => {
  it("returns only real ta-daemon executable processes", () => {
    const processTable = [
      "101 /Users/test/projects/taugentic/target/debug/ta-daemon",
      "102 target/debug/ta-daemon __runtime-control-bootstrap start",
      "103 /bin/zsh -c lsof -nU '/Users/test/Library/Application Support/taugentic/runtime/ta-daemon.sock'",
      "104 cargo run --package ta-orchestrator --bin ta-daemon",
      "105 /Users/test/projects/taugentic/target/debug/other-daemon",
    ].join("\n");

    expect(parseDaemonProcessEntries(processTable)).toEqual([
      { pid: 101, command: "/Users/test/projects/taugentic/target/debug/ta-daemon" },
      { pid: 102, command: "target/debug/ta-daemon __runtime-control-bootstrap start" },
    ]);
  });
});

describe("isDaemonCommand", () => {
  it("does not classify ta-daemon argument text as a daemon process", () => {
    expect(isDaemonCommand("/bin/zsh -c pgrep -af ta-daemon")).toBe(false);
    expect(isDaemonCommand("cargo run --package ta-orchestrator --bin ta-daemon")).toBe(false);
    expect(isDaemonCommand("/tmp/target/debug/ta-daemon")).toBe(true);
  });
});
