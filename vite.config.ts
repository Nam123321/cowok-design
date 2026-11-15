import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { cowokLoopbackBffPlugin } from "./src/lib/vite-bff-plugin.ts";

export default defineConfig({
  plugins: [react(), cowokLoopbackBffPlugin()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
      "@cowok-design/shared-types": path.resolve(import.meta.dirname, "./packages/shared-types/src/index.ts"),
      "@cowok-design/builtin-skills": path.resolve(import.meta.dirname, "./packages/builtin-skills/src/index.ts"),
      "@cowok-design/ui-shell": path.resolve(import.meta.dirname, "./packages/ui-shell/src/index.ts"),
    },
  },
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            { name: "vendor-react", test: /node_modules[\\/](react|react-dom|scheduler)[\\/]/, priority: 30 },
            { name: "vendor-zod", test: /node_modules[\\/]zod[\\/]/, priority: 20 },
            { name: "workbench-modals", test: /src[\\/]pages[\\/]workbench[\\/]components[\\/](?:.*Modal|Onboarding.*|Settings.*)/, priority: 15 },
            { name: "workbench-flow", test: /src[\\/]features[\\/]flow[\\/]/, priority: 10 },
          ],
        },
      },
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
