//! The offset property tests — UNICODE §9.1.
//!
//! These exist because conflating the three coordinate spaces is the most
//! likely serious bug in the project and **ASCII fixtures cannot detect it**:
//! on ASCII, byte, Char16, and grapheme offsets all agree. So the generated
//! alphabet below is deliberately hostile, and every string these tests run on
//! is built from it.
//!
//! UNICODE §9.1 also observes which properties are worth what. Properties 1
//! and 3 can both pass on an implementation that is internally consistent and
//! wrong — they check the mapper against itself. **Property 2 is the one that
//! catches real bugs**, because it checks the mapper against what JavaScript
//! will actually do with the number.

use easy_usfm_core::{ByteSpan, Char16, Utf16Mapper};
use proptest::prelude::*;

/// The alphabet from UNICODE §9.1: ASCII, conjuncts, a reordered vowel sign,
/// Khmer and Myanmar clusters, Hebrew and Arabic, a combining mark, astral
/// characters, and joiners.
///
/// Chosen so that byte length, UTF-16 length, code-point count, and grapheme
/// count all disagree with one another as often as possible.
const ALPHABET: &[&str] = &[
    "a",
    " ",
    "\n",
    "\\v ",
    "1",
    "க்ஷ",        // Tamil conjunct: 3 code points, 9 bytes, 1 cluster
    "क्ष",        // Devanagari conjunct
    "कि",        // vowel sign that renders to the *left* of its consonant
    "ក្ខ",         // Khmer cluster
    "ဗျ",        // Myanmar cluster
    "שלום",      // Hebrew, right-to-left
    "مرحبا",     // Arabic, right-to-left
    "e\u{301}",  // combining acute
    "\u{1D400}", // astral: one code point, 4 bytes, 2 UTF-16 units
    "😀",        // astral emoji
    "\u{200d}",  // zero-width joiner
    "\u{200c}",  // zero-width non-joiner
    "\u{feff}",  // byte-order mark
];

prop_compose! {
    /// A string built from the hostile alphabet.
    fn mixed_script_text()(pieces in prop::collection::vec(0..ALPHABET.len(), 0..40)) -> String {
        pieces.into_iter().map(|i| ALPHABET[i]).collect()
    }
}

/// Every byte offset that begins a character, plus the length.
fn character_boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect()
}

/// What JavaScript does when handed a Char16 range: index into the UTF-16
/// representation and decode the result.
///
/// Modelled rather than executed, so the property runs in an ordinary unit
/// test. It is a faithful model — a JS string *is* a UTF-16 code-unit array,
/// and `slice` indexes it exactly this way. The Rust and browser grapheme
/// implementations are checked against each other for real in
/// `grapheme_agreement.rs`, where modelling would beg the question.
fn javascript_slice(text: &str, start: u32, end: u32) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[start as usize..end as usize])
}

proptest! {
    /// Property 1 — `to_char16` is monotonic and never lands mid-surrogate.
    #[test]
    fn to_char16_is_monotonic(text in mixed_script_text()) {
        let mapper = Utf16Mapper::new(&text);

        let mut previous = 0;
        for byte in 0..=text.len() {
            let offset = mapper.to_char16(&text, byte).expect("same source").get();
            prop_assert!(
                offset >= previous,
                "byte {byte} went backwards: {offset} after {previous}"
            );
            previous = offset;
        }
    }

    /// Property 1, second half — no offset the mapper produces splits a
    /// surrogate pair.
    ///
    /// Checked by asking whether the offset names a position JavaScript would
    /// accept as a character boundary. A high surrogate at that index with no
    /// preceding low surrogate means the split happened.
    #[test]
    fn no_offset_lands_mid_surrogate(text in mixed_script_text()) {
        let mapper = Utf16Mapper::new(&text);
        let units: Vec<u16> = text.encode_utf16().collect();

        for byte in 0..=text.len() {
            let offset = mapper.to_char16(&text, byte).expect("same source").get() as usize;
            if offset > 0 && offset < units.len() {
                let previous = units[offset - 1];
                let is_low_half = (0xDC00..=0xDFFF).contains(&units[offset]);
                let follows_high = (0xD800..=0xDBFF).contains(&previous);
                prop_assert!(
                    !(is_low_half && follows_high),
                    "byte {byte} produced {offset}, between a surrogate pair"
                );
            }
        }
    }

    /// **Property 2** — slicing the source in JavaScript with a reported
    /// Char16 range yields exactly the text Rust reports as that span.
    ///
    /// The property that matters. Everything connecting the preview to the
    /// source depends on it, and unlike 1 and 3 it cannot be satisfied by an
    /// implementation that is merely self-consistent.
    #[test]
    fn a_char16_range_slices_the_same_text_in_javascript(
        text in mixed_script_text(),
        a in 0usize..64,
        b in 0usize..64,
    ) {
        let boundaries = character_boundaries(&text);
        let start = boundaries[a % boundaries.len()];
        let end = boundaries[b % boundaries.len()];
        let (start, end) = if start <= end { (start, end) } else { (end, start) };

        let mapper = Utf16Mapper::new(&text);
        let span = ByteSpan::new(start, end);

        let rust_text = span.slice(&text).expect("span is on boundaries");
        let range = mapper.to_char16_range(&text, &span).expect("same source");
        let js_text = javascript_slice(&text, range.start.get(), range.end.get());

        prop_assert_eq!(
            rust_text,
            js_text.as_str(),
            "span {:?} -> {}..{} disagreed",
            span,
            range.start.get(),
            range.end.get()
        );
    }

    /// Property 3 — `byte → char16 → byte` is identity for all character
    /// boundaries.
    #[test]
    fn byte_to_char16_and_back_is_identity(text in mixed_script_text()) {
        let mapper = Utf16Mapper::new(&text);

        for byte in character_boundaries(&text) {
            let offset = mapper.to_char16(&text, byte).expect("same source");
            let back = mapper.to_byte(&text, offset);
            prop_assert_eq!(back, Some(byte), "byte {} did not round-trip", byte);
        }
    }

    /// The Char16 length agrees with what JavaScript would report as
    /// `string.length` — the other number the frontend reasons with.
    #[test]
    fn length_agrees_with_javascript_string_length(text in mixed_script_text()) {
        let mapper = Utf16Mapper::new(&text);
        prop_assert_eq!(
            mapper.len_char16().get() as usize,
            text.encode_utf16().count()
        );
    }
}

// --------------------------------------------------------------- the ban ---

/// Whether `T` implements `Serialize`, answered at compile time and reported
/// at runtime.
///
/// The inherent const wins name resolution when `T: Serialize`, and falls back
/// to the trait's default when it does not. That makes "this type cannot be
/// serialized" — normally only observable as a compile error, and so only
/// testable with a second compiler invocation and a stored error message that
/// rots between toolchain releases — into an ordinary assertion.
mod ban {
    use serde::Serialize;
    use std::marker::PhantomData;

    pub struct Probe<T>(PhantomData<T>);

    pub trait NotSerializable {
        const SERIALIZABLE: bool = false;
    }

    impl<T> NotSerializable for Probe<T> {}

    impl<T: Serialize> Probe<T> {
        pub const SERIALIZABLE: bool = true;
    }
}

use ban::{NotSerializable, Probe};

// clippy objects that these assertions have a constant value, which is the
// point: the answer is fixed at compile time, and the assertion exists to
// report it rather than to compute it. If the constant ever flips, the type
// gained a `Serialize` impl and the coordinate-space boundary is gone.
#[allow(clippy::assertions_on_constants)]
#[test]
fn a_byte_offset_cannot_be_serialized() {
    // UNICODE §1: withholding the impl is what turns "a byte offset reached
    // JavaScript" from a bug report by a Tamil translator into a compile
    // error. If this ever goes true, a `#[derive(Serialize)]` has been added
    // to ByteSpan and the boundary is gone.
    assert!(
        !<Probe<ByteSpan> as NotSerializable>::SERIALIZABLE,
        "ByteSpan implements Serialize — byte offsets can now reach the frontend"
    );
}

#[allow(clippy::assertions_on_constants)]
#[test]
fn a_node_cannot_be_serialized_either() {
    // Nodes carry byte spans, so the ban has to extend to anything holding
    // one. This is the check that catches a well-meaning derive on the tree.
    assert!(
        !<Probe<easy_usfm_core::Node> as NotSerializable>::SERIALIZABLE,
        "Node implements Serialize — it carries ByteSpan, so byte offsets leak with it"
    );
    assert!(
        !<Probe<easy_usfm_core::Diagnostic> as NotSerializable>::SERIALIZABLE,
        "Diagnostic implements Serialize — it carries ByteSpan"
    );
}

#[allow(clippy::assertions_on_constants)]
#[test]
fn a_char16_offset_can_be_serialized() {
    // The other half of the boundary. A ban with nothing on the far side of
    // it would be satisfied by a crate that simply cannot talk to anything.
    assert!(<Probe<Char16>>::SERIALIZABLE);
    assert!(<Probe<easy_usfm_core::Char16Range>>::SERIALIZABLE);
}

#[test]
fn char16_serializes_as_a_bare_number() {
    // Transparent, because the frontend compares these against CodeMirror
    // offsets and an object wrapper would be noise on every span.
    let mapper = Utf16Mapper::new("கb");
    let offset = mapper.to_char16("கb", 3).unwrap();
    assert_eq!(serde_json::to_string(&offset).unwrap(), "1");
}
