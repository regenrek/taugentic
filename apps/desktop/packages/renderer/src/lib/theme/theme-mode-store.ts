import { createStore } from "@xstate/store";

/**
 * Canonical theme-mode store.
 *
 * Holds the active UI theme mode ("dark" by default, "light" alternative).
 * The store is the SSOT consumed by `<ThemeProvider>` and `useThemeMode`.
 * Persistence key must stay stable; do not rename without a migration.
 */

export type ThemeMode = "dark" | "light";

export const THEME_MODE_STORAGE_KEY = "taugentic.themeMode";
const DEFAULT_THEME_MODE: ThemeMode = "dark";

export interface ThemeModeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface ThemeModeContext {
  mode: ThemeMode;
}

export type ThemeModeSnapshot = {
  context: ThemeModeContext;
};

export type ThemeModeStore = ReturnType<typeof createThemeModeStore>;

function defaultThemeModeStorage(): ThemeModeStorage | null {
  return typeof window === "undefined" ? null : window.localStorage;
}

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "dark" || value === "light";
}

function loadPersistedThemeMode(storage: ThemeModeStorage | null): ThemeMode {
  if (storage == null) {
    return DEFAULT_THEME_MODE;
  }
  const raw = storage.getItem(THEME_MODE_STORAGE_KEY);
  return isThemeMode(raw) ? raw : DEFAULT_THEME_MODE;
}

function persistThemeMode(storage: ThemeModeStorage | null, mode: ThemeMode): void {
  if (storage == null) {
    return;
  }
  storage.setItem(THEME_MODE_STORAGE_KEY, mode);
}

export function selectThemeMode(snapshot: ThemeModeSnapshot): ThemeMode {
  return snapshot.context.mode;
}

export function createThemeModeStore(storage: ThemeModeStorage | null = defaultThemeModeStorage()) {
  const store = createStore({
    context: { mode: loadPersistedThemeMode(storage) } as ThemeModeContext,
    on: {
      modeSet: (context, event: { mode: ThemeMode }) => ({
        ...context,
        mode: event.mode,
      }),
      modeToggled: (context) => ({
        ...context,
        mode: context.mode === "dark" ? ("light" as const) : ("dark" as const),
      }),
    },
  });

  store.subscribe((snapshot) => {
    persistThemeMode(storage, snapshot.context.mode);
  });

  return store;
}

export const themeModeStore: ThemeModeStore = createThemeModeStore();

export function setThemeMode(mode: ThemeMode, store: ThemeModeStore = themeModeStore): void {
  store.trigger.modeSet({ mode });
}

export function toggleThemeMode(store: ThemeModeStore = themeModeStore): void {
  store.trigger.modeToggled();
}
