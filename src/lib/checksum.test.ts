import { describe, expect, it } from "vitest";

import { checksum } from "./checksum";

/**
 * The same constants asserted by `checksum.rs`.
 *
 * Two implementations in two languages agreeing by inspection is how they stop
 * agreeing six months later. These vectors are the contract between them: if
 * either side changes, one of the two test files fails.
 */
const VECTORS: [string, number][] = [
  ["", 0x811c9dc5],
  ["a", 0xe40c292c],
  ["\\id GEN\n", 0x8d05171e],
  ["க்ஷேமம்", 0xcbc84650],
  ["\u{1D400}", 0x9adbd370],
  ["\\v 1 שלום\r\n", 0x4c493ce0],
];

describe("checksum", () => {
  it.each(VECTORS)("agrees with the Rust implementation on %j", (text, expected) => {
    expect(checksum(text)).toBe(expected);
  });

  it("distinguishes a transposition", () => {
    // A sum would not, and a mirror that drifted by reordering two characters
    // would go unnoticed.
    expect(checksum("ab")).not.toBe(checksum("ba"));
  });

  it("stays within 32 bits on a long document", () => {
    // Math.imul rather than `*`: above 2^31 the multiply loses its low bits to
    // floating point, and the two languages quietly stop agreeing on exactly
    // the documents that matter.
    const long = "\\v 1 In the beginning.\n".repeat(5000);
    const value = checksum(long);

    expect(Number.isInteger(value)).toBe(true);
    expect(value).toBeGreaterThanOrEqual(0);
    expect(value).toBeLessThanOrEqual(0xffffffff);
  });
});
