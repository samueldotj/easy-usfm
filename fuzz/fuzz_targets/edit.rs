//! Editing, which is where the harder failures live.
//!
//! Parsing arbitrary text is one thing; applying arbitrary *edits* to it and
//! keeping the chunk index consistent is where P0.4's boundary arithmetic can
//! be wrong. This target drives a real session through a sequence of edits and
//! asserts after each one that the incremental state still matches a parse
//! from scratch — the same equivalence P0.5 asserts over the corpus, here over
//! inputs nobody chose.
//!
//! ```sh
//! cargo +nightly fuzz run edit
//! ```

#![no_main]

use easy_usfm_core::{ByteSpan, Session};
use libfuzzer_sys::fuzz_target;

/// Source, then a sequence of (offset, length, replacement) edits.
#[derive(arbitrary::Arbitrary, Debug)]
struct Scenario {
    source: String,
    edits: Vec<(u16, u16, String)>,
}

fuzz_target!(|scenario: Scenario| {
    let mut session = Session::new(scenario.source.clone());

    for (at, remove, insert) in scenario.edits.iter().take(16) {
        let length = session.source().len();
        if length == 0 {
            break;
        }

        // Snap to character boundaries. Offsets inside a character are refused
        // by design and the refusal is already tested; the interesting space
        // is edits the session accepts.
        let start = snap(session.source(), *at as usize % (length + 1));
        let end = snap(session.source(), (start + *remove as usize).min(length));

        if session.edit(ByteSpan::new(start, end), insert).is_err() {
            continue;
        }

        // Whatever the edit did, the result must be a document the engine
        // would have produced by parsing that text cold.
        if let Err(failure) = easy_usfm_core::invariants::check(session.source()) {
            panic!("after editing: {failure}");
        }

        let fresh = Session::new(session.source().to_string());
        let incremental: Vec<_> = session
            .chunks()
            .iter()
            .map(|chunk| (chunk.number(), chunk.range()))
            .collect();
        let expected: Vec<_> = fresh
            .chunks()
            .iter()
            .map(|chunk| (chunk.number(), chunk.range()))
            .collect();

        assert_eq!(
            incremental, expected,
            "incremental chunking diverged from a fresh parse of the same text"
        );
    }
});

fn snap(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
