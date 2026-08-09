//! Parsing arbitrary bytes.
//!
//! ARCHITECTURE §12.3: "Malformed USFM produces diagnostics without crashing"
//! is a fuzzing claim, and only fuzzing establishes it. The invariants asserted
//! here are the same ones the corpus tests assert, from
//! `usfm_core::invariants` — one definition, so the two cannot drift, and
//! the version that runs on every push is the version the fuzzer is proving.
//!
//! Run:
//!
//! ```sh
//! cargo +nightly fuzz run parse
//! cargo +nightly fuzz run parse -- -max_total_time=86400   # the 24 h gate
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Invalid UTF-8 is a file-loading concern, not a parser one: the editor
    // decodes before the engine ever sees the text. Feeding the parser bytes
    // it cannot receive would spend the fuzzer's time proving nothing.
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    if let Err(failure) = usfm_core::invariants::check(source) {
        panic!("{failure}\n  input: {source:?}");
    }
});
