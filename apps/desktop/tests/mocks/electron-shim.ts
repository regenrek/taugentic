/**
 * Vitest resolves `electron` here so main-process modules load under Node.
 * The real `electron` package entry is the CLI binary path, not an API surface.
 */
export const app = {
  get isPackaged(): boolean {
    return process.env.TAUGENTIC_TEST_ELECTRON_PACKAGED === "1";
  },
};

export function BrowserWindow(): void {}
BrowserWindow.getFocusedWindow = (): null => null;
BrowserWindow.fromWebContents = (): null => null;

export const ipcMain = {
  handle: (): void => {},
  on: (): void => {},
};

export const dialog = {
  showSaveDialog: async (): Promise<{ canceled: boolean; filePath?: string }> => ({
    canceled: true,
  }),
};

export function MessageChannelMain(): void {}

export default { app, BrowserWindow, dialog, ipcMain, MessageChannelMain };
