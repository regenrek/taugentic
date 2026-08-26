import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
export const { NativeDaemonBridge } = require("./taugentic_desktop_native.node");
