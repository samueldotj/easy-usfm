/**
 * The checksum that catches the engine's mirror drifting from the editor.
 *
 * The other half of `crates/usfm-core/src/checksum.rs`. Both run FNV-1a
 * over UTF-16 code units, which is the unit both languages already count in —
 * `charCodeAt` here, `encode_utf16` there — so the two agree by construction
 * rather than by translation.
 *
 * ARCHITECTURE §9 names xxh3. That is right for Rust and wrong for this side:
 * xxh3 in JavaScript means a second WASM module or a hand port to keep in
 * step, which would be a new place for the two sides to disagree inside the
 * mechanism whose only job is detecting disagreement. FNV-1a is weaker and
 * entirely adequate for catching accidental drift.
 */

const OFFSET = 2166136261;
const PRIME = 16777619;

export function checksum(text: string): number {
  let hash = OFFSET;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    // Math.imul, because `*` on a number above 2^31 loses the low bits to
    // floating point and the two languages stop agreeing on long documents.
    hash = Math.imul(hash, PRIME);
  }
  // Back to unsigned; the bitwise operations above work on signed 32-bit.
  return hash >>> 0;
}
