//! Incremental editing — P4.5.
//!
//! ARCHITECTURE §8's whole design is that an edit reparses one chunk rather than
//! the document, and the failure mode of an incremental parser is drift: the
//! state after a hundred edits differs from a parse of the same final text, and
//! nothing says so. Every offset in the interface is then wrong.
//!
//! So this applies a sequence of edits and compares the result against a fresh
//! parse of what the text became. The fuzzer's job is to find the sequence where
//! they diverge.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// An edit, in a shape a fuzzer can produce cheaply.
#[derive(Debug, arbitrary::Arbitrary)]
struct Edit {
    from: u16,
    to: u16,
    insert: String,
}

fuzz_target!(|input: (String, Vec<Edit>)| {
    let (start, edits) = input;
    let mut session = usfm_core::Session::new(&start);

    for edit in edits.into_iter().take(32) {
        let length = session.source().len();
        // Clamped to the document and to character boundaries: an editor cannot
        // produce an offset outside the text or inside a character, so a crash
        // from one would be testing something that cannot happen.
        let mut from = (edit.from as usize).min(length);
        let mut to = (edit.to as usize).min(length).max(from);
        while from > 0 && !session.source().is_char_boundary(from) {
            from -= 1;
        }
        while to < length && !session.source().is_char_boundary(to) {
            to += 1;
        }

        if session.edit(usfm_core::ByteSpan::new(from, to), &edit.insert).is_err() {
            return;
        }
    }

    let incremental = session.source().to_string();
    if let Err(violation) = usfm_core::invariants::check(&incremental) {
        panic!("invariant broken after edits: {violation}");
    }

    // The state a hundred edits reached must equal a parse of where they
    // arrived. Anything else is drift, and drift is silent.
    let fresh = usfm_core::Session::new(&incremental);
    assert_eq!(
        session.chunks().len(),
        fresh.chunks().len(),
        "chunking drifted from a fresh parse"
    );
    assert_eq!(
        session.diagnostics().len(),
        fresh.diagnostics().len(),
        "diagnostics drifted from a fresh parse"
    );
});
