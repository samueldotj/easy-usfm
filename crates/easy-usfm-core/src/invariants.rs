//! What must hold for *any* input, including bytes nobody would call USFM.
//!
//! ARCHITECTURE §12.3 states the fuzzing claim: malformed USFM produces
//! diagnostics without crashing, and only fuzzing establishes it. These are
//! the properties the fuzz target asserts — never panics, always returns a
//! tree however degenerate, every offset in bounds and on a UTF-8 boundary.
//!
//! They live here rather than inside the fuzz target so that the corpus tests
//! can assert exactly the same things. Two copies of an invariant drift, and
//! the copy that drifts is always the one that is not run on every push.

use crate::{Document, Session};

/// Checks every invariant, returning the first failure.
///
/// Returns `Err` rather than panicking so a fuzz failure prints something
/// actionable instead of a backtrace into the parser.
pub fn check(source: &str) -> Result<(), String> {
    check_document(source)?;
    check_session(source)?;
    Ok(())
}

fn check_document(source: &str) -> Result<(), String> {
    let document = Document::parse(source.to_string());

    if document.source() != source {
        return Err("the source was altered by parsing".into());
    }

    for node in document.descendants() {
        if let Some(span) = node.span.as_ref() {
            check_span(source, span.start, span.end, "node")?;
        }
        if let Some(raw) = node.raw.as_ref() {
            check_span(source, raw.start, raw.end, "raw")?;
        }
    }

    for diagnostic in document.diagnostics() {
        check_span(
            source,
            diagnostic.span.start,
            diagnostic.span.end,
            diagnostic.code.as_str(),
        )?;
    }

    for entry in document.verses().entries() {
        check_span(source, entry.span.start, entry.span.end, "verse")?;
    }

    // Every offset that can cross to the frontend must convert, and convert
    // back. UNICODE §9.1 properties 1 and 3, asserted over arbitrary bytes
    // rather than over a generated alphabet.
    let mapper = document.mapper();
    let mut previous = 0;
    for (offset, _) in source
        .char_indices()
        .chain(std::iter::once((source.len(), ' ')))
    {
        let char16 = mapper
            .to_char16(source, offset)
            .ok_or("the mapper refused its own source")?;
        if char16.get() < previous {
            return Err(format!("to_char16 went backwards at byte {offset}"));
        }
        previous = char16.get();

        if mapper.to_byte(source, char16) != Some(offset) {
            return Err(format!("byte {offset} did not round-trip"));
        }
    }

    Ok(())
}

fn check_session(source: &str) -> Result<(), String> {
    let session = Session::new(source.to_string());

    // Chunks must tile the document exactly. A gap loses text and an overlap
    // duplicates it, and neither is visible until something is edited nearby.
    let mut expected = 0usize;
    for chunk in session.chunks() {
        let range = chunk.range();
        if range.start != expected {
            return Err(format!(
                "chunks do not tile: expected {expected}, found {}",
                range.start
            ));
        }
        if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
            return Err(format!("chunk boundary {range:?} falls inside a character"));
        }
        expected = range.end;
    }
    if expected != source.len() {
        return Err(format!("chunks cover {expected} of {} bytes", source.len()));
    }

    for index in 0..session.chunks().len() {
        for node in session.chunk_content(index) {
            if let Some(span) = node.span.as_ref() {
                check_span(source, span.start, span.end, "chunked node")?;
            }
        }
    }

    Ok(())
}

fn check_span(source: &str, start: usize, end: usize, what: &str) -> Result<(), String> {
    if start > end {
        return Err(format!("{what} span {start}..{end} is inverted"));
    }
    if end > source.len() {
        return Err(format!(
            "{what} span {start}..{end} runs past {} bytes",
            source.len()
        ));
    }
    if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return Err(format!(
            "{what} span {start}..{end} does not fall on character boundaries"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invariants_hold_for_degenerate_input() {
        // The shapes worth failing fast on, so a regression shows up in a unit
        // test rather than six hours into a fuzzing session.
        let cases = [
            "",
            "\\",
            "\\\\\\\\",
            "\\id",
            "\u{feff}\\id GEN\n",
            "\\c\n\\v\n",
            "\\c 1\r\n\\v 1 text",
            "\\v 1 \u{1D400}க்ஷ\u{200d}\n",
            "\\zi\u{200d}d GEN\n",
            "\\c 99999999999999999999\n",
            "\\v 1-\n",
            "\\fig a|b|c|d|e|f\\fig*",
            "\\c 1\n\\c 1\n\\c 1\n",
            "plain text",
            "\\v 1 cafe\u{301}",
        ];

        for case in cases {
            check(case).unwrap_or_else(|error| panic!("{case:?}: {error}"));
        }
    }
}
