/**
 * Markers grouped by what they do together.
 *
 * The reference used to be an alphabetical list by class, which put `\tc` a
 * long way from `\tr` and `\erqe` nowhere near `\erq`. That ordering answers
 * "what is this marker?" and never answers the question people actually arrive
 * with, which is "how do I write a table?" — a table is five markers that only
 * mean anything together, and reading them one at a time in alphabetical order
 * is reading a recipe by ingredient.
 *
 * So each group carries a **combined example**: the markers in use, together,
 * as they would appear in a file. That is the part worth copying, and it is the
 * part a per-marker list structurally cannot show.
 *
 * # Written, not derived — and that is the trade
 *
 * `markers.toml` records what each marker *is*, not which markers collaborate.
 * Nothing in the table says `\th` belongs with `\tr`. So the grouping and the
 * combined examples are written here, and the cost is that a marker added to
 * the table lands in "Other markers" until someone places it. That is the right
 * failure: it is visible, it is harmless, and the alternative — guessing a
 * grouping from the marker's name — would silently file things wrongly.
 *
 * Every marker still appears exactly once. The last group is a catch-all, and a
 * test asserts nothing is lost between the table and the page.
 */

import type { MarkerHelp } from "./markerHelp";

export interface MarkerGroup {
  id: string;
  title: string;
  /** What this family of markers is for. */
  blurb: string;
  /** The markers in use together, as they appear in a file. */
  example: string;
  /**
   * Which markers belong here.
   *
   * Prefixes rather than an enumeration, so `\q1`…`\q9` and `\th1`…`\th5` are
   * covered without listing every level the specification numbers openly.
   */
  prefixes: string[];
}

/** The `\v` in a combined example, written once. */
const NL = "\n";

export const GROUPS: readonly MarkerGroup[] = [
  {
    id: "identification",
    title: "Identification and headers",
    blurb:
      "The head of the file: which book it is, what to print in the running " +
      "header, and what the table of contents should say.",
    example: [
      "\\id GEN Genesis — Example Version",
      "\\usfm 3.1",
      "\\h Genesis",
      "\\toc1 The First Book of Moses",
      "\\toc2 Genesis",
      "\\toc3 Gen",
      "\\mt1 Genesis",
    ].join(NL),
    prefixes: ["id", "ide", "usfm", "h", "toc", "toca", "rem", "sts"],
  },
  {
    id: "titles",
    title: "Titles and section headings",
    blurb:
      "Headings above the text, from the book's own title down to the section " +
      "headings inside a chapter, with their reference ranges.",
    example: [
      "\\ms1 Book One",
      "\\mr (Psalms 1–41)",
      "\\s1 The Creation",
      "\\sr Genesis 1:1–2:3",
      "\\r (John 1:1–5)",
      "\\d A Psalm of David.",
      "\\sp Job",
    ].join(NL),
    // The `e` forms close a heading that spans a passage.
    prefixes: ["mt", "mte", "ms", "mse", "mr", "s", "sr", "r", "d", "sp", "sd"],
  },
  {
    id: "chapters",
    title: "Chapters and verses",
    blurb:
      "The numbering. The alternative and published forms exist because " +
      "translations disagree about where verses fall, and both numbers " +
      "sometimes have to be shown.",
    example: [
      "\\c 1",
      "\\cl Chapter",
      "\\cd A description printed under the number.",
      "\\p",
      "\\v 1 In the beginning God created the heaven and the earth.",
      "\\v 2-3 \\va 2b\\va* Verses joined, with an alternative number.",
    ].join(NL),
    prefixes: ["c", "ca", "cp", "cl", "cd", "v", "va", "vp"],
  },
  {
    id: "paragraphs",
    title: "Paragraphs",
    blurb:
      "The prose. Most of a translation is `\\p`; the rest differ in indent, " +
      "alignment, or whether they continue across a chapter break.",
    example: [
      "\\p",
      "\\v 1 An ordinary paragraph of prose.",
      "\\m Continuing without a first-line indent.",
      "\\pi1 An indented paragraph.",
      "\\nb Continuing across the chapter break above.",
    ].join(NL),
    prefixes: ["p", "m", "po", "pr", "pc", "pm", "pmo", "pmc", "pmr", "pi", "mi", "nb", "cls", "ph"],
  },
  {
    id: "poetry",
    title: "Poetry",
    blurb:
      "Verse lines, indented by level. `\\b` is the blank line between " +
      "stanzas and takes no text of its own.",
    example: [
      "\\q1 The heavens declare the glory of God;",
      "\\q2 and the firmament sheweth his handywork.",
      "\\b",
      "\\q1 Day unto day uttereth speech, \\qs Selah\\qs*",
      "\\qr and night unto night sheweth knowledge.",
    ].join(NL),
    prefixes: ["q", "qr", "qc", "qa", "qm", "qd", "qs", "qac", "b"],
  },
  {
    id: "lists",
    title: "Lists",
    blurb:
      "Entries with an optional header and footer. The key and value forms " +
      "are for structured lists such as genealogies.",
    example: [
      "\\lh The sons of Israel:",
      "\\li1 Reuben",
      "\\li1 Simeon",
      "\\li2 \\lik Total\\lik* \\liv 12\\liv*",
      "\\lf These are the twelve tribes.",
    ].join(NL),
    prefixes: ["li", "lh", "lf", "lim", "litl", "lik", "liv"],
  },
  {
    id: "tables",
    title: "Tables",
    blurb:
      "A row at a time. `\\tr` begins each row; the cells inside it are " +
      "header cells or body cells, numbered by column, with `r` forms for " +
      "right alignment. None of them means anything on its own.",
    example: [
      "\\tr \\th1 Tribe \\th2 Leader \\thr3 Number",
      "\\tr \\tc1 Reuben \\tc2 Elizur \\tcr3 46,500",
      "\\tr \\tc1 Simeon \\tc2 Shelumiel \\tcr3 59,300",
    ].join(NL),
    // `thc`/`tcc` are the centred forms, and `tcr`/`thr` the right-aligned.
    prefixes: ["tr", "th", "thr", "thc", "tch", "tc", "tcr", "tcc"],
  },
  {
    id: "footnotes",
    title: "Footnotes and endnotes",
    blurb:
      "A note opens with a caller — `+` asks for automatic numbering — then " +
      "the reference it belongs to and its text, and closes with `*`. The " +
      "inner markers only appear inside a note.",
    example: [
      "\\v 1 In the beginning\\f + \\fr 1:1 \\fk beginning\\fk* " +
        "\\ft Or \\fqa when God began to create\\fqa*.\\f*",
      "\\v 2 And the earth\\fe + \\fr 1:2 \\ft An endnote instead.\\fe*",
    ].join(NL),
    prefixes: [
      "f", "fe", "ef", "efe", "efm", "fr", "ft", "fq", "fqa", "fk", "fl", "fv", "fp",
      "fdc", "fm", "fs", "fw",
    ],
  },
  {
    id: "cross-references",
    title: "Cross-references",
    blurb:
      "The same shape as a footnote, but pointing at other passages: the " +
      "origin reference, then the targets.",
    example: [
      "\\v 1 In the beginning\\x + \\xo 1:1 \\xt John 1:1-3; Hebrews 11:3\\x*",
      "\\v 3 And God said\\ex + \\xo 3 \\xt Psalm 33:6\\ex*",
    ].join(NL),
    prefixes: [
      "x", "ex", "exe", "xo", "xt", "xta", "xq", "xk", "xdc", "xnt", "xot", "xop",
      "xtSee", "xtSeeAlso",
    ],
  },
  {
    id: "character",
    title: "Character styles",
    blurb:
      "Formatting inside a paragraph. Each opens with the marker and closes " +
      "with the same marker followed by `*`, and they nest.",
    example: [
      "\\v 1 \\wj Come unto me\\wj*, said \\nd the Lord\\nd*.",
      "\\v 2 The word \\bd bold\\bd*, \\it italic\\it*, and \\bdit both\\bdit*.",
      "\\v 3 Words \\add supplied by the translator\\add* are marked.",
    ].join(NL),
    prefixes: [
      "add", "bd", "it", "bdit", "em", "no", "sc", "sup", "nd", "pn", "png", "addpn",
      "wj", "k", "w", "rq", "lit", "bk", "dc", "sig", "sls", "tl", "ord", "ndx", "rb",
      "pro", "jmp", "qt", "wg", "wh", "wa", "wr",
    ],
  },
  {
    id: "introduction",
    title: "Introduction",
    blurb:
      "Everything before chapter one. The markers mirror the ones used in the " +
      "text — paragraphs, headings, lists, poetry — with an `i` prefix.",
    example: [
      "\\imt1 The Letter to the Romans",
      "\\is1 Author",
      "\\ip Paul, an apostle, writing to the church at Rome.",
      "\\iot Outline",
      "\\io1 Greeting \\ior (1:1-7)\\ior*",
      "\\io2 The gospel \\ior (1:8-17)\\ior*",
      "\\ie",
    ].join(NL),
    prefixes: [
      "ip", "im", "ipi", "ipq", "ipr", "imi", "imq", "is", "imt", "imte", "io", "iot",
      "ior", "iq", "ili", "iex", "ie", "ib", "iqt", "intro",
    ],
  },
  {
    id: "study",
    title: "Sidebars, figures and study content",
    blurb:
      "Material set apart from the translation: sidebars, illustrations, and " +
      "the peripheral sections such as a glossary.",
    example: [
      "\\esb \\cat History\\cat*",
      "\\ms1 A sidebar heading",
      "\\p Text set apart from the translation.",
      "\\esbe",
      "",
      "\\fig A map of the region|src=\"art/map.png\" size=\"col\" ref=\"1:1\"\\fig*",
    ].join(NL),
    // The peripherals are the named sections a printed Bible carries around
    // the text -- cover, preface, concordance, maps -- which `\periph` opens.
    prefixes: [
      "esb", "esbe", "cat", "periph", "fig", "glo", "erq", "erqe", "pb",
      "conc", "cov", "idx", "maps", "pref", "pub", "pubinfo", "spine", "ps", "psi",
      "phi", "restore", "intro",
    ],
  },
  {
    id: "milestones",
    title: "Milestones",
    blurb:
      "A position rather than a span. They come in `-s` and `-e` halves that " +
      "mark where something starts and ends, and close with `\\*`.",
    example: [
      "\\v 1 \\qt-s |who=\"Pilate\"\\* Art thou the King of the Jews?\\qt-e\\*",
      "\\ts-s\\* A translation section begins here.\\ts-e\\*",
    ].join(NL),
    prefixes: ["qt-s", "qt-e", "ts", "t", "z", "zpa"],
  },
];

/**
 * Whether `marker` is a numbered level of `prefix` — `\q1` of `\q`.
 *
 * Milestone halves count too: `\qt-s` is a level of `\qt` in the sense that
 * matters here, which is that it belongs wherever `\qt` does.
 */
function isLevelOf(marker: string, prefix: string): boolean {
  if (!marker.startsWith(prefix)) return false;

  const rest = marker.slice(prefix.length);
  // A level (`1`), optionally an "end" form (`s1e` closes `s1`), or a
  // hyphenated suffix — milestone halves (`-s`, `-e`) and the tool extensions
  // that spell theirs `-xb`. Still not a bare `startsWith`: the remainder has
  // to be one of those shapes, so an unrelated marker cannot slip in.
  // A level may carry a suffix as well: `\qt1-s` is level one of a quotation
  // milestone's opening half.
  return /^\d*e?$/.test(rest) || /^\d*-[a-z]+$/.test(rest);
}

/** Where a marker belongs. `null` for one nothing has placed yet. */
export function groupOf(marker: string): MarkerGroup | null {
  // The base name: levels and milestone halves do not change which family a
  // marker belongs to.
  const base = marker.replace(/\d+$/, "");

  let best: MarkerGroup | null = null;
  let bestLength = -1;

  for (const group of GROUPS) {
    for (const prefix of group.prefixes) {
      // Exact, or a numbered level of the prefix — never a bare `startsWith`.
      //
      // A loose prefix match is how a hand-written grouping files things
      // wrongly without saying so: single-letter prefixes are unavoidable here
      // (`\q`, `\p`, `\f`, `\x`), and `startsWith` gave every unknown marker
      // beginning with `q` to poetry. Requiring the remainder to be digits
      // means `\q1`…`\q9` are placed and `\qzz` falls to the catch-all, which
      // is visible and harmless.
      const hit = marker === prefix || base === prefix || isLevelOf(marker, prefix);
      if (hit && prefix.length > bestLength) {
        best = group;
        bestLength = prefix.length;
      }
    }
  }
  return best;
}

/** One group as the page renders it. */
export interface RenderedGroup extends MarkerGroup {
  markers: MarkerHelp[];
}

/**
 * Sorts every marker into its group, keeping the group order above.
 *
 * The catch-all is appended only when something lands in it, so a complete
 * grouping shows no "Other markers" heading at all.
 */
export function grouped(table: MarkerHelp[]): RenderedGroup[] {
  const buckets = new Map<string, MarkerHelp[]>();
  const leftovers: MarkerHelp[] = [];

  for (const help of table) {
    const group = groupOf(help.marker);
    if (!group) {
      leftovers.push(help);
      continue;
    }
    const bucket = buckets.get(group.id) ?? [];
    bucket.push(help);
    buckets.set(group.id, bucket);
  }

  const rendered: RenderedGroup[] = GROUPS.filter((group) => (buckets.get(group.id)?.length ?? 0) > 0)
    .map((group) => ({ ...group, markers: buckets.get(group.id) ?? [] }));

  if (leftovers.length > 0) {
    rendered.push({
      id: "other",
      title: "Other markers",
      blurb:
        "Markers the specification defines that this page has not yet placed " +
        "in a group. The syntax below still comes from the specification.",
      example: "",
      prefixes: [],
      markers: leftovers,
    });
  }
  return rendered;
}
