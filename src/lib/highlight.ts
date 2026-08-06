/**
 * Syntax highlighting, from the engine's own token stream.
 *
 * The tokens come from the lexer (P2.6), not from a pattern over the text.
 * `\+bd` is a nested marker, `\bd*` closes one, `\qt-s` is a milestone and
 * `|src="x"` is an attribute run — distinctions a regular expression gets
 * subtly wrong at exactly the places USFM is interesting. Highlighting that
 * disagrees with the parser is worse than none: it teaches a model of the file
 * that the diagnostics then contradict.
 *
 * # Asynchronous highlighting
 *
 * The engine is in a worker, so tokens arrive after the keystroke that changed
 * them. Decorations are therefore held in a `StateField` and replaced when an
 * answer comes back, and they are *mapped* through any edits that happened
 * meanwhile — otherwise a token painted for the document as it was would land
 * a few characters off in the document as it is.
 */

import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";

import type { Token } from "../worker/protocol";

/** Tokens for a range have arrived. */
export const setTokens = StateEffect.define<{ from: number; to: number; tokens: Token[] }>();

const marks = new Map<string, Decoration>();

function markFor(className: string): Decoration {
  let mark = marks.get(className);
  if (!mark) {
    mark = Decoration.mark({ class: className });
    marks.set(className, mark);
  }
  return mark;
}

export const highlighting: StateField<DecorationSet> = StateField.define<DecorationSet>({
  create: () => Decoration.none,

  update(decorations, transaction) {
    // Mapped through the edit first. A decoration that is not moved with the
    // text it describes drifts, and the drift is invisible until someone
    // types near the start of a long line.
    decorations = decorations.map(transaction.changes);

    for (const effect of transaction.effects) {
      if (!effect.is(setTokens)) continue;

      const { from, to, tokens } = effect.value;
      const length = transaction.state.doc.length;

      const built = tokens
        // The answer describes the document as it was when asked. Anything
        // now out of bounds belongs to text that has since been deleted.
        .filter((token) => token.end <= length && token.start < token.end)
        .map((token) => markFor(token.class).range(token.start, token.end));

      decorations = decorations.update({
        // Only the range that was asked about is replaced, so highlighting
        // outside the viewport survives a scroll back to it.
        filter: (start, end) => end <= Math.min(from, length) || start >= Math.min(to, length),
        add: built,
        sort: true,
      });
    }

    return decorations;
  },

  provide: (field) => EditorView.decorations.from(field),
});

/**
 * Asks for tokens whenever the viewport moves or the document changes.
 *
 * `request` is called with a range; the answer comes back as a {@link setTokens}
 * effect. Debounced by the caller, not here — this only decides *what* to ask
 * for.
 */
export function tokenRequests(request: (from: number, to: number) => void): Extension {
  return EditorView.updateListener.of((update) => {
    if (!update.docChanged && !update.viewportChanged) return;

    // One screen of overscan either side, so scrolling does not reveal
    // unhighlighted text before the answer arrives.
    const { from, to } = update.view.viewport;
    const overscan = to - from;
    const length = update.state.doc.length;

    request(Math.max(0, from - overscan), Math.min(length, to + overscan));
  });
}
