import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import type { AppShellProps } from "../../packages/renderer/src/app/shell.js";
import type { DaemonControlModel } from "../../packages/renderer/src/features/daemon/model.js";
import type { SessionId } from "../../packages/shared/generated/index.js";

type CreateElementFn = (
  component: string | ((...args: any[]) => unknown),
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

const captured = vi.hoisted(() => ({
  shellProps: null as AppShellProps | null,
}));

const selection = vi.hoisted(() => ({
  loadPersistedCurrentSessionId: vi.fn<() => SessionId | null>(() => null),
  persistCurrentSessionId: vi.fn<(sessionId: SessionId | null) => void>(() => {}),
}));

const daemonModel = vi.hoisted(
  (): DaemonControlModel => ({
    disableBackground: async () => {},
    enableBackground: async () => {},
    errorMessage: null,
    pendingAction: null,
    reconcile: async () => {},
    refresh: async () => {},
    start: async () => {},
    state: null,
    stop: async () => {},
  }),
);

vi.mock("../../packages/renderer/src/features/sessions/selection.js", () => selection);
vi.mock("../../packages/renderer/src/features/daemon/model.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/renderer/src/features/daemon/model.js")>();
  return {
    ...actual,
    useDaemonControlModel: vi.fn(() => daemonModel),
  };
});
vi.mock("../../packages/renderer/src/app/shell.js", () => ({
  AppShell(props: AppShellProps) {
    captured.shellProps = props;
    return createElement("div", null, "app-shell-probe");
  },
}));

import App from "../../packages/renderer/src/App.js";
import {
  resetWorkspaceShellForTests,
  workspaceShellStore,
} from "../../packages/renderer/src/features/workspace/state/store.js";
import { themeModeStore } from "../../packages/renderer/src/lib/theme/theme-mode-store.js";

describe("App bootstrap", () => {
  afterEach(() => {
    captured.shellProps = null;
    resetWorkspaceShellForTests();
    selection.loadPersistedCurrentSessionId.mockReset();
    selection.loadPersistedCurrentSessionId.mockReturnValue(null);
    selection.persistCurrentSessionId.mockClear();
  });

  it("does not hydrate a persisted selected session id into shell state before validation", () => {
    selection.loadPersistedCurrentSessionId.mockReturnValue("session-restore-42session-restore-42");

    const markup = renderToStaticMarkup(createElement(App));

    expect(markup).toContain("app-shell-probe");
    expect(captured.shellProps).not.toBeNull();
    expect(captured.shellProps?.currentSessionId).toBeNull();
    expect(captured.shellProps?.daemon).toBe(daemonModel);
    expect(workspaceShellStore.getSnapshot().context.currentRouteId).toBe("workspace");
    expect(selection.persistCurrentSessionId).not.toHaveBeenCalled();
  });

  it("boots the shell without an active session when no persisted selection exists", () => {
    const markup = renderToStaticMarkup(createElement(App));

    expect(markup).toContain("app-shell-probe");
    expect(captured.shellProps?.currentSessionId).toBeNull();
    expect(workspaceShellStore.getSnapshot().context.currentRouteId).toBe("workspace");
    expect(selection.persistCurrentSessionId).not.toHaveBeenCalled();
  });

  it("mounts the ThemeProvider with the canonical dark default mode", () => {
    renderToStaticMarkup(createElement(App));

    expect(themeModeStore.getSnapshot().context.mode).toBe("dark");
  });
});
