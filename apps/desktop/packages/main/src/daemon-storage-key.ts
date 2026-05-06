import { createHash } from "node:crypto";

export function clientStorageKey(clientName: string): string {
  return stableStorageKey(clientName.trim());
}

export function sessionStorageKey(sessionId: string): string {
  return stableStorageKey(sessionId);
}

function stableStorageKey(value: string): string {
  return createHash("sha256").update(value, "utf8").digest("hex");
}
