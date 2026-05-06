import path from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite-plus";

const desktopRootDir = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  run: {
    tasks: {
      "desktop:start": {
        command: "node ./scripts/launch-desktop.mjs start",
        cache: false,
      },
      "desktop:dev": {
        command: "node ./scripts/launch-desktop.mjs dev",
        cache: false,
      },
    },
  },
  fmt: {
    ignorePatterns: ["**/dist/**", "packages/shared/generated/**"],
  },
  lint: {
    ignorePatterns: ["**/dist/**", "packages/shared/generated/**"],
    options: {
      typeAware: true,
      typeCheck: true,
    },
  },
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    alias: {
      "@": path.join(desktopRootDir, "packages/renderer/src"),
      electron: path.join(desktopRootDir, "tests/mocks/electron-shim.ts"),
    },
  },
});
