/**
 * What the reference page says about a marker.
 *
 * # Examples are generated, not written
 *
 * There are 335 markers. Three hundred and thirty-five hand-written examples
 * would be wrong within a release — someone adds a marker to `markers.toml`,
 * the page keeps its old list, and the reference now disagrees with the parser
 * about what the editor accepts. So the *syntax* comes from the same table the
 * parser uses: whether a marker closes, what attributes it takes, whether it
 * begins a line. That is exactly the part a reader needs and exactly the part
 * that can be derived.
 *
 * # Descriptions are by family, and are honest about it
 *
 * Prose cannot be derived. Writing 335 descriptions would mean inventing most
 * of them, and a reference that confidently describes a marker wrongly is worse
 * than one that says nothing — the reader cannot tell which entries were
 * guessed.
 *
 * So descriptions are keyed by family, matched longest-prefix-first: every
 * `\q1`…`\q9` is a poetry line, every `\io1`…`\io9` an introduction outline
 * entry. A marker with no family gets its class and its syntax and no
 * sentence, which is the truthful outcome. The specification is linked for the
 * rest.
 */

/** One row of the engine's marker table. */
export interface MarkerRow {
  marker: string;
  class: string;
  closing: string;
  nests_under: string[];
  attributes: string[];
  default_attr: string | null;
  since: string | null;
  deprecated_in: string | null;
  replacement: string | null;
  publishable: boolean;
}

/** What the page shows for one marker. */
export interface MarkerHelp extends MarkerRow {
  /** A worked example, generated from the row. */
  example: string;
  /** A sentence, where the family is known. */
  description: string | null;
  /** The family it was described by, so the page can group. */
  family: string | null;
}

/**
 * Families, longest prefix first.
 *
 * Ordered by length at lookup rather than here, so adding one cannot break
 * another by sitting in the wrong place — `\io` must not claim `\iot`, and the
 * only reliable rule is that the longer name wins.
 */
const FAMILIES: Record<string, string> = {
  id: "Identifies the book. Required, and the first marker in the file.",
  usfm: "Declares which version of USFM the file is written in.",
  ide: "Records the character encoding the file was saved in. Obsolete; files are UTF-8.",
  h: "The running header — the book name printed at the top of a page.",
  toc: "A table-of-contents entry: long name, short name, and abbreviation.",
  toca: "The alternative-language table-of-contents entries.",
  mt: "Major title, the book's heading. Numbered for levels.",
  mte: "Major title at the end of the book.",
  ms: "A major section heading, above the ordinary section headings.",
  mr: "The reference range covered by a major section.",
  s: "A section heading. Numbered for levels.",
  sr: "The reference range covered by a section.",
  r: "A parallel-passage reference, printed under the heading.",
  d: "A descriptive title, as used for the Psalm ascriptions.",
  sp: "A speaker's name, as in Job and the Song of Songs.",
  c: "Begins a chapter. Takes the chapter number.",
  ca: "An alternative chapter number, for versification differences.",
  cp: "A published chapter number, printed instead of the real one.",
  cl: "The word used for “Chapter”, either once or per chapter.",
  cd: "A chapter description, printed under the chapter number.",
  v: "Begins a verse. Takes the verse number, and may be a range.",
  va: "An alternative verse number, for versification differences.",
  vp: "A published verse number, printed instead of the real one.",
  p: "An ordinary paragraph of prose.",
  m: "A paragraph continuing without its first-line indent.",
  po: "A paragraph opening a letter, as in the epistles.",
  pr: "A right-aligned paragraph.",
  cls: "A closing paragraph, as at the end of a letter.",
  pmo: "An embedded-text opening paragraph.",
  pm: "An embedded text paragraph, such as a quoted letter.",
  pmc: "An embedded-text closing paragraph.",
  pmr: "An embedded-text refrain.",
  pi: "An indented paragraph. Numbered for levels.",
  mi: "An indented paragraph with no first-line indent.",
  nb: "Continues the previous paragraph across a chapter break.",
  b: "A blank line — the space between stanzas. Takes no text.",
  q: "A line of poetry. Numbered for indent levels.",
  qr: "A right-aligned poetry line, as for a refrain.",
  qc: "A centred poetry line.",
  qa: "An acrostic heading within a poem.",
  qm: "An embedded poetry line, inside quoted text.",
  qd: "A Hebrew note at the end of a psalm.",
  qs: "Selah, set at the end of a poetry line.",
  qt: "Quoted text — words spoken or quoted from elsewhere.",
  li: "A list entry. Numbered for levels.",
  lh: "The header above a list.",
  lf: "The footer below a list.",
  lim: "An embedded list entry.",
  litl: "A list entry's total, as in a genealogy.",
  lik: "The key part of a structured list entry.",
  liv: "The value part of a structured list entry.",
  tr: "A table row. Contains header and body cells.",
  th: "A table header cell. Numbered for columns.",
  thr: "A right-aligned table header cell.",
  tc: "A table body cell. Numbered for columns.",
  tcr: "A right-aligned table body cell.",
  f: "A footnote. Contains a caller and the note's own markers.",
  fe: "An endnote, collected at the end rather than the foot.",
  ef: "A study footnote, outside the translated text.",
  fr: "The verse reference a footnote belongs to.",
  ft: "The text of a footnote.",
  fq: "A quotation from the translation, inside a footnote.",
  fqa: "An alternative translation, inside a footnote.",
  fk: "A keyword, inside a footnote.",
  fl: "A label such as “Heb.”, inside a footnote.",
  fv: "A verse number, inside a footnote.",
  fp: "An additional paragraph within a footnote.",
  x: "A cross-reference to other passages.",
  ex: "A study cross-reference, outside the translated text.",
  xo: "The verse reference a cross-reference belongs to.",
  xt: "The target references of a cross-reference.",
  xq: "A quotation, inside a cross-reference.",
  xk: "A keyword, inside a cross-reference.",
  add: "Words supplied by the translator that are not in the original.",
  bd: "Bold text.",
  it: "Italic text.",
  bdit: "Bold italic text.",
  em: "Emphasis.",
  no: "Normal text, cancelling an enclosing style.",
  sc: "Small capitals.",
  sup: "Superscript text.",
  nd: "The name of God, often set in small capitals.",
  pn: "A proper name.",
  png: "A geographic proper name.",
  addpn: "A proper name supplied by the translator. Deprecated.",
  wj: "The words of Jesus.",
  k: "A keyword, as marked for a glossary.",
  w: "A word marked for the glossary or a lexicon.",
  rq: "An inline quotation reference.",
  qac: "The acrostic letter within a poetry line.",
  lit: "A liturgical note, such as a congregational response.",
  bk: "The title of a book quoted in the text.",
  dc: "Text found only in the Deuterocanon.",
  sig: "The signature at the end of a letter.",
  sls: "Text in a secondary language or source.",
  tl: "A transliterated word.",
  ord: "An ordinal number suffix, as in the “st” of 1st.",
  fig: "An illustration, with a caption and a file to show.",
  ndx: "A subject-index entry.",
  rb: "Ruby glossing, for Chinese and Japanese.",
  pro: "A pronunciation gloss, for Chinese and Japanese.",
  jmp: "A link to somewhere else, inside or outside the document.",
  ip: "An introduction paragraph.",
  im: "An introduction paragraph without its first-line indent.",
  ipi: "An indented introduction paragraph.",
  ipq: "An introduction quotation paragraph.",
  ipr: "An introduction right-aligned paragraph.",
  imi: "An indented introduction paragraph, no first-line indent.",
  imq: "An introduction quotation paragraph, no first-line indent.",
  is: "An introduction section heading.",
  imt: "An introduction major title.",
  imte: "An introduction major title at the end.",
  io: "An introduction outline entry. Numbered for levels.",
  iot: "The title above an introduction outline.",
  ior: "A reference range inside an introduction outline.",
  iq: "An introduction poetry line.",
  ili: "An introduction list entry.",
  iex: "An introduction explanatory or bridge paragraph.",
  ie: "Marks the end of the introduction.",
  ib: "A blank line within the introduction.",
  esb: "A sidebar — material set apart from the main text.",
  esbe: "Ends a sidebar.",
  cat: "A category, for a sidebar or a study note.",
  periph: "A peripheral section, such as a map index or a glossary.",
  pb: "An explicit page break, honoured when printing.",
  zaln: "Alignment data, written by translation tools rather than by hand.",
  ts: "A translation-section milestone, written by translation tools.",
  wg: "A Greek word-level attribute.",
  wh: "A Hebrew word-level attribute.",
  wa: "An Aramaic word-level attribute.",
  rem: "A remark — a note to translators, never printed.",
  sd: "A semantic division, a break stronger than a paragraph.",
  ph: "A paragraph within a list entry. Deprecated.",
  pc: "A centred paragraph, as for an inscription.",
  qt1: "Quoted text, level one.",
  erq: "An inline quotation reference, in study content outside the translation.",
  erqe: "Ends an extended quotation reference.",
  glo: "The glossary, as a peripheral section.",
  intro: "The introduction, as a peripheral section.",
  t: "A translation-section milestone, written by translation tools.",
  // The `z` prefix is USFM's own namespace for markers outside the
  // specification, so this is a fact about the name rather than a guess about
  // the marker: whatever it means, it means it to one tool.
  zpa: "A tool-specific extension. The z prefix marks markers outside the specification.",
  z: "A tool-specific extension. The z prefix marks markers outside the specification.",
};

/**
 * The description for a marker, by longest matching family.
 *
 * Digits are stripped before matching, so `\q1` finds `q` and `\io2` finds
 * `io` — the level is in the syntax, not in the meaning.
 */
function describe(marker: string): { description: string | null; family: string | null } {
  const exact = FAMILIES[marker];
  if (exact) return { description: exact, family: marker };

  // Milestones carry `-s` and `-e` suffixes for their two halves.
  const base = marker.replace(/-[se]$/, "").replace(/\d+$/, "");
  const byBase = FAMILIES[base];
  if (byBase) return { description: byBase, family: base };

  // Then the longest prefix that is a family, so `iot` is not claimed by `io`.
  const candidates = Object.keys(FAMILIES)
    .filter((family) => base.startsWith(family))
    .sort((left, right) => right.length - left.length);

  const family = candidates[0];
  if (family === undefined) return { description: null, family: null };

  return { description: FAMILIES[family] ?? null, family };
}

/**
 * A worked example, from the marker's own shape.
 *
 * Every part of this is in the table: whether it closes, what attributes it
 * takes, which class it belongs to. Nothing is invented, which is what makes it
 * safe to generate for a marker nobody has written prose for.
 */
export function exampleFor(row: MarkerRow): string {
  const name = `\\${row.marker}`;

  // The attribute list, with the default one first because that is the one
  // that may be written bare.
  const attributes =
    row.attributes.length > 0
      ? ` |${row.attributes
          .slice(0, 3)
          .map((attribute) => `${attribute}="…"`)
          .join(" ")}`
      : "";

  // Numbers first, before the class is consulted. `\v` is a *character* marker
  // in the stylesheet, not a paragraph one, so a number check inside the
  // paragraph arm never ran for it and the reference showed `\v text` — for
  // the second most-used marker in USFM.
  if (["c", "v", "ca", "va", "cp", "vp"].includes(row.marker)) {
    return `${name} 1`;
  }
  // Markers that take nothing at all.
  if (["b", "ib", "pb", "ie", "esbe"].includes(row.marker)) {
    return name;
  }

  switch (row.class) {
    case "milestone":
      // A milestone is a position, not a span: it opens and closes with `\*`.
      return `${name}${attributes}\\*`;

    case "note":
      // Notes take a caller and then their own inner markers.
      return `${name} + \\ft The note's text.${name}*`;

    case "character":
      return row.closing === "explicit"
        ? `${name} text${attributes}${name}*`
        : `${name} text${attributes}`;

    case "paragraph":
    default:
      return `${name} Text of the ${row.marker} paragraph.${attributes}`;
  }
}

/** Everything the page needs about one marker. */
export function helpFor(row: MarkerRow): MarkerHelp {
  return { ...row, example: exampleFor(row), ...describe(row.marker) };
}

/** The whole table, described and sorted for display. */
export function helpTable(rows: MarkerRow[]): MarkerHelp[] {
  return rows.map(helpFor).sort((left, right) => left.marker.localeCompare(right.marker));
}

/**
 * Whether a marker matches what was typed in the search box.
 *
 * The marker, its description and its class, because someone looking for
 * "footnote" does not know it is spelled `\f`, and someone looking for `\f`
 * does not want every marker whose description mentions footnotes first.
 */
export function matches(help: MarkerHelp, query: string): boolean {
  const needle = query.trim().toLowerCase().replace(/^\\/, "");
  if (needle === "") return true;

  return (
    help.marker.toLowerCase().includes(needle) ||
    help.class.includes(needle) ||
    (help.description?.toLowerCase().includes(needle) ?? false)
  );
}
