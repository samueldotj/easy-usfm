/**
 * Two bans the build enforces, because a convention is not a control.
 *
 * # Raw markup
 *
 * SECURITY 1: "the preview never executes content", and the control is that
 * *no path exists* from document content to raw markup -- not that escaping is
 * applied carefully. That holds only while nothing reaches for `{@html}` or
 * `innerHTML`, and the moment something does, every argument in that section
 * stops being true. USFM files arrive by email and USB from third parties;
 * this is the one realistic attack surface the application has.
 *
 * # Physical CSS properties
 *
 * UNICODE §8 asks for logical properties throughout — `margin-inline-start`,
 * never `margin-left` — and names this "the single factor determining whether
 * adding interface mirroring later costs a week or a month, since retrofitting
 * means auditing every stylesheet written in the interim". A rule nobody
 * enforces is a rule that holds until the first hurried afternoon.
 *
 * # Why not stylelint
 *
 * The rule wanted is one rule. stylelint plus a logical-properties plugin is
 * some fifty packages and a configuration file, to check a list of about
 * twenty property names in eleven files. This project already hand-wrote a
 * parser for `markers.toml` on the same reasoning: a dependency is paid for on
 * every install, every audit, and every upgrade, and this one would earn none
 * of that.
 *
 * The check is deliberately textual and deliberately strict. It cannot
 * understand CSS, so it does not try; it looks for property names at the start
 * of a declaration, which is where they are.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");

/**
 * Physical properties, and what to use instead.
 *
 * `width` and `height` are here too. They are not directionally wrong, but
 * `inline-size` and `block-size` are the ones that follow `writing-mode`, and
 * a stylesheet that is half logical is one nobody can reason about.
 */
const PHYSICAL = new Map([
  ["margin-left", "margin-inline-start"],
  ["margin-right", "margin-inline-end"],
  ["margin-top", "margin-block-start"],
  ["margin-bottom", "margin-block-end"],
  ["padding-left", "padding-inline-start"],
  ["padding-right", "padding-inline-end"],
  ["padding-top", "padding-block-start"],
  ["padding-bottom", "padding-block-end"],
  ["border-left", "border-inline-start"],
  ["border-right", "border-inline-end"],
  ["border-top", "border-block-start"],
  ["border-bottom", "border-block-end"],
  ["border-left-width", "border-inline-start-width"],
  ["border-right-width", "border-inline-end-width"],
  ["border-top-width", "border-block-start-width"],
  ["border-bottom-width", "border-block-end-width"],
  ["border-left-color", "border-inline-start-color"],
  ["border-right-color", "border-inline-end-color"],
  ["border-top-color", "border-block-start-color"],
  ["border-bottom-color", "border-block-end-color"],
  ["width", "inline-size"],
  ["height", "block-size"],
  ["min-width", "min-inline-size"],
  ["max-width", "max-inline-size"],
  ["min-height", "min-block-size"],
  ["max-height", "max-block-size"],
  ["top", "inset-block-start"],
  ["bottom", "inset-block-end"],
  ["left", "inset-inline-start"],
  ["right", "inset-inline-end"],
]);

/**
 * Values that are directional even when the property is not.
 *
 * `text-align: left` is the same mistake as `margin-left`, and it is the one
 * that survives a review because the property name looks innocent.
 */
const PHYSICAL_VALUES = [
  [/\btext-align\s*:\s*(left|right)\b/, "text-align: start / end"],
  [/\bfloat\s*:\s*(left|right)\b/, "float: inline-start / inline-end"],
  [/\bclear\s*:\s*(left|right)\b/, "clear: inline-start / inline-end"],
];

/**
 * Lines allowed to break the rule, and why.
 *
 * An exemption must name a reason. The point is not to have an escape hatch;
 * it is that the few real exceptions are visible and countable rather than
 * scattered through the stylesheets as ordinary-looking code.
 */
const ALLOW = /\blint-logical-ok\b/;

/** `width`/`height` on an SVG or a canvas attribute is not CSS. */
const NOT_CSS = /<(svg|canvas|img|rect|circle|image)\b/;

function* files(directory) {
  for (const entry of readdirSync(directory)) {
    if (entry === "node_modules" || entry === "generated" || entry.startsWith(".")) continue;

    const path = join(directory, entry);
    if (statSync(path).isDirectory()) yield* files(path);
    else if (/\.(css|svelte|ts|js|mjs)$/.test(path)) yield path;
  }
}

/**
 * Whether a line is inside a `<style>` block or a `.css` file.
 *
 * A `.svelte` file is mostly markup and script, where `width=` is an attribute
 * and `height` is a variable name. Only the style block is CSS.
 */
function* cssLines(source, isCss) {
  let inStyle = isCss;
  let number = 0;

  for (const line of source.split("\n")) {
    number += 1;
    if (!isCss && /<style\b/.test(line)) {
      inStyle = true;
      continue;
    }
    if (!isCss && /<\/style>/.test(line)) {
      inStyle = false;
      continue;
    }
    if (inStyle) yield [number, line];
  }
}

/**
 * Ways to put a string into the DOM as markup.
 *
 * `{@html}` is Svelte's; the DOM's own are worth naming too, since reaching
 * for `innerHTML` gets to the same place without tripping the first rule.
 */
const RAW_MARKUP = [
  [/\{@html\b/, "{@html}"],
  [/\.innerHTML\s*=/, ".innerHTML ="],
  [/\.outerHTML\s*=/, ".outerHTML ="],
  [/insertAdjacentHTML\s*\(/, "insertAdjacentHTML()"],
];

/**
 * Raw-markup uses anywhere in a file.
 *
 * Not restricted to the style block, and not restricted to the preview: the
 * ban is on the codebase, because the point is that no path exists at all.
 */
export function checkMarkup(source) {
  const problems = [];
  let number = 0;

  for (const line of source.split("\n")) {
    number += 1;

    // No exemption, deliberately. SECURITY 1's control is that no path
    // exists from document content to markup, and a marker that waved one
    // through would turn that back into a convention. The tests for this
    // rule live beside this file rather than under `src/` for the same
    // reason: their fixtures are the banned strings.

    for (const [pattern, name] of RAW_MARKUP) {
      if (pattern.test(line)) {
        problems.push({ number, found: name, use: "typed nodes", line: line.trim() });
      }
    }
  }

  return problems;
}

/**
 * Every physical property in one file's CSS.
 *
 * Separated from the walk so it can be tested against sources that do break
 * the rule. A lint whose only evidence is that it passes on a clean tree has
 * not been shown to do anything at all — this one passed the first time it was
 * run, which is exactly the situation where that matters.
 */
export function check(source, isCss) {
  const problems = [];

  for (const [number, line] of cssLines(source, isCss)) {
    // A comment is where the alternatives get explained, so it is not code.
    const code = line.replace(/\/\*.*?\*\//g, "").split("/*")[0];
    if (ALLOW.test(line) || NOT_CSS.test(code)) continue;

    for (const [property, replacement] of PHYSICAL) {
      // Anchored at a declaration start: after `{`, `;`, or line start.
      // Without that, `border-inline-start-width` matches `width`.
      const pattern = new RegExp(`(^|[{;])\\s*${property}\\s*:`);
      if (pattern.test(code)) {
        problems.push({ number, found: property, use: replacement, line: line.trim() });
      }
    }

    for (const [pattern, replacement] of PHYSICAL_VALUES) {
      const hit = code.match(pattern);
      if (hit) {
        problems.push({ number, found: hit[0].trim(), use: replacement, line: line.trim() });
      }
    }
  }

  return problems;
}

/** The whole tree. Returns everything found, so one run reports all of it. */
export function scan(root) {
  const problems = [];
  for (const path of files(join(root, "src"))) {
    const source = readFileSync(path, "utf8");
    for (const problem of check(source, path.endsWith(".css"))) {
      problems.push({ ...problem, path });
    }
    for (const problem of checkMarkup(source)) {
      problems.push({ ...problem, path });
    }
  }
  return problems;
}

// Run as a script, not when imported by the test.
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].replaceAll("\\", "/"))) {
  const problems = scan(ROOT);

  if (problems.length === 0) {
    console.log("no raw markup, no physical properties");
    process.exit(0);
  }

  const plural = problems.length === 1 ? "physical property" : "physical properties";
  console.error(`\n${problems.length} ${plural}:\n`);

  for (const problem of problems) {
    console.error(`  ${relative(ROOT, problem.path)}:${problem.number}`);
    console.error(`    ${problem.line}`);
    console.error(`    use ${problem.use} instead of ${problem.found}\n`);
  }

  console.error(
    [
      "UNICODE §8 asks for logical properties throughout, so that adding",
      "right-to-left interface mirroring later stays a week of work rather",
      "than a month. If a line genuinely needs a physical property, mark it",
      "with a reason: /* lint-logical-ok: … */",
      "",
    ].join("\n"),
  );
  process.exit(1);
}
