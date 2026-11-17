import { defineConfig } from "vitest/config";
import path from "node:path";

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "../../../src"),
      "@cowok-design/shared-types": path.resolve(__dirname, "../../../packages/shared-types/src/index.ts"),
      "@cowok-design/builtin-skills": path.resolve(__dirname, "../../../packages/builtin-skills/src/index.ts"),
      "@cowok-design/ui-shell": path.resolve(__dirname, "../../../packages/ui-shell/src/index.ts"),
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/features/flow/**/*.test.ts", "src/features/flow/**/*.test.tsx", "src/features/sdi/**/*.test.ts", "src/features/sdi/**/*.test.tsx"],
  },
});
