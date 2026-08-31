import { describe, expect, it } from "bun:test";

import {
  developmentTerminalDiagnostic,
  resolveDevelopmentTerminalPaths,
} from "../scripts/development-terminal.mjs";

const characterDevice = { isCharacterDevice: () => true };
const nonCharacterDevice = { isCharacterDevice: () => false };

describe("development terminal resolver", () => {
  it("accepts separate concrete stdout and stderr TTY character devices", async () => {
    const paths = await resolveDevelopmentTerminalPaths({
      stdoutDescriptor: 41,
      stderrDescriptor: 42,
      runTtyImpl: async (descriptor) =>
        descriptor === 41 ? "/dev/ttys001\n" : "/dev/ttys002\n",
      statImpl: async () => characterDevice,
    });

    expect(paths).toEqual({
      stdoutPath: "/dev/ttys001",
      stderrPath: "/dev/ttys002",
    });
  });

  it.each([
    ["a pipe target", "not a tty\n", characterDevice],
    ["a non-TTY character device", "/dev/console", characterDevice],
    ["a FIFO TTY-looking target", "/dev/ttys001", nonCharacterDevice],
  ])("rejects %s", async (_description, resolvedPath, stats) => {
    await expect(
      resolveDevelopmentTerminalPaths({
        runTtyImpl: async () => resolvedPath,
        statImpl: async () => stats,
      }),
    ).rejects.toThrow(developmentTerminalDiagnostic);
  });

  it("rejects a failed descriptor-bound tty invocation with the fixed diagnostic", async () => {
    await expect(
      resolveDevelopmentTerminalPaths({
        runTtyImpl: async () => {
          throw new Error("unavailable");
        },
      }),
    ).rejects.toThrow(developmentTerminalDiagnostic);
  });
});
