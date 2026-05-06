import type { SessionId } from "@taugentic/desktop-shared";
import { useSelector } from "@xstate/store/react";

import { AppShell } from "./app/shell";
import { AppProviders } from "./app/providers";
import { useDaemonControlModel } from "./features/daemon/model";
import { usePersistedSessionBootstrap } from "./features/sessions/bootstrap";
import { persistCurrentSessionId } from "./features/sessions/selection";
import { ThemeProvider } from "./lib/theme/ThemeProvider";
import { bootstrapWorkspaceShell, workspaceShellStore } from "./features/workspace/state/store";

export default function App() {
  bootstrapWorkspaceShell();

  const currentSessionId = useSelector(
    workspaceShellStore,
    (snapshot) => snapshot.context.currentSessionId,
  );
  const daemon = useDaemonControlModel();

  function handleSessionChange(sessionId: SessionId | null) {
    workspaceShellStore.trigger.sessionChanged({ sessionId });
    persistCurrentSessionId(sessionId);
  }

  usePersistedSessionBootstrap(handleSessionChange);

  return (
    <AppProviders>
      <ThemeProvider>
        <AppShell
          currentSessionId={currentSessionId}
          daemon={daemon}
          onRunStarted={() => workspaceShellStore.trigger.runStarted()}
          onSessionChange={handleSessionChange}
        />
      </ThemeProvider>
    </AppProviders>
  );
}
