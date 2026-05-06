import {
  createDesktopWindowState,
  type DesktopWindowApi,
  type DesktopWindowState,
} from "@taugentic/desktop-shared";

const SERVER_WINDOW_STATE = createDesktopWindowState("macos");

let cachedDesktopWindowState = SERVER_WINDOW_STATE;
let cachedDesktopWindowApi: DesktopWindowApi | null = null;

function resolveDesktopWindowApi(): DesktopWindowApi | null {
  if (typeof window === "undefined" || window.desktopWindow == null) {
    return null;
  }
  return window.desktopWindow;
}

function cacheDesktopWindowSnapshot(api: DesktopWindowApi): DesktopWindowState {
  cachedDesktopWindowApi = api;
  cachedDesktopWindowState = api.getSnapshot();
  return cachedDesktopWindowState;
}

export function getDesktopWindowSnapshot(): DesktopWindowState {
  const api = resolveDesktopWindowApi();
  if (api == null) {
    return SERVER_WINDOW_STATE;
  }
  if (cachedDesktopWindowApi !== api) {
    return cacheDesktopWindowSnapshot(api);
  }
  return cachedDesktopWindowState;
}

export function getDesktopWindowServerSnapshot(): DesktopWindowState {
  return SERVER_WINDOW_STATE;
}

export function subscribeDesktopWindow(listener: () => void): () => void {
  const api = resolveDesktopWindowApi();
  if (api == null) {
    return () => undefined;
  }

  if (cachedDesktopWindowApi !== api) {
    cacheDesktopWindowSnapshot(api);
  }

  return api.subscribe(() => {
    cacheDesktopWindowSnapshot(api);
    listener();
  });
}

export function resetDesktopWindowSnapshotCacheForTests(): void {
  cachedDesktopWindowApi = null;
  cachedDesktopWindowState = SERVER_WINDOW_STATE;
}
