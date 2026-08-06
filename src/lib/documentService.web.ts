/**
 * The browser half of {@link DocumentService}.
 *
 * # Two browsers, not one
 *
 * The File System Access API gives a real handle: open a file, edit it, save
 * it back in place, exactly as on the desktop. Chromium has it; Firefox and
 * Safari do not, and there is no polyfill that can invent a writable handle.
 *
 * So there are two paths, and the difference is visible rather than hidden.
 * Where there is no handle, opening is a file input and saving is a download —
 * which lands in the downloads folder under a name the browser chooses, and
 * does *not* update the file the user opened. Pretending otherwise would be
 * the worst possible failure for an editor: silently not saving.
 * {@link webDocuments} reports that in `limitations`, and `canSaveInPlace`
 * answers honestly so the interface can say "Download" instead of "Save".
 *
 * # Fidelity is not a desktop feature
 *
 * The envelope — BOM, per-line endings, final newline — is read and written by
 * the same Rust that the desktop uses, compiled to WASM. A file opened here
 * and saved unchanged produces the same bytes it would there.
 *
 * The one guarantee that cannot cross is atomicity. ADR-005's ladder needs
 * `rename` within a directory, and no browser API offers it: a
 * `FileSystemWritableFileStream` truncates on open, so an interrupted write
 * leaves a short file. That is a real difference and it is listed.
 */

import type { DocumentService, Editable, Opened, SaveOutcome } from "./documentService";
import type { Eol } from "./eol";

/**
 * The File System Access API, which TypeScript's DOM library does not yet
 * declare.
 *
 * Only the two entry points this file uses, and only the arguments it passes.
 * A fuller declaration would be a second, drifting copy of a specification
 * that is still moving; `hasFileSystemAccess` is what actually decides whether
 * these exist, and it asks the browser rather than the type system.
 */
declare global {
  interface Window {
    showOpenFilePicker?: (options?: {
      multiple?: boolean;
      types?: { description: string; accept: Record<string, string[]> }[];
    }) => Promise<FileSystemFileHandle[]>;
    showSaveFilePicker?: (options?: {
      suggestedName?: string;
      types?: { description: string; accept: Record<string, string[]> }[];
    }) => Promise<FileSystemFileHandle>;
  }
}

/** What the engine's `decodeFile` returns. */
interface Decoded {
  text: string;
  bom: boolean;
  eols: Eol[];
  eol: string;
  final_newline: boolean;
  mixed_eol: boolean;
  len: number;
}

/**
 * The fidelity half of the engine, loaded directly rather than through the
 * worker.
 *
 * Deliberately not the worker: this runs once per open and once per save, both
 * of which already involve a file dialog, and routing it through the delta
 * protocol would mean adding two message kinds to carry bytes — the one thing
 * ARCHITECTURE §9 says must not cross that boundary.
 */
async function fidelity() {
  const module = await import("../generated/wasm/easy_usfm_wasm");
  const url = (await import("../generated/wasm/easy_usfm_wasm_bg.wasm?url")).default;
  await module.default({ module_or_path: url });
  return module;
}


function summaryOf(decoded: Decoded) {
  return {
    encoding: decoded.bom ? "UTF-8 with BOM" : "UTF-8",
    eol: decoded.eol,
    bom: decoded.bom,
    final_newline: decoded.final_newline,
    mixed_eol: decoded.mixed_eol,
  };
}

/** Whether this browser can hand back a writable handle. */
function hasFileSystemAccess(): boolean {
  return typeof window !== "undefined" && "showOpenFilePicker" in window;
}

export function webDocuments(): DocumentService {
  /**
   * The handle for the open document, where the browser gave one.
   *
   * Held here rather than in the document state because it is not
   * serialisable, not displayable, and means nothing to the interface — all it
   * can do is be saved back to.
   */
  let handle: FileSystemFileHandle | null = null;

  // SECURITY §3's last sentence, said out loud rather than left as a figure
  // that silently never appears.
  const noImages =
    "Images in figures are not shown in a browser. A browser hands over one " +
    "file, not the folder around it, so there is nowhere to read them from.";

  const limitations = hasFileSystemAccess()
    ? [
        noImages,
        "Saving is not atomic: the browser truncates the file before writing, " +
          "so an interrupted save can leave it short. The desktop application " +
          "writes through a temporary file and renames it.",
      ]
    : [
        noImages,
        "This browser cannot save back to the file you opened. Save downloads " +
          "a copy instead, and you will need to replace the original yourself.",
        "Saving is not atomic.",
      ];

  return {
    limitations,

    async createNew(): Promise<Opened> {
      handle = null;
      const engine = await fidelity();
      // The template comes from the engine, so New gives the same document
      // here as it does on the desktop.
      const decoded = engine.decodeFile(
        new TextEncoder().encode(engine.newDocument()),
      ) as Decoded;

      return {
        id: null,
        path: null,
        text: decoded.text,
        summary: summaryOf(decoded),
        eols: decoded.eols,
      };
    },

    async open(): Promise<Opened | null> {
      const file = hasFileSystemAccess() ? await pickWithHandle() : await pickWithInput();
      if (!file) return null;

      const engine = await fidelity();
      const bytes = new Uint8Array(await file.arrayBuffer());

      // Throws on bytes that are not UTF-8, which is the honest answer: a
      // lossy decode makes byte-for-byte preservation impossible before the
      // user has typed anything (FILE-FIDELITY §1).
      const decoded = engine.decodeFile(bytes) as Decoded;

      return {
        id: null,
        path: file.name,
        text: decoded.text,
        summary: summaryOf(decoded),
        eols: decoded.eols,
      };
    },

    canSaveInPlace(): boolean {
      return handle !== null;
    },

    async save(document: Editable): Promise<SaveOutcome> {
      if (!handle) return this.saveAs(document);

      const bytes = await serialize(document);
      const writable = await handle.createWritable();
      await writable.write(new Blob([bytes as BlobPart]));
      await writable.close();

      return outcome(document, handle.name, "the browser");
    },

    async saveAs(document: Editable): Promise<SaveOutcome> {
      const bytes = await serialize(document);
      const name = document.path ?? "untitled.usfm";

      if (hasFileSystemAccess()) {
        const chosen = await saveWithHandle(name);
        if (!chosen) return notSaved(document);

        handle = chosen;
        const writable = await chosen.createWritable();
        await writable.write(new Blob([bytes as BlobPart]));
        await writable.close();
        return outcome(document, chosen.name, "the browser");
      }

      download(bytes, name);
      // No handle, so the next Save downloads again rather than pretending to
      // write back to something.
      return outcome(document, name, "download");
    },

    async confirmDiscard(name: string): Promise<boolean> {
      return confirm(`${name} has unsaved changes. Discard them?`);
    },

    async setRecentFiles(): Promise<void> {
      // No native menu to push into. The list still exists in settings and the
      // interface's own recent list reads it.
    },

    async readFigure(): Promise<Uint8Array | null> {
      // SECURITY §3: "the web build never loads local images". A browser has
      // no path to the folder the file came from -- the File System Access API
      // hands over one file and nothing around it -- so this is not a
      // restriction being enforced, it is the honest answer.
      return null;
    },
  };
}

async function serialize(document: Editable): Promise<Uint8Array> {
  const engine = await fidelity();
  // The same serializer the desktop uses, so the bytes are the same bytes.
  return engine.encodeFile(document.text, document.bom, document.eols);
}

async function pickWithHandle(): Promise<File | null> {
  const picker = window.showOpenFilePicker;
  if (!picker) return null;

  try {
    const [chosen] = await picker({
      multiple: false,
      types: [{ description: "USFM", accept: { "text/plain": [".usfm", ".sfm", ".SFM", ".USFM"] } }],
    });
    if (!chosen) return null;
    return chosen.getFile();
  } catch {
    // The picker throws on cancel, which is not an error.
    return null;
  }
}

async function saveWithHandle(name: string): Promise<FileSystemFileHandle | null> {
  try {
    const picker = window.showSaveFilePicker;
    if (!picker) return null;
    return await picker({
      suggestedName: name,
      types: [{ description: "USFM", accept: { "text/plain": [".usfm", ".sfm"] } }],
    });
  } catch {
    return null;
  }
}

/**
 * The fallback opener: an `<input type="file">`, clicked programmatically.
 *
 * There is no cancel event, so the promise settles on the first of `change`
 * (a file was chosen) and the window regaining focus with nothing chosen.
 * Without the second, cancelling would leave the caller awaiting forever.
 */
function pickWithInput(): Promise<File | null> {
  return new Promise((settle) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".usfm,.sfm,.SFM,.USFM,text/plain";
    input.style.display = "none";

    let done = false;
    const finish = (file: File | null) => {
      if (done) return;
      done = true;
      input.remove();
      window.removeEventListener("focus", onFocus);
      settle(file);
    };

    // Deferred a tick past the focus event, because `change` fires after it.
    const onFocus = () => setTimeout(() => finish(input.files?.[0] ?? null), 300);

    input.addEventListener("change", () => finish(input.files?.[0] ?? null));
    window.addEventListener("focus", onFocus, { once: true });

    document.body.append(input);
    input.click();
  });
}

/** The fallback saver. Lands in the downloads folder, not the original file. */
function download(bytes: Uint8Array, name: string): void {
  // Typed as an octet stream rather than text/plain: a text type invites the
  // browser to help, and there is nothing here it could help with that would
  // not be a change to the bytes.
  const blob = new Blob([bytes as BlobPart], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);

  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();

  // Revoked on the next turn: revoking synchronously races the download in
  // some browsers, which then saves an empty file.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

function outcome(document: Editable, path: string, note: string): SaveOutcome {
  return {
    path,
    summary: {
      encoding: document.bom ? "UTF-8 with BOM" : "UTF-8",
      eol: document.eols[0] ?? "lf",
      bom: document.bom,
      final_newline: document.text.endsWith("\n"),
      mixed_eol: document.eols.some((eol) => eol !== document.eols[0]),
    },
    note,
    saved: true,
  };
}

function notSaved(document: Editable): SaveOutcome {
  return { ...outcome(document, document.path ?? "", ""), saved: false, note: null };
}
