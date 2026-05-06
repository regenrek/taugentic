import path from "node:path";

import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import browserEcho from "@browser-echo/vite";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // Terminal-only browser log ingestion for local renderer debugging.
    browserEcho({
      tag: "[desktop-renderer]",
      stackMode: "condensed",
      colors: true,
      mcp: {
        url: "",
        suppressTerminal: false,
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "src"),
    },
  },
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
