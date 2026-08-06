/**
 * Marker autocomplete (P2.10).
 *
 * PRODUCT §6: "Typing `\` opens the marker list, ranked by validity in
 * context, then frequency in the document, then alphabetically. Deprecated
 * markers are greyed with their replacement shown and never ranked first."
 *
 * The ranking is the engine's — it needs the marker table, the parse tree at
 * the cursor, and a count over the whole document. This file asks once per
 * backslash and then lets CodeMirror filter as the name is typed, so holding
 * down a key does not mean a round trip per character.
 *
 * # Why the order has to be forced
 *
 * CodeMirror scores completions itself, by how well each matches what has been
 * typed, and that score would override everything the engine decided. So each
 * offer carries an explicit `boost` derived from its rank. Boost is a small
 * signed number, so the rank is mapped into it rather than used directly; what
 * matters is that the relative order survives, and it does because the mapping
 * is monotonic.
 */

import {
  type Completion as CodeMirrorCompletion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";

import type { Completion } from "../worker/protocol";

/**
 * What a marker name may contain.
 *
 * Letters, digits and `-` (milestones are `qt-s`), plus a leading `+` for the
 * nesting form. Deliberately not `\w`: a marker name is ASCII, and matching
 * Unicode letters here would swallow the Scripture after the backslash.
 */
const NAME = /\\\+?[A-Za-z0-9-]*$/;

/**
 * The highest boost CodeMirror will honour, and the lowest.
 *
 * The scale is small and the list is 300-odd long, so rank cannot map onto it
 * one for one. What the mapping has to preserve is the *order*, and a linear
 * squeeze does that.
 */
const BOOST_RANGE = 198;

/**
 * A completion, plus the one fact the list has to render differently.
 *
 * Carried on the option rather than in a lookup table because CodeMirror hands
 * the very same object back to {@link optionClass}, and a second structure
 * keyed on it would be one more thing to keep in step.
 */
type MarkerOption = CodeMirrorCompletion & { deprecated: boolean };

/**
 * The class for one row.
 *
 * A separate function because this is how CodeMirror lets a row be styled at
 * all: `Completion` has no `class` field, and `type` only reaches the DOM
 * through the icon element — which is off, since a sprite sheet of generic
 * shapes says nothing about USFM's marker classes that the detail line does
 * not say in words.
 */
export function optionClass(completion: CodeMirrorCompletion): string {
  return (completion as MarkerOption).deprecated ? "cm-usfm-deprecated" : "";
}

function decorate(offer: Completion, rank: number, total: number): MarkerOption {
  return {
    deprecated: offer.deprecated_in !== null,
    label: `\\${offer.marker}`,
    detail: offer.detail,
    // The engine's order, expressed in the only currency CodeMirror ranks in.
    boost: 99 - Math.round((rank / Math.max(1, total - 1)) * BOOST_RANGE),
    apply: (view, _completion, from, to) => {
      view.dispatch({
        changes: { from, to, insert: `\\${offer.insert}` },
        // Past the backslash and into the marker's content, which is where the
        // next thing typed belongs.
        selection: { anchor: from + 1 + offer.caret },
      });
    },
  };
}

/**
 * Asks the engine for the marker list.
 *
 * `ask` takes the offset of the backslash, because what is valid depends on
 * where the marker starts and by the time the caret has moved the user may
 * have typed half a name.
 */
export function markerCompletions(ask: (at: number) => Promise<Completion[]>) {
  return async (context: CompletionContext): Promise<CompletionResult | null> => {
    const match = context.matchBefore(NAME);
    if (!match) return null;

    // An explicit request on a bare backslash still opens the list; an
    // automatic one waits until there is a backslash to complete, which there
    // always is by the time `match` succeeded.
    if (!context.explicit && match.from === match.to) return null;

    const offers = await ask(match.from);
    if (context.aborted || offers.length === 0) return null;

    return {
      from: match.from,
      options: offers.map((offer, rank) => decorate(offer, rank, offers.length)),
      // The list is the same until the backslash is left behind, so CodeMirror
      // filters what it has rather than asking again per keystroke.
      validFor: NAME,
    };
  };
}
