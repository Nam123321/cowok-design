import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { cowokLoopbackBffPlugin } from "./src/lib/vite-bff-plugin";

export default defineConfig({
  plugins: [react(), cowokLoopbackBffPlugin()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@cowok-design/shared-types": path.resolve(__dirname, "./packages/shared-types/src/index.ts"),
      "@cowok-design/builtin-skills": path.resolve(__dirname, "./packages/builtin-skills/src/index.ts"),
      "@cowok-design/ui-shell": path.resolve(__dirname, "./packages/ui-shell/src/index.ts"),
    },
  },
  optimizeDeps: {
    entries: ["index.html"],
  },
  server: {
    host: "127.0.0.1",
  },
  preview: {
    host: "127.0.0.1",
  },
});
