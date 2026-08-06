/**
 * The performance gate — P4.5.
 *
 * "> 20 % perf regression fails the build."
 *
 * Reads the JSON `cargo bench -p easy-usfm-core --bench engine -- --json`
 * produces and compares each measurement against the pinned baseline. Anything
 * more than the tolerance slower fails.
 *
 * # Why ratios and not nanoseconds
 *
 * A hosted runner is not the machine the baselines were recorded on, and its
 * speed varies between runs of the same job — a noisy neighbour on the same
 * hypervisor is enough to move a wall-clock number by a third. Pinning
 * nanoseconds would mean a gate that fails for reasons no commit caused, and a
 * gate that cries wolf is one people learn to re-run until it passes.
 *
 * So the benchmark measures a fixed arithmetic loop in the same process and
 * reports every timing as a ratio against it. A runner half the speed doubles
 * both numbers and leaves the ratio alone.
 *
 * # Updating
 *
 * `node scripts/check-bench.mjs --update <results.json>` rewrites the baselines.
 * Do it when the work genuinely changed, in the commit that changed it — never
 * to turn a red build green. That is the one way this file stops being useful.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const BASELINE = join(here, "..", "crates", "easy-usfm-core", "benches", "baseline.json");

const args = process.argv.slice(2);
const updating = args.includes("--update");
const resultsPath = args.find((argument) => !argument.startsWith("--"));

if (!resultsPath) {
  console.error("usage: check-bench.mjs [--update] <results.json>");
  process.exit(2);
}

const results = JSON.parse(readFileSync(resultsPath, "utf8"));
const baseline = JSON.parse(readFileSync(BASELINE, "utf8"));

if (updating) {
  const ratios = {};
  for (const [name, measured] of Object.entries(results.measurements)) {
    ratios[name] = Number(measured.ratio.toFixed(6));
  }
  writeFileSync(BASELINE, `${JSON.stringify({ ...baseline, ratios }, null, 2)}\n`);
  console.log("baselines updated:");
  for (const [name, ratio] of Object.entries(ratios)) console.log(`  ${name}: ${ratio}`);
  process.exit(0);
}

const tolerance = baseline.tolerance ?? 1.2;
const problems = [];
const rows = [];

for (const [name, pinned] of Object.entries(baseline.ratios)) {
  const measured = results.measurements[name];
  if (!measured) {
    // A benchmark that stopped existing is not a pass. Either it was renamed
    // and the baseline should say so, or it was deleted and nobody noticed.
    problems.push(`${name}: missing from the results`);
    continue;
  }

  const change = measured.ratio / pinned;
  rows.push({ name, pinned, measured: measured.ratio, change });

  if (change > tolerance) {
    const percent = ((change - 1) * 100).toFixed(1);
    problems.push(`${name}: ${percent}% slower than the baseline`);
  }
}

// A measurement with no baseline is reported but does not fail: adding a
// benchmark should not break the build of the commit that adds it.
for (const name of Object.keys(results.measurements)) {
  if (!(name in baseline.ratios)) {
    rows.push({ name, pinned: null, measured: results.measurements[name].ratio, change: null });
  }
}

const width = Math.max(...rows.map((row) => row.name.length), 10);
for (const row of rows) {
  const pinned = row.pinned === null ? "     (new)" : row.pinned.toFixed(6);
  const change =
    row.change === null ? "" : `  ${row.change >= 1 ? "+" : ""}${((row.change - 1) * 100).toFixed(1)}%`;
  console.log(`${row.name.padEnd(width)}  baseline ${pinned}  now ${row.measured.toFixed(6)}${change}`);
}

if (problems.length > 0) {
  console.error(`\nperformance regressed beyond ${((tolerance - 1) * 100).toFixed(0)}%:`);
  for (const problem of problems) console.error(`  ${problem}`);
  console.error(
    "\nIf this is a deliberate trade, re-record the baselines in the same " +
      "commit:\n  node scripts/check-bench.mjs --update <results.json>",
  );
  process.exit(1);
}

console.log("\nwithin tolerance");
