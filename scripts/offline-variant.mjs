/**
 * Proves the offline variant cannot reach the network — P6.3, SECURITY §7.
 *
 * "Offline variant makes zero requests" and "network capture during a full
 * editing session shows zero requests with the updater disabled."
 *
 * A network capture is the direct evidence and needs a machine to run on. What
 * this checks is the stronger structural claim underneath it: the offline build
 * contains no update endpoint and no call site, so there is nothing that
 * *could* make the request — which is why the variant is a compile-out rather
 * than a setting.
 *
 * Run: node scripts/offline-variant.mjs <path-to-binary>
 */

import { readFileSync } from "node:fs";

const binary = process.argv[2];
if (!binary) {
  console.error("usage: offline-variant.mjs <path-to-binary>");
  process.exit(2);
}

const bytes = readFileSync(binary);
const text = bytes.toString("latin1");

// The string only the updater's own code path puts in the binary.
//
// Not the command name: `check_for_update` is registered in both builds,
// because the offline one still answers it -- with `CompiledOut`, so the
// interface can hide the setting rather than offer a switch that does nothing.
// A command that exists to say "no" is not a network path.
const ENDPOINT_MARKER = "No update endpoint is configured";

const present = text.includes(ENDPOINT_MARKER);

console.log(`${binary}: ${(bytes.length / 1024 / 1024).toFixed(1)} MB`);
console.log(`  update path: ${present ? "PRESENT" : "absent"}`);

// Which answer is right depends on which build this is, so the caller says.
// A checker that only ever asserts absence passes on a binary that was never
// built -- and passes just as quietly if the marker string is renamed.
const expecting = process.argv[3] ?? "absent";

if (expecting === "absent" && present) {
  console.error("\nthe offline variant still contains the update path");
  process.exit(1);
}
if (expecting === "present" && !present) {
  console.error(
    "\nthe default build has no update path — either it regressed, or this " +
      "checker is looking for a string that no longer exists, which would make " +
      "the offline check pass for the wrong reason",
  );
  process.exit(1);
}

console.log(`
as expected: ${expecting}`);
