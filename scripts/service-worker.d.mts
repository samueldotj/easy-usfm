/**
 * Types for the build-time plugin.
 *
 * The plugin itself is `.mjs` because it runs in Vite's own Node process,
 * outside the application's TypeScript build — but `vite.config.ts` is checked
 * with everything else, so it needs a declaration to import against.
 */

import type { Plugin } from "vite";

export declare function serviceWorker(): Plugin;
