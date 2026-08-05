import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Vite, no SvelteKit. ARCHITECTURE §1: a single-window editor needs no router,
// no SSR, and no file-based routing, and Vite's static output is consumed
// unchanged by both the Tauri bundle and the static host (M5).
export default defineConfig({
  plugins: [svelte()],

  // Tauri drives the dev server and expects it at a known port. Failing rather
  // than silently moving to 1421 matters: the shell would load a stale build
  // from the port it was told about and the mismatch is invisible.
  server: {
    port: 1420,
    strictPort: true,
    host: false,

    // Cargo's output is not source, and watching it is actively fatal on
    // Windows: the linker holds the shell's .exe open exclusively while it
    // writes, so `fs.watch` on that path throws EBUSY, chokidar re-emits it as
    // an unhandled error, and Vite exits — taking `tauri dev` down with it in
    // the middle of the build that caused it. There is nothing in here Vite
    // could serve, so the fix is simply not to look.
    watch: {
      ignored: ["**/target/**", "**/fuzz/**", "**/corpus/**"],
    },
  },

  // Tauri shows its own startup output; clearing the screen hides it.
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],

  build: {
    outDir: "dist",
    emptyOutDir: true,
    // Windows 10 and macOS ship WebView2 and WKWebView respectively, both
    // evergreen, so there is no reason to transpile down.
    target: "es2022",
    // Debug bundles keep their sources so a panic in the shell can be traced
    // back to a line rather than to a minified column.
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    // Vite 8's default minifier. Naming esbuild explicitly no longer works --
    // it is a separate install since the move to rolldown.
    minify: !process.env.TAURI_ENV_DEBUG,
  },
});
