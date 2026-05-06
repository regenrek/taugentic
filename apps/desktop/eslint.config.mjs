import eslint from "@eslint/js";
import prettierConfig from "eslint-config-prettier";
import globals from "globals";
import tseslint from "typescript-eslint";

const mainBoundaryImports = [
  "@taugentic/desktop-preload",
  "@taugentic/desktop-preload/*",
  "@taugentic/desktop-renderer",
  "@taugentic/desktop-renderer/*",
  "**/preload/**",
  "**/renderer/**",
  "react",
  "react/*",
  "react-dom",
  "react-dom/*",
];

const preloadBoundaryImports = [
  "@taugentic/desktop-main",
  "@taugentic/desktop-main/*",
  "@taugentic/desktop-renderer",
  "@taugentic/desktop-renderer/*",
  "**/main/**",
  "**/renderer/**",
  "react",
  "react/*",
  "react-dom",
  "react-dom/*",
];

const rendererBoundaryImports = [
  "electron",
  "electron/*",
  "node:*",
  "@taugentic/desktop-main",
  "@taugentic/desktop-main/*",
  "@taugentic/desktop-preload",
  "@taugentic/desktop-preload/*",
  "**/main/**",
  "**/preload/**",
];

const sharedBoundaryImports = [
  "electron",
  "electron/*",
  "node:*",
  "react",
  "react/*",
  "react-dom",
  "react-dom/*",
  "@taugentic/desktop-main",
  "@taugentic/desktop-main/*",
  "@taugentic/desktop-preload",
  "@taugentic/desktop-preload/*",
  "@taugentic/desktop-renderer",
  "@taugentic/desktop-renderer/*",
  "**/main/**",
  "**/preload/**",
  "**/renderer/**",
];

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  {
    ignores: ["**/dist/**", "**/coverage/**", "**/.artifacts/**", "packages/shared/generated/**"],
  },
  {
    files: ["**/*.{js,mjs,cjs,ts,tsx}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
      "no-empty": [
        "error",
        {
          allowEmptyCatch: true,
        },
      ],
    },
  },
  {
    files: [
      "electron-builder.config.mjs",
      "eslint.config.mjs",
      "scripts/**/*.{js,mjs,cjs}",
      "vite.config.ts",
      "packages/*/vite.config.ts",
    ],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    files: ["packages/main/src/**/*.ts", "packages/preload/src/**/*.ts", "tests/**/*.ts"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    files: ["packages/renderer/src/**/*.{ts,tsx}"],
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
  },
  {
    files: ["packages/main/src/**/*.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: mainBoundaryImports,
              message:
                "desktop-main owns native orchestration. Do not import preload or renderer code into main.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["packages/preload/src/**/*.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: preloadBoundaryImports,
              message:
                "desktop-preload is a thin bridge only. Keep main orchestration and renderer UI out of preload.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["packages/renderer/src/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: rendererBoundaryImports,
              message:
                "desktop-renderer must stay sandboxed. Go through lib/ipc and shared contracts instead of Electron, main, or preload modules.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["packages/renderer/src/**/*.{ts,tsx}"],
    ignores: ["packages/renderer/src/lib/ipc/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-syntax": [
        "error",
        {
          selector: "MemberExpression[object.name='window'][property.name='desktopApi']",
          message:
            "Use packages/renderer/src/lib/ipc as the canonical desktop bridge instead of accessing window.desktopApi directly.",
        },
      ],
    },
  },
  {
    files: ["packages/shared/src/**/*.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: sharedBoundaryImports,
              message:
                "desktop-shared is the transport and validation SSOT. Keep Electron, Node, UI, and other package runtime code out of shared.",
            },
          ],
        },
      ],
    },
  },
  prettierConfig,
);
