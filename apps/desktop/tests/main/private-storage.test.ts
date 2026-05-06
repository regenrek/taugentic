import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const fsPromises = vi.hoisted(() => ({
  chmod: vi.fn(async () => {}),
  mkdir: vi.fn(async () => undefined),
  rm: vi.fn(async () => undefined),
  writeFile: vi.fn(async () => undefined),
}));

vi.mock("node:fs/promises", () => fsPromises);

describe("private storage", () => {
  beforeEach(() => {
    fsPromises.chmod.mockReset();
    fsPromises.chmod.mockImplementation(async () => {});
    fsPromises.mkdir.mockReset();
    fsPromises.mkdir.mockImplementation(async () => undefined);
    fsPromises.rm.mockReset();
    fsPromises.rm.mockImplementation(async () => undefined);
    fsPromises.writeFile.mockReset();
    fsPromises.writeFile.mockImplementation(async () => undefined);
  });

  it("ignores chmod ENOENT when a concurrent delete wins after write", async () => {
    const { writePrivateStorageFile } = await import("../../packages/main/src/private-storage.js");
    fsPromises.chmod.mockRejectedValueOnce(
      Object.assign(new Error("missing after delete"), { code: "ENOENT" }),
    );

    await expect(
      writePrivateStorageFile("/tmp/taugentic", ["client", "session.authority"], "secret"),
    ).resolves.toBe("/tmp/taugentic/client/session.authority");
  });
});
