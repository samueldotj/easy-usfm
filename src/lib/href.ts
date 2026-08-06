/**
 * What to do with a URL that came out of a document (SECURITY §2).
 *
 * USFM 3.0 put user-controlled URLs into Scripture files: `\jmp` carries
 * `link-href`, figures carry `src`, and `link-href`/`link-id` are generic on
 * character markers. These files are exchanged by email and USB and opened
 * without inspection, so a `javascript:` or `data:text/html` payload is a
 * realistic threat rather than a theoretical one — it is the only realistic
 * attack surface an offline editor has.
 *
 * # Three outcomes, not two
 *
 * The tempting design is "safe or dropped". Both halves would be wrong.
 * Dropping loses content, which ADR-003 forbids and which hides from the user
 * that their file contains something odd. And "safe" is not one thing: a
 * scripture reference is resolved in the application and must never be
 * navigated to at all, while an external link is navigated but only through
 * the operating system.
 */

/**
 * The schemes a link may use.
 *
 * An allowlist, and short. `javascript:` is the obvious exclusion; `data:` and
 * `blob:` matter as much, since `data:text/html` is a document with an origin
 * and `file:` would read the local disk. Anything not named here renders as
 * text.
 */
const SAFE_SCHEMES = new Set(["http:", "https:", "mailto:"]);

/**
 * A scripture reference, as USFM writes one in a link target.
 *
 * `GEN 1:1`, `1CO 13.4`. Book codes may begin with a digit, so the character
 * class allows it — the same trap `Reference::parse` has in the engine.
 */
const SCRIPTURE_REFERENCE = /^[A-Z1-9][A-Z0-9]{2}\s+\d+[:.]\d+/;

export type SafeHref =
  /** Resolved through Go to Reference. Never navigated. */
  | { kind: "ref"; value: string }
  /**
   * Opened through the operating system's handler after confirmation.
   *
   * Never in the webview: a link opening there is a link running in the
   * application's own origin, which is the whole thing being defended against.
   */
  | { kind: "external"; value: string }
  /**
   * Rendered as plain text with a warning, never as an anchor.
   *
   * The user sees exactly what the file contained without the application
   * acting on it — which is more useful than a removed link and is the only
   * outcome that tells them their document is odd.
   */
  | { kind: "inert"; value: string };

export function sanitizeHref(raw: string): SafeHref {
  const trimmed = raw.trim();

  // Before parsing as a URL, because `GEN 1:1` is not one and would come back
  // inert — which would make every internal reference in every file look like
  // a security warning.
  if (SCRIPTURE_REFERENCE.test(trimmed)) return { kind: "ref", value: trimmed };

  let url: URL;
  try {
    // `about:blank` as the base is what makes a relative URL fail rather than
    // resolve. Against the application's own origin, `//evil.test/x` and
    // `/etc/passwd` would both parse into something navigable.
    url = new URL(trimmed, "about:blank");
  } catch {
    return { kind: "inert", value: raw };
  }

  if (!SAFE_SCHEMES.has(url.protocol)) return { kind: "inert", value: raw };
  return { kind: "external", value: url.href };
}

/**
 * Whether a figure's source may be loaded at all (SECURITY §3).
 *
 * Images are off by default and local only, so this answers the narrower
 * question of whether a path is *shaped* like something loadable — traversal
 * and absolute paths rejected, remote schemes rejected outright so a document
 * cannot phone home and reveal that a particular file was opened.
 *
 * Decoded before checking, because `%2e%2e%2f` is `../` and a check that only
 * sees the raw form is a check that has been walked around.
 */
export function isLocalFigurePath(raw: string): boolean {
  const trimmed = raw.trim();
  if (trimmed === "") return false;

  let decoded = trimmed;
  for (let round = 0; round < 3; round += 1) {
    try {
      const next = decodeURIComponent(decoded);
      if (next === decoded) break;
      decoded = next;
    } catch {
      // Malformed percent-encoding. Refusing is the only safe reading: it
      // cannot be checked, so it cannot be cleared.
      return false;
    }
  }

  // Backslashes are separators on Windows, so a check that only knows about
  // `/` is one that `..\..\` walks straight through.
  const path = decoded.replaceAll("\\", "/");

  if (path.includes("://")) return false; // any scheme, not just http
  if (path.startsWith("/")) return false; // absolute
  if (/^[A-Za-z]:/.test(path)) return false; // drive-letter absolute
  if (path.split("/").includes("..")) return false; // traversal

  return true;
}
