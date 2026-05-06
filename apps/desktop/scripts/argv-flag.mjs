/**
 * @param {string[]} argv
 * @param {string} flagName e.g. "--platform"
 * @returns {string | null}
 */
export function parseArgvFlagValue(argv, flagName) {
  const prefix = `${flagName}=`;
  for (const entry of argv) {
    if (typeof entry === "string" && entry.startsWith(prefix)) {
      const value = entry.slice(prefix.length).trim();
      return value.length > 0 ? value : null;
    }
  }
  const idx = argv.indexOf(flagName);
  if (idx !== -1) {
    const next = argv[idx + 1];
    if (typeof next === "string" && next.length > 0 && !next.startsWith("-")) {
      return next.trim();
    }
  }
  return null;
}
