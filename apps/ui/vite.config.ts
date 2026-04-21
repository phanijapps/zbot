import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// z-Bot Web Dashboard
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    target: "esnext",
    outDir: "../../dist",
    minify: "terser",
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
        pure_funcs: ["console.log", "console.info", "console.debug"],
      },
    },
    rollupOptions: {
      output: {
        manualChunks: {
          "radix-ui": [
            "@radix-ui/react-dialog",
            "@radix-ui/react-dropdown-menu",
            "@radix-ui/react-label",
            "@radix-ui/react-scroll-area",
            "@radix-ui/react-select",
            "@radix-ui/react-separator",
            "@radix-ui/react-slot",
            "@radix-ui/react-switch",
            "@radix-ui/react-tabs",
            "@radix-ui/react-tooltip",
          ],
          markdown: ["@uiw/react-md-editor", "react-markdown", "remark-gfm"],
        },
      },
    },
    chunkSizeWarningLimit: 1000,
  },
  server: {
    host: "0.0.0.0",
    port: 3000,
    strictPort: false,
    // HMR over LAN: the default client assumes `wss://localhost:<port>`
    // when the page is served over anything other than localhost, which
    // silently hangs on phones. Use an explicit non-TLS websocket with
    // the same port as the page; the `host` field is left unset so the
    // client infers `window.location.hostname` dynamically — that way
    // the same config works for the dev machine and any LAN device.
    hmr: {
      protocol: "ws",
      clientPort: 3000,
    },
    proxy: {
      "/api": {
        target: "http://localhost:18791",
        changeOrigin: true,
      },
    },
  },
});
