import { beforeEach, describe, expect, it } from "vite-plus/test";

import {
  createThemeModeStore,
  THEME_MODE_STORAGE_KEY,
  type ThemeMode,
  type ThemeModeStorage,
} from "../../packages/renderer/src/lib/theme/theme-mode-store.js";

function createLocalStorageMock(): ThemeModeStorage & { readonly data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem(key: string): string | null {
      return data.has(key) ? (data.get(key) as string) : null;
    },
    setItem(key: string, value: string): void {
      data.set(key, value);
    },
  };
}

describe("theme mode store", () => {
  let storage: ReturnType<typeof createLocalStorageMock>;

  beforeEach(() => {
    storage = createLocalStorageMock();
  });

  it("defaults to dark when no mode has been persisted", () => {
    const store = createThemeModeStore(storage);

    const mode: ThemeMode = store.getSnapshot().context.mode;

    expect(mode).toBe("dark");
  });

  it("applies modeSet to flip the mode to light", () => {
    const store = createThemeModeStore(storage);

    store.trigger.modeSet({ mode: "light" });

    expect(store.getSnapshot().context.mode).toBe("light");
    expect(storage.getItem(THEME_MODE_STORAGE_KEY)).toBe("light");
  });

  it("toggles from light back to dark", () => {
    const store = createThemeModeStore(storage);
    store.trigger.modeSet({ mode: "light" });

    store.trigger.modeToggled();

    expect(store.getSnapshot().context.mode).toBe("dark");
    expect(storage.getItem(THEME_MODE_STORAGE_KEY)).toBe("dark");
  });

  it("reads the persisted mode from storage on next store creation", () => {
    const firstStore = createThemeModeStore(storage);
    firstStore.trigger.modeSet({ mode: "light" });

    const secondStore = createThemeModeStore(storage);

    expect(secondStore.getSnapshot().context.mode).toBe("light");
  });

  it("ignores unknown persisted values and falls back to dark", () => {
    storage.setItem(THEME_MODE_STORAGE_KEY, "sepia");

    const store = createThemeModeStore(storage);

    expect(store.getSnapshot().context.mode).toBe("dark");
  });
});
