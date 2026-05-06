import type { SessionId } from "@taugentic/desktop-shared";
import { createStore } from "@xstate/store";

import type { AppRouteId } from "@/app/router";

interface WorkspaceShellContext {
  currentRouteId: AppRouteId;
  currentSessionId: SessionId | null;
}

const initialWorkspaceShellContext: WorkspaceShellContext = {
  currentRouteId: "workspace",
  currentSessionId: null,
};

export const workspaceShellStore = createStore({
  context: initialWorkspaceShellContext,
  on: {
    runStarted: (context) => ({
      ...context,
    }),
    sessionChanged: (
      context,
      event: {
        sessionId: SessionId | null;
      },
    ) => ({
      ...context,
      currentRouteId: "workspace" as const,
      currentSessionId: event.sessionId,
    }),
    shellReset: () => ({
      ...initialWorkspaceShellContext,
    }),
  },
});

let workspaceShellBootstrapped = false;

export function bootstrapWorkspaceShell(): void {
  if (workspaceShellBootstrapped) {
    return;
  }
  workspaceShellBootstrapped = true;
}

export function resetWorkspaceShellForTests(): void {
  workspaceShellBootstrapped = false;
  workspaceShellStore.trigger.shellReset();
}

export function getCurrentWorkspaceSessionId(): SessionId | null {
  return workspaceShellStore.getSnapshot().context.currentSessionId;
}
