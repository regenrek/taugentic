import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const electron = require("electron") as typeof import("electron");

export const app = electron.app;
export const BrowserWindow = electron.BrowserWindow;
export const dialog = electron.dialog;
export const ipcMain = electron.ipcMain;
export const MessageChannelMain = electron.MessageChannelMain;
