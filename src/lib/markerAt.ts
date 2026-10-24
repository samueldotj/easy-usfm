/**
 * Which marker the caret is in.
 *
 * The inspector's marker tab answers "what am I inside?", which is a question
 * with a surprisingly precise answer: USFM's character markers nest inside the
 * paragraph marker that begins the line, and a character span that has already
 * closed is not somewhere the caret can be.
 *
 * So the rule is: the innermost span still open at the caret, and the line's
 * paragraph marker when none is. That is one line of text to look at, which is
 * why this can run on every cursor move without anyone noticing.
 *
 * A note on what this deliberately is not: it is not a parser. The engine
 * knows the document's structure and this does not try to. It answers a
 * question about one line for the benefit of a help panel, and when it is
 * wrong the cost is a paragraph of prose about the wrong marker.
 */

/** A marker name, and its closing star if it had one. */
const MARKERS = /\\(\+?[a-z]+[0-9]*)(\*?)/g;

export function markerAt(text: string, offset: number): string | null {
  const at = Math.max(0, Math.min(offset, text.length));
  const lineStart = text.lastIndexOf("\n", at - 1) + 1;
  const before = text.slice(lineStart, at);

  const found = [...before.matchAll(MARKERS)];

  for (let index = found.length - 1; index >= 0; index -= 1) {
    const match = found[index];
    if (!match) continue;

    const [whole, name = "", star = ""] = match;
    // A closing marker is the end of something, not somewhere to be.
    if (star === "*") continue;

    // Nested character markers are written `\+nd`; the plus is syntax for the
    // nesting, not part of the marker's name.
    const bare = name.replace(/^\+/, "");

    // An opening marker whose partner is already behind the caret has closed.
    const after = (match.index ?? 0) + whole.length;
    if (before.includes(`\\${name}*`, after)) continue;

    return bare;
  }

  return null;
}
