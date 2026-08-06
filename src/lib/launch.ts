/**
 * The shape of `window.launchQueue` — P5.2.
 *
 * Declared here because no TypeScript DOM library carries it: file handling is
 * a Chromium-only capability at the time of writing, and `lib.dom` describes
 * what is standard. Written as a narrow interface rather than a global
 * augmentation so that reaching for it is visibly a capability check, not an
 * assumption that it exists.
 */

export interface LaunchParams {
  /** Handles for the files the operating system opened this application with. */
  files?: FileSystemFileHandle[];
}

export interface LaunchQueueHost {
  launchQueue: {
    /**
     * Set once, as early as possible. The browser holds the launch until a
     * consumer exists, so setting one late means the file arrives at a
     * document the user is already looking at.
     */
    setConsumer(consumer: (launch: LaunchParams) => void): void;
  };
}
