/**
 * Installing the service worker, and the update bar — P5.1.
 *
 * PRODUCT §12: "New service workers install in the background behind a 'A new
 * version is ready — Reload' bar; never auto-reload mid-edit. The offline claim
 * is scoped honestly: **after first load**, no network required."
 *
 * # The bar exists because the alternative is worse
 *
 * A service worker that updates itself replaces the running one, and the next
 * navigation gets new code. In most applications that is invisible. In an editor
 * it is a page reload while somebody is typing into a document that is not
 * saved — the one thing this whole project is arranged to prevent. So the new
 * worker installs, waits, and does nothing until the user says so.
 *
 * # Not on the desktop
 *
 * The Tauri build serves its assets from the shell, not over HTTP. Registering a
 * worker there would be caching a cache, and a stale one at that: the desktop's
 * assets change when the application is updated, which the worker knows nothing
 * about.
 */

import { isDesktop } from "./shell";

class Pwa {
  /**
   * A new version has installed and is waiting.
   *
   * Drives the bar. Never acted on automatically.
   */
  updateReady = $state(false);

  /** Whether the shell is cached, so the offline claim can be made honestly. */
  offlineReady = $state(false);

  #waiting: ServiceWorker | null = null;

  /**
   * Registers the worker, and watches for a successor.
   *
   * Silent on failure. A service worker is what makes the application work
   * offline; failing to register one means it works online, which is how it
   * behaved before this existed and is not worth an error message.
   */
  async register(): Promise<void> {
    if (isDesktop() || !("serviceWorker" in navigator)) return;

    try {
      // `updateViaCache: "none"` so the worker script is fetched from the
      // network rather than the HTTP cache. Without it the browser can hand
      // back a previous `sw.js`, which then precaches a previous bundle's file
      // names — and the application is offline-ready for a version that is no
      // longer being served. Seen: a reload with the server stopped served the
      // shell from the cache and then failed on every asset in it.
      const registration = await navigator.serviceWorker.register("./sw.js", {
        scope: "./",
        updateViaCache: "none",
      });

      // Already waiting when the page loaded: a previous visit installed an
      // update and the user never reloaded.
      if (registration.waiting) this.#found(registration.waiting);

      registration.addEventListener("updatefound", () => {
        const installing = registration.installing;
        if (!installing) return;

        installing.addEventListener("statechange", () => {
          if (installing.state !== "installed") return;

          // `controller` is null on the very first visit, when this worker is
          // not an update at all -- it is the first one. Telling somebody a
          // new version is ready thirty seconds after they first opened the
          // page would be nonsense.
          if (navigator.serviceWorker.controller) this.#found(installing);
          else this.offlineReady = true;
        });
      });

      if (navigator.serviceWorker.controller) this.offlineReady = true;
    } catch {
      // See above.
    }
  }

  #found(worker: ServiceWorker): void {
    this.#waiting = worker;
    this.updateReady = true;
  }

  /**
   * Takes the update, when the user asks for it.
   *
   * Two steps, in this order: the worker is told to step forward, and the page
   * reloads only once it has. Reloading first would load the old assets again
   * from the old worker and the bar would come back.
   */
  reload(): void {
    const waiting = this.#waiting;
    if (!waiting) {
      window.location.reload();
      return;
    }

    navigator.serviceWorker.addEventListener("controllerchange", () => window.location.reload(), {
      once: true,
    });
    waiting.postMessage("take-over");
  }

  /** Leaves it waiting. It will still be there next time. */
  dismiss(): void {
    this.updateReady = false;
  }
}

export const pwa = new Pwa();
