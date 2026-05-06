import { defineConfig } from "vite-plus";

export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
    minify: false,
    lib: {
      entry: "src/index.ts",
      formats: ["cjs"],
    },
    rollupOptions: {
      external: ["electron"],
      output: {
        entryFileNames: "preload.cjs",
      },
    },
  },
});
