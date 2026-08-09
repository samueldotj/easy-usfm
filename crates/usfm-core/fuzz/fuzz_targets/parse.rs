//! Parsing anything at all — P4.5.
//!
//! The assertion is not "this parses correctly". Nobody knows what an arbitrary
//! byte string means as USFM, and a fuzzer will never produce a file a
//! translator would recognise. What it is very good at producing is the input
//! that makes a span point one byte past the end of a string, and that is what
//! `invariants::check` exists to catch — the same checker the pathological
//! corpus runs, so a fuzzer finding and a hand-written case are held to one
//! standard.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Invalid UTF-8 is refused before the parser sees it, so there is nothing
    // here to learn from it. The `round_trip` target covers the decoder.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    if let Err(violation) = usfm_core::invariants::check(text) {
        panic!("invariant broken: {violation}");
    }
});
