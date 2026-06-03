import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

// Component tests run under happy-dom. Only *.test.ts here — the existing
// plain-Node *.test.mjs files (i18n / rate) keep running via `node` directly.
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.ts"],
  },
});
