// @ts-check
import { defineConfig } from "astro/config";

// https://astro.build/config
export default defineConfig({
  site: "https://terminal-svg.dev",
  vite: {
    server: {
      // Gallery and docs SVGs are imported from the repo root, outside site/
      fs: { allow: [".."] },
    },
  },
});
