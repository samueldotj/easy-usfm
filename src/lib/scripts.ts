/**
 * The scripts this editor is for, and what each one needs to be readable.
 *
 * UNICODE §7 states the requirements: a proportional script-appropriate
 * content font, a per-script size multiplier defaulting to 1.15 for scripts
 * with dense conjuncts or stacked marks, line height 1.7 where marks sit above
 * and below the baseline, and a notice when the font for a script is missing.
 *
 * # One table, three consumers
 *
 * The same rows drive the `@font-face` rules that apply the multiplier, the
 * line-height decision, and the missing-font notice. Splitting them would mean
 * three lists of scripts that have to agree, and the one that falls behind is
 * always the one nobody is looking at.
 *
 * Coverage is the corpus (ARCHITECTURE §12.4) rather than all of Unicode. A
 * script nobody has a translation in is a row that has never been checked.
 */

export interface Script {
  /** What the notice calls it. */
  readonly name: string;
  /** The Noto family that covers it, for the notice and the font stack. */
  readonly font: string;
  /**
   * CSS `unicode-range`, and the same ranges as pairs for testing text.
   *
   * Written once and derived, because a range list that disagrees with itself
   * would apply the size multiplier to one set of characters and the
   * missing-font check to another.
   */
  readonly ranges: readonly (readonly [number, number])[];
  /**
   * The size multiplier. 1 leaves the script alone.
   *
   * UNICODE §7: scripts with dense conjuncts or stacked marks need 110–120% of
   * a Latin face's size for equivalent legibility, because the meaningful
   * detail is in the marks — a virama, a nukta, a vowel sign — and at Latin
   * size those are a few pixels.
   */
  readonly scale: number;
  /**
   * Whether lines need the taller leading.
   *
   * True where marks stack both above and below the baseline. At Latin
   * leading, the descending mark of one line and the ascending mark of the
   * next collide, which is not a subtle degradation — it is unreadable.
   */
  readonly tall: boolean;
  /**
   * A printable base letter, for probing whether the script renders at all.
   *
   * Not derived from the first codepoint of the first range: that is often a
   * combining mark, which measures as zero width and cannot be probed.
   */
  readonly sample: string;
}

/**
 * The scripts, in no significant order.
 *
 * Latin and Cyrillic are absent deliberately: they need neither the multiplier
 * nor the leading, and a row for them would be a rule that does nothing except
 * invite someone to change it.
 */
export const SCRIPTS: readonly Script[] = [
  // ---- Indic. Conjuncts and stacked vowel signs throughout.
  { name: "Devanagari", font: "Noto Sans Devanagari", scale: 1.15, tall: true,
    sample: "क",
    ranges: [[0x0900, 0x097f], [0xa8e0, 0xa8ff]] },
  { name: "Bengali", font: "Noto Sans Bengali", scale: 1.15, tall: true,
    sample: "ক",
    ranges: [[0x0980, 0x09ff]] },
  { name: "Gurmukhi", font: "Noto Sans Gurmukhi", scale: 1.15, tall: true,
    sample: "ਕ",
    ranges: [[0x0a00, 0x0a7f]] },
  { name: "Gujarati", font: "Noto Sans Gujarati", scale: 1.15, tall: true,
    sample: "ક",
    ranges: [[0x0a80, 0x0aff]] },
  { name: "Oriya", font: "Noto Sans Oriya", scale: 1.15, tall: true,
    sample: "କ",
    ranges: [[0x0b00, 0x0b7f]] },
  // Tamil has fewer stacked forms than its neighbours but long vowel signs
  // that extend well past the em box.
  { name: "Tamil", font: "Noto Sans Tamil", scale: 1.15, tall: true,
    sample: "க",
    ranges: [[0x0b80, 0x0bff]] },
  { name: "Telugu", font: "Noto Sans Telugu", scale: 1.15, tall: true,
    sample: "క",
    ranges: [[0x0c00, 0x0c7f]] },
  { name: "Kannada", font: "Noto Sans Kannada", scale: 1.15, tall: true,
    sample: "ಕ",
    ranges: [[0x0c80, 0x0cff]] },
  { name: "Malayalam", font: "Noto Sans Malayalam", scale: 1.15, tall: true,
    sample: "ക",
    ranges: [[0x0d00, 0x0d7f]] },
  { name: "Sinhala", font: "Noto Sans Sinhala", scale: 1.15, tall: true,
    sample: "ක",
    ranges: [[0x0d80, 0x0dff]] },

  // ---- Other complex scripts.
  { name: "Thai", font: "Noto Sans Thai", scale: 1.15, tall: true,
    sample: "ก",
    ranges: [[0x0e00, 0x0e7f]] },
  { name: "Lao", font: "Noto Sans Lao", scale: 1.15, tall: true,
    sample: "ກ",
    ranges: [[0x0e80, 0x0eff]] },
  { name: "Tibetan", font: "Noto Serif Tibetan", scale: 1.15, tall: true,
    sample: "ཀ",
    ranges: [[0x0f00, 0x0fff]] },
  { name: "Myanmar", font: "Noto Sans Myanmar", scale: 1.15, tall: true,
    sample: "က",
    ranges: [[0x1000, 0x109f]] },
  { name: "Khmer", font: "Noto Sans Khmer", scale: 1.15, tall: true,
    sample: "ក",
    ranges: [[0x1780, 0x17ff]] },
  { name: "Ethiopic", font: "Noto Sans Ethiopic", scale: 1, tall: false,
    sample: "ሀ",
    ranges: [[0x1200, 0x137f]] },

  // ---- Right to left. Pointed Hebrew and vocalised Arabic stack marks.
  { name: "Hebrew", font: "Noto Sans Hebrew", scale: 1.1, tall: true,
    sample: "א",
    ranges: [[0x0590, 0x05ff], [0xfb1d, 0xfb4f]] },
  { name: "Arabic", font: "Noto Sans Arabic", scale: 1.1, tall: true,
    sample: "ا",
    ranges: [[0x0600, 0x06ff], [0x0750, 0x077f], [0xfb50, 0xfdff], [0xfe70, 0xfeff]] },
  { name: "Syriac", font: "Noto Sans Syriac", scale: 1.1, tall: true,
    sample: "ܐ",
    ranges: [[0x0700, 0x074f]] },

  // ---- Scripts that need the face but not the metrics.
  { name: "Greek", font: "Noto Sans", scale: 1, tall: false,
    sample: "α",
    ranges: [[0x0370, 0x03ff], [0x1f00, 0x1fff]] },
  { name: "Coptic", font: "Noto Sans Coptic", scale: 1, tall: false,
    sample: "Ⲁ",
    ranges: [[0x2c80, 0x2cff]] },
  { name: "Han", font: "Noto Sans SC", scale: 1, tall: false,
    sample: "一",
    ranges: [[0x3400, 0x4dbf], [0x4e00, 0x9fff], [0xf900, 0xfaff]] },
];

const hex = (code: number) => code.toString(16).toUpperCase().padStart(4, "0");

/**
 * A script's ranges as a CSS `unicode-range` value.
 *
 * `U+0B80-0BFF`: the prefix appears once, on the start. Repeating it on the
 * end is not the syntax, and a rule with a malformed `unicode-range` is
 * dropped in full -- so the face would simply never apply, with nothing said.
 */
export function unicodeRange(script: Script): string {
  return script.ranges.map(([from, to]) => `U+${hex(from)}-${hex(to)}`).join(", ");
}

/** Whether a code point falls in this script. */
function covers(script: Script, code: number): boolean {
  return script.ranges.some(([from, to]) => code >= from && code <= to);
}

/**
 * Which scripts a document actually uses.
 *
 * Every character, not a sample. Sampling was the first attempt and it is
 * wrong for this question: the thing being looked for is *presence*, and a
 * script used once in a large file is still a script the reader will meet — so
 * stepping through the text misses exactly the case the notice exists for.
 *
 * The cost is a linear pass on open, which is nothing beside the parse that is
 * about to happen anyway. Most of it is the first comparison, since below
 * U+0370 no row can match and that is nearly all of a USFM file. It stops
 * early once every script is accounted for.
 */
export function scriptsIn(text: string): Script[] {
  const found = new Set<Script>();

  // Iterated by code point rather than by index: stepping through UTF-16 units
  // would hand a lone low surrogate to the range test, which is a codepoint
  // nobody's script claims.
  for (const character of text) {
    const code = character.codePointAt(0);
    if (code === undefined || code < 0x0370) continue;

    for (const script of SCRIPTS) {
      if (!found.has(script) && covers(script, code)) {
        found.add(script);
        if (found.size === SCRIPTS.length) return [...SCRIPTS];
      }
    }
  }

  return SCRIPTS.filter((script) => found.has(script));
}

/**
 * The line height a document needs.
 *
 * One value for the whole document, because line height is a block property —
 * there is no per-run equivalent of the size multiplier. So the question is
 * which mistake to make in a mixed file, and the answer is the roomier one: a
 * Latin paragraph set at 1.7 is slightly airy, and a Devanagari one set at 1.5
 * has marks from adjacent lines touching.
 */
export function lineHeightFor(scripts: readonly Script[]): number {
  return scripts.some((script) => script.tall) ? 1.7 : 1.5;
}
