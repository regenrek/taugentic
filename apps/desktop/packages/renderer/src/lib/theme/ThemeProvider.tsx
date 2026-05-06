import type { ReactNode } from "react";

import { useMountEffect } from "../react/use-mount-effect";
import { themeModeStore } from "./theme-mode-store";

export interface ThemeProviderProps {
  children: ReactNode;
}

/**
 * Applies the current theme mode to `document.documentElement` via the
 * `data-theme` attribute and keeps it in sync with the theme-mode store.
 *
 * `data-theme` is always either `"dark"` or `"light"`; `tokens.css` scopes
 * every design token to those two selectors.
 */
export function ThemeProvider({ children }: ThemeProviderProps) {
  useMountEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    const root = document.documentElement;
    root.dataset.theme = themeModeStore.getSnapshot().context.mode;

    const subscription = themeModeStore.subscribe((snapshot) => {
      root.dataset.theme = snapshot.context.mode;
    });

    return () => {
      subscription.unsubscribe();
    };
  });

  return <>{children}</>;
}
