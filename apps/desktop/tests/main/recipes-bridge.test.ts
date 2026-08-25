import { describe, expect, it, vi } from "vite-plus/test";

import { DaemonSessionRequestClient } from "../../packages/main/src/daemon-session-request-client.js";

type TestableRequestClient = {
  connection: {
    request: (
      method: string,
      params: Record<string, unknown>,
      parseResult: (value: unknown) => unknown,
    ) => Promise<unknown>;
  };
  ensureConnected: () => Promise<void>;
  listRecipes: DaemonSessionRequestClient["listRecipes"];
};

describe("recipes bridge", () => {
  it("passes daemon recipes through with all built-in fields", async () => {
    const daemonResult = {
      recipes: [
        makeRecipe("debug-agent", "Debug Agent", "debug"),
        makeRecipe("patch-agent", "Patch Agent", "patch"),
        makeRecipe("review-agent", "Review Agent", "review"),
        makeRecipe("test-agent", "Test Agent", "test"),
        makeRecipe("plan-agent", "Plan Agent", "plan"),
      ],
    };
    const session = new DaemonSessionRequestClient() as unknown as TestableRequestClient;

    session.ensureConnected = vi.fn(async () => {});
    const request = vi.fn(
      async (
        _method: string,
        _params: Record<string, unknown>,
        parseResult: (value: unknown) => unknown,
      ) => parseResult(daemonResult),
    );
    session.connection.request = request;

    await expect(session.listRecipes()).resolves.toEqual(daemonResult);
    expect(request).toHaveBeenCalledWith("daemon.recipes.list", {}, expect.any(Function));
  });
});

function makeRecipe(id: string, name: string, contract: string) {
  return {
    id,
    name,
    description: `${name} description`,
    contract,
    promptTemplate: `${name} prompt`,
    defaultModel: null,
  };
}
