import { Minus, Square, X } from "lucide-react";
import { useSyncExternalStore, type CSSProperties, type ReactNode } from "react";

import {
  rendererOwnsWindowControls,
  WINDOW_CHROME_NATIVE_INSET_PX,
  type DesktopWindowState,
} from "@taugentic/desktop-shared";

import { cn } from "@/lib/ui/cn";
import {
  getDesktopWindowServerSnapshot,
  getDesktopWindowSnapshot,
  subscribeDesktopWindow,
} from "./state";

const DRAG_REGION_STYLE = {
  WebkitAppRegion: "drag",
} as CSSProperties;

const NO_DRAG_REGION_STYLE = {
  WebkitAppRegion: "no-drag",
} as CSSProperties;

export interface DesktopWindowChromeProps {
  readonly children?: ReactNode;
}

/**
 * Terminal-style drag region that plays nicely with the native window chrome.
 *
 * Cross-OS window chrome rules:
 * - macOS: the OS renders the traffic lights inset via
 *   `titleBarStyle: "hiddenInset"`. We never draw our own — instead we
 *   reserve ~78px of leading padding so our shell content does not collide
 *   with them.
 * - Windows: Window Controls Overlay paints the native min/max/close at the
 *   trailing edge. We reserve ~138px of trailing padding.
 * - Linux: no reliable native chrome across distros. We draw our own
 *   controls at the trailing edge.
 */
export function DesktopWindowChrome({ children }: DesktopWindowChromeProps) {
  const windowState = useDesktopWindowState();
  const platform = windowState.platform;
  const ownsControls = rendererOwnsWindowControls(platform);

  const padStyle: CSSProperties =
    platform === "macos"
      ? { paddingInlineStart: `${WINDOW_CHROME_NATIVE_INSET_PX.macosLeading}px` }
      : platform === "windows"
        ? { paddingInlineEnd: `${WINDOW_CHROME_NATIVE_INSET_PX.windowsTrailing}px` }
        : {};

  return (
    <div
      className="flex min-h-9 items-center gap-3 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1"
      data-window-platform={platform}
      style={{ ...DRAG_REGION_STYLE, ...padStyle }}
    >
      <div className="min-w-0 flex-1">{children ?? null}</div>
      {ownsControls ? <LinuxWindowControls state={windowState} /> : null}
    </div>
  );
}

function LinuxWindowControls({ state }: { state: DesktopWindowState }) {
  return (
    <div
      aria-label="Window controls"
      className="flex items-center gap-1"
      style={NO_DRAG_REGION_STYLE}
    >
      <LinuxControlButton action={minimizeDesktopWindow} label="Minimize window">
        <Minus className="size-3.5" />
      </LinuxControlButton>
      <LinuxControlButton
        action={toggleDesktopWindowMaximize}
        disabled={!state.canMaximize}
        label={state.isMaximized ? "Restore window" : "Maximize window"}
      >
        <Square className="size-3" />
      </LinuxControlButton>
      <LinuxControlButton
        action={closeDesktopWindow}
        className="border-[var(--status-failed)]/40 text-[var(--status-failed)] hover:border-[var(--status-failed)]/70"
        disabled={!state.canClose}
        label="Close window"
      >
        <X className="size-3.5" />
      </LinuxControlButton>
    </div>
  );
}

function LinuxControlButton({
  action,
  children,
  className,
  disabled = false,
  label,
}: {
  action: () => Promise<void>;
  children: ReactNode;
  className?: string;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      aria-label={label}
      className={cn(
        "inline-flex size-6 items-center justify-center border border-[var(--border)] bg-[var(--bg)] text-[var(--fg-dim)] transition-colors hover:border-[var(--border-strong)] hover:bg-[var(--bg-sunken)] hover:text-[var(--fg)] disabled:cursor-not-allowed disabled:opacity-45",
        className,
      )}
      disabled={disabled}
      onClick={() => {
        void action();
      }}
      type="button"
    >
      {children}
    </button>
  );
}

function useDesktopWindowState(): DesktopWindowState {
  return useSyncExternalStore(
    subscribeDesktopWindow,
    getDesktopWindowSnapshot,
    getDesktopWindowServerSnapshot,
  );
}

async function closeDesktopWindow(): Promise<void> {
  await runDesktopWindowAction("close window", () => window.desktopWindow.close());
}

async function minimizeDesktopWindow(): Promise<void> {
  await runDesktopWindowAction("minimize window", () => window.desktopWindow.minimize());
}

async function toggleDesktopWindowMaximize(): Promise<void> {
  await runDesktopWindowAction("toggle maximize window", () =>
    window.desktopWindow.toggleMaximize(),
  );
}

async function runDesktopWindowAction(
  label: string,
  action: () => Promise<unknown>,
): Promise<void> {
  if (typeof window === "undefined" || window.desktopWindow == null) {
    return;
  }

  try {
    await action();
  } catch (error) {
    console.error(`failed to ${label}`, error);
  }
}
