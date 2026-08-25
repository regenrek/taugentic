import type { ExecutionContext } from "../../../packages/shared/src/contracts.js";

export function makeExecutionContext(): ExecutionContext {
  const workspaceRoot = "/tmp/taugentic-workspace";

  return {
    workspaceId: "workspace-default",
    workspaceRoot,
    effectiveCwd: workspaceRoot,
    artifactRoot: "/tmp/taugentic-artifacts",
    workspaceScope: {
      kind: "local",
      root: workspaceRoot,
    },
    sandboxProfile: {
      readRoots: [workspaceRoot],
      writeRoots: [workspaceRoot, "/tmp/taugentic-artifacts"],
      deniedRoots: [],
      processExec: { kind: "allowAll" },
    },
    permissionPolicy: "workspaceWrite",
    networkPolicy: { kind: "open" },
    envPolicy: {
      kind: "allowlist",
      vars: ["PATH", "HOME", "TMPDIR"],
    },
  };
}
