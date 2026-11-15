import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    // During development the UI talks to a locally running hearthd.
    proxy: { "/api": { target: process.env.HEARTH_API ?? "http://127.0.0.1:9000", ws: true } },
  },
  build: {
    // hearthd embeds this folder into the binary.
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 350,
    rolldownOptions: {
      output: {
        // Stable vendor chunks: app updates don't invalidate cached libraries.
        manualChunks(id: string) {
          if (!id.includes("node_modules")) return undefined;
          // Package name = path after the last node_modules/ (pnpm dirs embed peer names).
          const pkg = id.split("node_modules/").pop() ?? "";
          if (pkg.startsWith("@phosphor-icons/")) return "icons";
          if (/^(react|react-dom|scheduler)\//.test(pkg)) return "react";
          if (/^(@cloudflare\/kumo|@base-ui|@floating-ui|date-fns|@date-fns)/.test(pkg)) return "kumo";
          if (/^(motion|framer-motion|motion-dom|motion-utils)\//.test(pkg)) return "motion";
          if (pkg.startsWith("@tanstack/")) return "query";
          return "vendor";
        },
      },
    },
  },
});
