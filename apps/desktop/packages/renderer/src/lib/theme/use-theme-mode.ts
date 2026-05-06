import { useSelector } from "@xstate/store/react";

import {
  selectThemeMode,
  setThemeMode,
  themeModeStore,
  toggleThemeMode,
  type ThemeMode,
} from "./theme-mode-store";

export interface UseThemeModeResult {
  mode: ThemeMode;
  setMode: (mode: ThemeMode) => void;
  toggle: () => void;
}

export function useThemeMode(): UseThemeModeResult {
  const mode = useSelector(themeModeStore, selectThemeMode);
  return {
    mode,
    setMode: setThemeMode,
    toggle: toggleThemeMode,
  };
}
