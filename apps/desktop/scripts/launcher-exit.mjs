import { constants } from "node:os";

export function resolveLauncherExitCode({ exitCode, signal } = {}) {
  if (typeof exitCode === "number") {
    return exitCode;
  }

  if (typeof signal === "string") {
    const signalNumber = constants.signals[signal];
    return typeof signalNumber === "number" ? 128 + signalNumber : 1;
  }

  return 0;
}
