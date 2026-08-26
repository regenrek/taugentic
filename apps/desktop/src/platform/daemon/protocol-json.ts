/** Decode one generated daemon wire value without changing its canonical JSON shape. */
export function decodeProtocolJson<T>(json: string): T {
  return JSON.parse(json) as T
}

/** Compare canonical unsigned decimal strings without crossing JavaScript's safe-integer limit. */
export function compareProtocolU64(left: string, right: string): number {
  if (left.length !== right.length) return left.length < right.length ? -1 : 1
  return left < right ? -1 : left > right ? 1 : 0
}
