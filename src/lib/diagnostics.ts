/**
 * Diagnostics in the editor: underlines, gutter glyphs, and F8 navigation.
 *
 * # Why the offsets line up in Tamil
 *
 * The acceptance criterion for this item is that diagnostics land on the right
 * characters in mixed-script text, and the reason they do is that nothing here
 * converts anything. The engine reports Char16 (UNICODE §1), CodeMirror counts
 * in Char16, so the numbers are already in the same space — the one conversion
 * happens in the WASM boundary and nowhere else. A diagnostic pointing at a
 * Hebrew word is off by the number of non-ASCII characters before it the
 * moment any layer here decides to be clever about byte offsets, and it is off
 * by zero on the English fixtures that would be written to test it.
 *
 * # Why they are mapped rather than repainted
 *
 * The engine is in a worker, so a diagnostic describes the document as it was
 * a round trip ago. Positions are mapped through every edit in the meantime,
 * for the same reason highlighting is: a decoration that does not move with
 * the text it describes drifts, and the drift is invisible until someone types
 * near the start of a long line.
 */

import {
  RangeSet,
  StateEffect,
  StateField,
  findClusterBreak,
  type ChangeDesc,
  type EditorState,
  type Extension,
  type Range,
  type TransactionSpec,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  GutterMarker,
  gutter,
  type Command,
  type DecorationSet,
} from "@codemirror/view";

import type { Diagnostic } from "../worker/protocol";

type Severity = Diagnostic["severity"];

/** A parse result has arrived. Replaces everything; diagnostics are not additive. */
export const setDiagnostics = StateEffect.define<readonly Diagnostic[]>();

/** Worst first, so a line showing one glyph shows the worst thing on it. */
const RANK: Record<Severity, number> = { error: 2, warning: 1, information: 0 };

/**
 * A distinct *shape* per severity, not a colour.
 *
 * PRODUCT §10: diagnostics use an underline plus a gutter glyph, never hue
 * alone. Three geometric shapes rather than three tints of the same mark, so
 * the gutter still says which is which in greyscale and to a colour-blind
 * reader. Kept to Geometric Shapes, which every platform font covers — a
 * prettier glyph that renders as tofu on Linux is not a better glyph.
 */
const GLYPH: Record<Severity, string> = { error: "✕", warning: "▲", information: "●" };

const LABEL: Record<Severity, string> = {
  error: "Error",
  warning: "Warning",
  information: "Information",
};

// ------------------------------------------------------------- decoration ---

/**
 * The underline for one diagnostic.
 *
 * The index rides along in the decoration's spec so the panel can ask where a
 * diagnostic *ended up* rather than where the engine last said it was. The
 * spec survives mapping, which makes this the only handle that stays valid
 * across the edits between one parse and the next.
 */
function underlineFor(diagnostic: Diagnostic, index: number): Decoration {
  return Decoration.mark({
    class: `cm-diagnostic-underline cm-diagnostic-${diagnostic.severity}`,
    index,
  });
}

class SeverityMarker extends GutterMarker {
  constructor(readonly severity: Severity) {
    super();
  }

  eq(other: GutterMarker): boolean {
    return other instanceof SeverityMarker && other.severity === this.severity;
  }

  toDOM(): Node {
    const glyph = document.createElement("span");
    glyph.className = `cm-diagnostic-glyph cm-diagnostic-${this.severity}`;
    glyph.textContent = GLYPH[this.severity];
    glyph.title = LABEL[this.severity];
    // The panel is the accessible surface for diagnostics. A screen reader
    // announcing a glyph beside every line makes the document unreadable,
    // which is the failure PRODUCT §10 means by "technically passes".
    glyph.setAttribute("aria-hidden", "true");
    return glyph;
  }
}

const MARKERS: Record<Severity, SeverityMarker> = {
  error: new SeverityMarker("error"),
  warning: new SeverityMarker("warning"),
  information: new SeverityMarker("information"),
};

/**
 * Where a diagnostic's underline actually goes.
 *
 * A zero-width diagnostic — something *absent* rather than wrong, like a
 * missing `\id` — has nothing to underline, and CodeMirror refuses an empty
 * mark outright. It is widened by one grapheme cluster: forward within the
 * line, or backward when there is nothing ahead of it.
 *
 * A cluster and not a code unit. Widening by one UTF-16 unit in Devanagari
 * underlines half a conjunct and in an emoji underlines half a surrogate pair
 * — interaction happens in grapheme space (UNICODE §3), and this is
 * interaction.
 *
 * `null` means there is nothing to mark at all, on an empty line. The gutter
 * glyph and the panel still show it; only the underline is skipped.
 */
function place(state: EditorState, start: number, end: number): { from: number; to: number } | null {
  const length = state.doc.length;
  const from = Math.max(0, Math.min(start, length));
  const to = Math.max(from, Math.min(end, length));
  if (to > from) return { from, to };

  const line = state.doc.lineAt(from);
  const at = from - line.from;

  const forward = findClusterBreak(line.text, at, true);
  if (forward > at) return { from, to: line.from + forward };

  const back = findClusterBreak(line.text, at, false);
  if (back < at) return { from: line.from + back, to: from };

  return null;
}

/** Everything the editor holds about diagnostics, in one mappable value. */
class Placed {
  static readonly empty = new Placed(Decoration.none, RangeSet.empty);

  constructor(
    readonly underlines: DecorationSet,
    readonly glyphs: RangeSet<GutterMarker>,
  ) {}

  map(changes: ChangeDesc): Placed {
    // Both are range sets, so this is the cheap structural remap rather than a
    // rebuild — which matters because it happens on every keystroke.
    return new Placed(this.underlines.map(changes), this.glyphs.map(changes));
  }

  static build(state: EditorState, diagnostics: readonly Diagnostic[]): Placed {
    const underlines: Range<Decoration>[] = [];
    /** Line start to the worst severity on it. One glyph per line. */
    const worst = new Map<number, Severity>();

    diagnostics.forEach((diagnostic, index) => {
      // Past the end describes text that has since been deleted. The next
      // parse will say so; until then there is nothing to point at.
      if (diagnostic.start > state.doc.length) return;

      const range = place(state, diagnostic.start, diagnostic.end);
      if (range) {
        underlines.push(underlineFor(diagnostic, index).range(range.from, range.to));
      }

      const line = state.doc.lineAt(Math.min(diagnostic.start, state.doc.length)).from;
      const held = worst.get(line);
      if (held === undefined || RANK[diagnostic.severity] > RANK[held]) {
        worst.set(line, diagnostic.severity);
      }
    });

    const glyphs = [...worst]
      .sort(([a], [b]) => a - b)
      .map(([line, severity]) => MARKERS[severity].range(line));

    // Sorted rather than assumed sorted: the engine emits diagnostics in the
    // order it finds them, which is not document order once more than one pass
    // contributes, and a range set built out of order throws.
    return new Placed(Decoration.set(underlines, true), RangeSet.of(glyphs, true));
  }
}

const placed = StateField.define<Placed>({
  create: () => Placed.empty,

  update(value, transaction) {
    value = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setDiagnostics)) value = Placed.build(transaction.state, effect.value);
    }
    return value;
  },
});

/**
 * Which entry F8 last landed on.
 *
 * Position alone cannot answer that. Two diagnostics routinely cover exactly
 * the same characters — an unclosed `\bd` is both "missing its close" and
 * "spanning a verse boundary", reported over the same run — so "the entry
 * whose range matches the selection" finds the first of them every time and
 * F8 alternates between two indistinguishable places forever.
 *
 * Cleared by any selection change that is not a step, because at that point
 * the user has moved and where the walk had got to is no longer where they
 * are.
 */
const setVisited = StateEffect.define<number>();

const visited = StateField.define<number | null>({
  create: () => null,

  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setVisited)) return effect.value;
    }
    return transaction.selection ? null : value;
  },
});

/** The whole feature, as one extension. */
export const diagnostics: Extension = [
  placed,
  visited,
  EditorView.decorations.from(placed, (value) => value.underlines),
  gutter({
    class: "cm-diagnostic-gutter",
    markers: (view) => view.state.field(placed).glyphs,
    // Gives the gutter its width up front, so the text does not shift sideways
    // the first time a diagnostic appears.
    initialSpacer: () => MARKERS.error,
  }),
];

// ------------------------------------------------------------- navigation ---

/** A from/to pair in the editor's coordinates. */
export interface Span {
  from: number;
  to: number;
}

/** A span, plus which entry in the walk it is — see {@link setVisited}. */
export interface Located extends Span {
  at: number;
}

/** Every underline currently placed, in document order. */
function walk(state: EditorState): Located[] {
  const set = state.field(placed, false)?.underlines;
  if (!set) return [];

  const all: Located[] = [];
  for (const iterator = set.iter(); iterator.value; iterator.next()) {
    all.push({ from: iterator.from, to: iterator.to, at: all.length });
  }
  return all;
}

/**
 * Where the next diagnostic after the cursor is, or the previous one, wrapping.
 *
 * PRODUCT §6.4 binds these to F8 and Shift+F8. Wrapping rather than stopping
 * at the ends: the point of the key is to walk the whole list without having
 * to know where in it you are.
 *
 * A query over the state rather than a command, so the interesting half — the
 * wrap, and which diagnostic counts as "after" — can be tested without a DOM.
 */
export function nextDiagnostic(state: EditorState, forward: boolean): Located | null {
  const all = walk(state);
  if (all.length === 0) return null;

  const { anchor, head } = state.selection.main;
  const from = Math.min(anchor, head);
  const to = Math.max(anchor, head);

  // Where the walk had got to, if the selection is still sitting on it.
  const remembered = state.field(visited, false) ?? null;
  const held = remembered === null ? undefined : all[remembered];
  const standing =
    held && held.from === from && held.to === to
      ? held.at
      : all.findIndex((range) => range.from === from && range.to === to);

  // Standing on one, so step to its neighbour *in the list*. Position is not
  // enough here either: USFM diagnostics nest heavily — an unclosed `\bd`
  // spans everything after it, swallowing every diagnostic inside — and "the
  // next one starting after the cursor" silently skips all of them. The list
  // is in document order, so stepping through it visits each exactly once.
  if (standing >= 0) {
    return all[(standing + (forward ? 1 : -1) + all.length) % all.length] ?? null;
  }

  // Otherwise the cursor is just somewhere in the text: the nearest whole run
  // in the direction asked for. Measured from opposite ends, so a cursor
  // *inside* a diagnostic steps out of it either way rather than sticking.
  if (forward) return all.find((range) => range.from > head) ?? all[0] ?? null;

  for (let index = all.length - 1; index >= 0; index -= 1) {
    const range = all[index];
    if (range && range.to < head) return range;
  }
  return all[all.length - 1] ?? null;
}

/**
 * Where a diagnostic *is now*, given the index the panel is showing.
 *
 * Found by the index carried in the decoration's spec rather than by the
 * offsets the engine reported, because those differ by every edit made since
 * the last parse.
 */
export function diagnosticRange(state: EditorState, index: number): Located | null {
  const set = state.field(placed, false)?.underlines;
  if (!set) return null;

  let at = 0;
  for (const iterator = set.iter(); iterator.value; iterator.next(), at += 1) {
    if (iterator.value.spec.index === index) {
      return { from: iterator.from, to: iterator.to, at };
    }
  }
  return null;
}

/**
 * The transaction that moves to `target`, or `null` if there is nowhere to go.
 *
 * Separated from the dispatch so the whole step — including the walk position
 * it records — is a function of the state, and can be exercised in a test
 * without a view.
 */
function stepTo(target: Located | null): TransactionSpec | null {
  if (!target) return null;
  return {
    selection: { anchor: target.from, head: target.to },
    scrollIntoView: true,
    // Recorded so the next step knows which of several identical ranges this
    // one was.
    effects: setVisited.of(target.at),
  };
}

/** F8 and Shift+F8, as a transaction. */
export function stepSpec(state: EditorState, forward: boolean): TransactionSpec | null {
  return stepTo(nextDiagnostic(state, forward));
}

/** The panel's jump, as a transaction. */
export function revealSpec(state: EditorState, index: number): TransactionSpec | null {
  return stepTo(diagnosticRange(state, index));
}

function dispatch(view: EditorView, spec: TransactionSpec | null, focus: boolean): boolean {
  if (!spec) return false;
  view.dispatch(spec);
  // Not animated, so `prefers-reduced-motion` has nothing to disable here.
  if (focus) view.focus();
  return true;
}

/** F8 and Shift+F8. */
export function stepDiagnostic(forward: boolean): Command {
  return (view) => dispatch(view, stepSpec(view.state, forward), true);
}

/**
 * The panel asking to go to one of them.
 *
 * `focus` is false while the panel is being browsed: moving focus into the
 * editor on every arrow-key press would make the list impossible to walk.
 */
export function revealDiagnostic(view: EditorView, index: number, focus: boolean): boolean {
  return dispatch(view, revealSpec(view.state, index), focus);
}
