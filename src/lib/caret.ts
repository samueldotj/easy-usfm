/**
 * Where in a run of text a click landed — P3.6.
 *
 * The acceptance criterion is that a click "lands on the right character across
 * conjuncts and reordered vowel signs", and the way to get that right is to not
 * do it. Measuring character widths and walking left to right fails on both:
 * a conjunct is several code points drawn as one glyph, and a Devanagari
 * i-matra is stored after the consonant and drawn before it, so the character
 * under the pointer is not the one at that position in the string.
 *
 * The browser already knows, because it is the thing that laid the text out and
 * it has to answer this question every time someone clicks in a text field. So
 * this module asks it, and does nothing else.
 *
 * # Two spellings of the same question
 *
 * `caretPositionFromPoint` is the standard. WebKit ships only
 * `caretRangeFromPoint`, and WebKit is the webview on macOS and Linux — two of
 * the three platforms this runs on. Both are tried, and neither is assumed.
 */

/** The standard shape, which TypeScript's DOM library does not always carry. */
interface CaretPosition {
  offsetNode: Node;
  offset: number;
}

/**
 * What is actually reachable on `document`, whatever the library says.
 *
 * Declared as its own shape rather than as an extension of `Document`, because
 * the library's own declarations of these two disagree between TypeScript
 * versions — and the point here is to survive their *absence* at runtime, which
 * no compile-time declaration can promise.
 */
interface CaretCapableDocument {
  caretPositionFromPoint?: (x: number, y: number) => CaretPosition | null;
  caretRangeFromPoint?: (x: number, y: number) => Range | null;
}

/**
 * The text node and offset under a point, or `null`.
 *
 * The offset is in UTF-16 code units and on a cluster boundary, because that is
 * what a caret position is — which is the same unit the engine reports spans
 * in, so the two compose without a conversion.
 */
export function caretAt(x: number, y: number): { node: Node; offset: number } | null {
  const owner = document as unknown as CaretCapableDocument;

  const position = owner.caretPositionFromPoint?.(x, y);
  if (position) return { node: position.offsetNode, offset: position.offset };

  const range = owner.caretRangeFromPoint?.(x, y);
  if (range) return { node: range.startContainer, offset: range.startOffset };

  return null;
}

/**
 * How far into `host`'s text a point falls, or `null` if it falls outside.
 *
 * Counted across every text node inside `host` rather than within the one that
 * was hit, because a run is not guaranteed to be a single DOM text node — a
 * framework is free to split it, and this has to stay true when one does.
 *
 * The containment check is what makes this safe to call from a handler on a
 * nested element: a click near a paragraph's edge can resolve to a caret in the
 * *next* paragraph, and adding that offset to this node's start would report a
 * position inside a run the user did not click.
 */
export function offsetWithin(host: HTMLElement, x: number, y: number): number | null {
  const caret = caretAt(x, y);
  if (!caret || !host.contains(caret.node)) return null;

  let counted = 0;
  const walker = document.createTreeWalker(host, NodeFilter.SHOW_TEXT);

  for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
    if (text === caret.node) return counted + caret.offset;
    counted += text.textContent?.length ?? 0;
  }

  // The caret is inside `host` but not in a text node — an empty element, or a
  // gap between them. There is no character to report.
  return null;
}
