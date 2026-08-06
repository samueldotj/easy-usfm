/**
 * Zero-width characters, made visible (P3.12).
 *
 * UNICODE's appendix: joiners "change rendering, carry meaning, are invisible,
 * and are trivially inserted or deleted by accident". An editor that shows
 * nothing where one sits is an editor in which the most confusing class of
 * defect in these scripts cannot be seen at all — the file looks right, the
 * diagnostic looks wrong, and there is nothing on screen to reconcile them.
 *
 * # Which characters, and why not more
 *
 * The joiners, and the bidirectional controls. Both are invisible and both
 * change how the text around them renders, which is the pair of properties
 * that makes a character worth marking. Ordinary spaces are not here: a
 * document is mostly spaces, and marking them turns the editor into a field of
 * dots for no gain in a format where whitespace is not significant.
 */

import { RangeSetBuilder, StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";

/**
 * What each marked character is called and shown as.
 *
 * The label is what a translator can search for and repeat to a colleague;
 * "an invisible character" is not actionable and "•" is not either.
 */
const INVISIBLE = new Map<string, { glyph: string; name: string }>([
  ["​", { glyph: "ZWSP", name: "U+200B zero-width space" }],
  ["‌", { glyph: "ZWNJ", name: "U+200C zero-width non-joiner" }],
  ["‍", { glyph: "ZWJ", name: "U+200D zero-width joiner" }],
  ["⁠", { glyph: "WJ", name: "U+2060 word joiner" }],
  ["﻿", { glyph: "BOM", name: "U+FEFF zero-width no-break space" }],
  // Bidirectional controls. Invisible, and they reorder everything after them
  // — a stray one is a line that reads backwards with no visible cause.
  ["‎", { glyph: "LRM", name: "U+200E left-to-right mark" }],
  ["‏", { glyph: "RLM", name: "U+200F right-to-left mark" }],
  ["⁦", { glyph: "LRI", name: "U+2066 left-to-right isolate" }],
  ["⁧", { glyph: "RLI", name: "U+2067 right-to-left isolate" }],
  ["⁨", { glyph: "FSI", name: "U+2068 first-strong isolate" }],
  ["⁩", { glyph: "PDI", name: "U+2069 pop directional isolate" }],
  ["‪", { glyph: "LRE", name: "U+202A left-to-right embedding" }],
  ["‫", { glyph: "RLE", name: "U+202B right-to-left embedding" }],
  ["‬", { glyph: "PDF", name: "U+202C pop directional formatting" }],
  ["‭", { glyph: "LRO", name: "U+202D left-to-right override" }],
  ["‮", { glyph: "RLO", name: "U+202E right-to-left override" }],
]);

const PATTERN = new RegExp(`[${[...INVISIBLE.keys()].join("")}]`, "gu");

/** Whether a document contains anything this would mark. */
export function hasInvisibles(text: string): boolean {
  PATTERN.lastIndex = 0;
  return PATTERN.test(text);
}

/**
 * The mark itself.
 *
 * A widget beside the character rather than a replacement for it. Replacing
 * would change what the document *is* as far as every offset is concerned, and
 * the character has to stay exactly where it is — this is an editor whose
 * entire premise is byte fidelity.
 */
class InvisibleMark extends WidgetType {
  constructor(
    readonly glyph: string,
    readonly name: string,
  ) {
    super();
  }

  eq(other: InvisibleMark): boolean {
    return other.glyph === this.glyph;
  }

  toDOM(): HTMLElement {
    const mark = document.createElement("span");
    mark.className = "cm-invisible";
    mark.textContent = this.glyph;
    mark.title = this.name;
    // The character it marks is already in the text; a screen reader
    // announcing the label too would read every joiner twice.
    mark.setAttribute("aria-hidden", "true");
    return mark;
  }

  /** Not part of the document, so the cursor must not stop inside it. */
  ignoreEvent(): boolean {
    return false;
  }
}

function build(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();

  // The visible ranges only. A two-megabyte file has no reason to be scanned
  // for this on every update, and nothing off screen can be seen anyway.
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to);
    PATTERN.lastIndex = 0;

    let match: RegExpExecArray | null;
    while ((match = PATTERN.exec(text)) !== null) {
      const found = INVISIBLE.get(match[0]);
      if (!found) continue;

      const at = from + match.index + match[0].length;
      builder.add(
        at,
        at,
        Decoration.widget({ widget: new InvisibleMark(found.glyph, found.name), side: 1 }),
      );
    }
  }

  return builder.finish();
}

/** The setting changed. */
export const setShowInvisibles = StateEffect.define<boolean>();

/**
 * Whether marks are shown, as editor state.
 *
 * A field rather than a closure over a reactive variable, which was the first
 * attempt and does nothing: a view plugin is only asked to update when the
 * document, the viewport or the geometry changes, and toggling a setting is
 * none of those. The plugin sat there with its old answer and the marks never
 * appeared.
 */
const showing = StateField.define<boolean>({
  create: () => false,
  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setShowInvisibles)) return effect.value;
    }
    return value;
  },
});

/**
 * Shows invisible characters (UNICODE appendix, P3.12).
 *
 * Off until something says otherwise. The setting is per document — "defaulting
 * to on when the document's script uses them" is a property of the file rather
 * than of the application — so the interface decides and dispatches.
 */
export const invisibles: Extension = [
  showing,
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = view.state.field(showing) ? build(view) : Decoration.none;
      }

      update(update: ViewUpdate) {
        const was = update.startState.field(showing);
        const now = update.state.field(showing);

        // Rebuilt rather than mapped. These are point widgets keyed to
        // characters, and a character can be typed or deleted anywhere; the
        // scan is over the viewport, so rebuilding costs a screenful.
        if (was !== now || update.docChanged || update.viewportChanged || update.geometryChanged) {
          this.decorations = now ? build(update.view) : Decoration.none;
        }
      }
    },
    { decorations: (plugin) => plugin.decorations },
  ),
];
