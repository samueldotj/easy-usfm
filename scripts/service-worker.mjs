/**
 * The service worker, generated at build time — P5.1.
 *
 * PRODUCT §12: "Precached shell, WASM binary, and fonts; no runtime caching
 * needed since there are no runtime requests."
 *
 * That last clause is what makes this small. A service worker for an
 * application that talks to a server is a cache-strategy problem — stale-while-
 * revalidate, network-first for this route, cache-first for that one. This
 * application makes no requests after it loads: the engine is a WASM module in
 * the bundle, the document comes from the user's disk, and nothing is fetched.
 * So the whole worker is "install everything, then serve from the cache".
 *
 * # A Vite plugin rather than Workbox
 *
 * Workbox would generate this, and it would bring a build-time dependency and a
 * runtime library to solve a problem that is one `cache.addAll` long. The list
 * of files to precache is the only thing that has to be derived, and Vite
 * already knows it — the bundle is in the manifest it just emitted.
 *
 * # Why the worker never activates itself
 *
 * "New service workers install in the background behind a 'A new version is
 * ready — Reload' bar; never auto-reload mid-edit." A worker that calls
 * `skipWaiting()` on its own replaces the running one and the next navigation
 * gets new code — which, in an editor, can happen while somebody is typing into
 * a document that is not saved. So it waits, and only steps forward when the
 * page tells it to, which the page only does when the user asks.
 */

import { createHash } from "node:crypto";

/** Files that are part of the shell even though nothing imports them. */
const EXTRA = ["manifest.webmanifest", "icons/icon-128.png", "icons/icon-256.png"];

/**
 * Emits `sw.js` alongside the bundle, precaching everything in it.
 *
 * @returns {import("vite").Plugin}
 */
export function serviceWorker() {
  return {
    name: "easy-usfm-service-worker",
    apply: "build",

    generateBundle(_options, bundle) {
      // Every emitted chunk and asset. Nothing is filtered by extension: the
      // WASM binary, the fonts and the stylesheets are all shell, and a list
      // that names extensions is a list that forgets one.
      const files = Object.keys(bundle).sort();
      const precache = ["./", ...files.map((name) => `./${name}`), ...EXTRA.map((name) => `./${name}`)];

      // The cache name has to change when the bundle does, or an update
      // installs and then serves the old files out of the old cache.
      const version = createHash("sha256").update(precache.join("\n")).digest("hex").slice(0, 16);

      this.emitFile({ type: "asset", fileName: "sw.js", source: worker(version, precache) });
    },
  };
}

function worker(version, precache) {
  return `// Generated at build time. Do not edit; see scripts/service-worker.mjs.
const CACHE = "easy-usfm-${version}";
const PRECACHE = ${JSON.stringify(precache, null, 2)};

self.addEventListener("install", (event) => {
  // Deliberately no skipWaiting(). A new worker installs in the background and
  // waits; the page decides when it takes over, and only when the user asks.
  // Replacing the running worker mid-edit is how an editor reloads out from
  // under somebody who has not saved.
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      // Every other version of this application's cache, which is now dead
      // weight. Other origins' caches are not visible here, so this cannot
      // touch anything that is not ours.
      const names = await caches.keys();
      await Promise.all(
        names
          .filter((name) => name.startsWith("easy-usfm-") && name !== CACHE)
          .map((name) => caches.delete(name)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  // Only this origin's own GETs. A POST is not cacheable and a cross-origin
  // request is not ours to answer -- and the CSP forbids them anyway.
  if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) return;

  event.respondWith(
    (async () => {
      // Matched by URL with \`ignoreVary\`, not by passing the Request.
      //
      // A precached response was stored against the request \`addAll\` made; the
      // page's own request for the same file carries different \`Accept\` and
      // \`Sec-Fetch-*\` headers, so if the server sent any \`Vary\` the two do not
      // match and a file that is demonstrably in the cache is reported as a
      // miss. With the network up that is invisible -- it just fetches. With
      // the network down the application does not start.
      const options = { cacheName: CACHE, ignoreVary: true };

      const cached = await caches.match(request.url, options);
      if (cached) return cached;

      // A navigation is answered with the shell whatever its path, so a deep
      // link or a reload works with the network off.
      if (request.mode === "navigate") {
        const shell = await caches.match(new URL("./", self.location.href).href, options);
        if (shell) return shell;
      }

      // Not in the cache. There are no runtime requests by design, so this is
      // something new -- go to the network, and let it fail honestly rather
      // than serving a wrong file from a different version.
      return fetch(request);
    })().catch(() => fetch(request)),
  );
});

self.addEventListener("message", (event) => {
  // The page asking this worker to take over, which it does only after the
  // user has chosen to reload.
  if (event.data === "take-over") self.skipWaiting();
});
`;
}
