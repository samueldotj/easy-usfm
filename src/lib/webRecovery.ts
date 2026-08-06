/**
 * Recovery in a browser — FILE-FIDELITY §4, P4.6.
 *
 * "IndexedDB keyed by file name, size, and last-modified; same cadence and
 * retention. `navigator.locks` for the cross-tab equivalent. `beforeunload`
 * warns on dirty state; a final snapshot flushes on `visibilitychange →
 * hidden`, the only reliably-fired teardown event."
 *
 * # Why the key is three things
 *
 * A browser is never told where a file came from. The File System Access API
 * hands over a handle, and `showOpenFilePicker` gives a different handle object
 * for the same file next session — there is no path and no inode to key on. Name
 * alone would make every `GEN.usfm` on the machine one document, which is the
 * common case in translation work: a folder per project, the same book names in
 * each. Adding size and last-modified makes a collision require two files with
 * the same name, the same length, and the same timestamp to the millisecond.
 *
 * It also means editing a file elsewhere and coming back finds no snapshot,
 * because the timestamp moved. That is the right failure: a snapshot taken
 * against different contents is not a recovery, it is an overwrite waiting to
 * happen.
 *
 * # Why localStorage is not enough
 *
 * A snapshot is a whole document, and `localStorage` is synchronous, string-only,
 * and capped around five megabytes across the origin. PRODUCT §2 has documents up
 * to two megabytes; three generations of one would not fit beside the settings.
 * IndexedDB is asynchronous, stores structured values, and has quota measured in
 * hundreds of megabytes.
 */

const DATABASE = "easy-usfm-recovery";
const STORE = "snapshots";

/** FILE-FIDELITY §4: "Last 3 retained". Identical to the desktop. */
const KEEP = 3;

/** What identifies a document without a path. */
export interface FileKey {
  name: string;
  size: number;
  lastModified: number;
}

/** One stored snapshot. */
export interface WebSnapshot {
  key: string;
  takenAt: number;
  text: string;
  cursor: number;
  dirty: boolean;
  /** Enough of the envelope to write the file back as it was. */
  eol: string;
  bom: boolean;
  finalNewline: boolean;
}

/**
 * The identity of a file, as a string.
 *
 * The name is percent-encoded so a file called `a|1|2` cannot be made to look
 * like a different file's key. Document content is not involved, but a file
 * *name* is still something someone else chose.
 */
export function keyOf(file: FileKey): string {
  return [encodeURIComponent(file.name), file.size, file.lastModified].join("|");
}

/** Opens the database, creating the store on first use. */
function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);

    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(STORE)) {
        // Keyed by the pair, so retention can read one document's generations
        // without scanning every document in the store.
        const store = database.createObjectStore(STORE, { keyPath: ["key", "takenAt"] });
        store.createIndex("key", "key");
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("could not open the database"));
  });
}

function finish(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("transaction aborted"));
  });
}

function request<T>(source: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    source.onsuccess = () => resolve(source.result);
    source.onerror = () => reject(source.error ?? new Error("request failed"));
  });
}

/**
 * Writes a snapshot and drops whatever falls off the end.
 *
 * Retention after the write, for the same reason the desktop does it that way:
 * pruning first would leave a crash between the two with fewer snapshots than a
 * moment earlier, to save one record.
 */
export async function put(snapshot: WebSnapshot): Promise<void> {
  const database = await open();
  try {
    const transaction = database.transaction(STORE, "readwrite");
    transaction.objectStore(STORE).put(snapshot);
    await finish(transaction);
    await retain(database, snapshot.key);
  } finally {
    database.close();
  }
}

async function retain(database: IDBDatabase, key: string): Promise<void> {
  const transaction = database.transaction(STORE, "readwrite");
  const store = transaction.objectStore(STORE);
  const existing = await request<WebSnapshot[]>(
    store.index("key").getAll(IDBKeyRange.only(key)) as IDBRequest<WebSnapshot[]>,
  );

  // Newest first, so what is dropped is the tail.
  existing.sort((left, right) => right.takenAt - left.takenAt);
  for (const stale of existing.slice(KEEP)) {
    store.delete([stale.key, stale.takenAt]);
  }
  await finish(transaction);
}

/** The newest snapshot for a file, if there is one. */
export async function newest(key: string): Promise<WebSnapshot | null> {
  const database = await open();
  try {
    const store = database.transaction(STORE, "readonly").objectStore(STORE);
    const found = await request<WebSnapshot[]>(
      store.index("key").getAll(IDBKeyRange.only(key)) as IDBRequest<WebSnapshot[]>,
    );
    if (found.length === 0) return null;

    found.sort((left, right) => right.takenAt - left.takenAt);
    return found[0] ?? null;
  } finally {
    database.close();
  }
}

/** Forgets a file's snapshots, on a clean save. */
export async function clear(key: string): Promise<void> {
  const database = await open();
  try {
    const transaction = database.transaction(STORE, "readwrite");
    const store = transaction.objectStore(STORE);
    const found = await request<WebSnapshot[]>(
      store.index("key").getAll(IDBKeyRange.only(key)) as IDBRequest<WebSnapshot[]>,
    );
    for (const stale of found) store.delete([stale.key, stale.takenAt]);
    await finish(transaction);
  } finally {
    database.close();
  }
}

/**
 * Holds a cross-tab lock on a file for as long as this tab lives.
 *
 * `navigator.locks` is the browser's equivalent of the advisory lock: a lock is
 * released automatically when the tab holding it goes away, including a crash,
 * which is the part a lock file cannot do without a liveness check.
 *
 * The lock is held by a promise that never settles, which is the documented way
 * to hold one indefinitely — the callback's promise *is* the lock's lifetime.
 * `ifAvailable` so a second tab is told immediately rather than queueing behind
 * the first and appearing to hang.
 */
export class TabLock {
  #release: (() => void) | null = null;

  /** Takes it, or reports that another tab has it. */
  async take(key: string): Promise<boolean> {
    if (!("locks" in navigator)) return true;
    this.release();

    return new Promise<boolean>((resolve) => {
      void navigator.locks.request(
        `easy-usfm:${key}`,
        { ifAvailable: true },
        (lock) =>
          new Promise<void>((releaseLock) => {
            if (lock === null) {
              resolve(false);
              releaseLock();
              return;
            }
            this.#release = releaseLock;
            resolve(true);
          }),
      );
    });
  }

  /** Gives it up, when moving to another file. */
  release(): void {
    this.#release?.();
    this.#release = null;
  }
}
