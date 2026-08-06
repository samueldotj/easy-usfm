//! Recovering where a text leaf came from.
//!
//! The parser records source locations for structural nodes only; text leaves
//! arrive with no span and no CST anchor (see [`crate::Node::span`]). That is
//! honest, and it is also the difference between click-to-source landing on a
//! paragraph and landing on a word — P3.6 asks for "the right character", and a
//! node with no span cannot answer at better than node granularity.
//!
//! They are recoverable without descending into the CST, because the lowering
//! copies text through in document order. So a walk that keeps a cursor and
//! looks forward from it finds each run where it actually is.
//!
//! The cursor is what makes this exact rather than a guess. Searching the whole
//! source for a run would find the first occurrence of "and" in Genesis, not
//! the one being clicked. Searching forward from the end of everything already
//! placed can only find the run in its own position.
//!
//! # Whitespace is not verbatim, and everything else is
//!
//! A verse continued on the next line arrives as one run with the line break
//! turned into a space. So an exact search finds nothing for every verse but
//! the last one in a chapter, which is a feature that works only on the fixture
//! you wrote to test it.
//!
//! The match therefore treats any whitespace character as equal to any other —
//! **one for one**, never collapsing a run of them. That distinction is the
//! whole correctness argument: a match that let two source spaces stand for one
//! rendered space would shift every offset after it by one, and click-to-source
//! would land next to the character the user pointed at rather than on it.
//! Matching one for one means the run and the source agree position by position
//! in UTF-16 as well, since every whitespace character is one code unit.
//!
//! # When it declines
//!
//! A run that cannot be matched returns `None` and the caller keeps node
//! granularity. Degrading to the enclosing node is a click that is coarse;
//! fabricating an offset is a click that is wrong, and wrong is worse, because
//! it looks like the editor scrolled somewhere for a reason.

use crate::span::ByteSpan;

/// Where `text` sits in `source`, at or after `from`.
///
/// Byte offsets throughout, which is why this is in the core and returns a
/// [`ByteSpan`]: the conversion to something JavaScript can index is the
/// boundary's job, and doing the search in UTF-16 would mean carrying a second
/// copy of the document in a second encoding.
pub fn locate(source: &str, from: usize, text: &str) -> Option<ByteSpan> {
    // The first character that is not whitespace, and how many precede it.
    // The search anchors there rather than at the start of the run, because
    // whitespace is the part allowed to differ and so the part that cannot be
    // searched for. A run of nothing but whitespace has no anchor and no
    // character worth clicking.
    let leading = text
        .chars()
        .take_while(|character| character.is_whitespace())
        .count();
    let anchor = text.chars().nth(leading)?;

    let mut at = from;
    loop {
        let hit = source.get(at..)?.find(anchor)?;
        let found = at + hit;

        if let Some(start) = back_over_whitespace(source, found, leading, from) {
            if let Some(end) = match_run(source, start, text) {
                return Some(ByteSpan::new(start, end));
            }
        }

        at = found + anchor.len_utf8();
    }
}

/// Steps back over `count` whitespace characters, or `None` if they are not.
///
/// Not below `floor`, which is the cursor: a run's leading space belongs to
/// this run only if it comes after everything already placed.
fn back_over_whitespace(source: &str, from: usize, count: usize, floor: usize) -> Option<usize> {
    let mut at = from;

    for _ in 0..count {
        let previous = source.get(..at)?.chars().next_back()?;
        if !previous.is_whitespace() {
            return None;
        }
        at -= previous.len_utf8();
        if at < floor {
            return None;
        }
    }
    Some(at)
}

/// Matches `text` against the source at `at`, and returns where it ends.
///
/// One character at a time rather than a byte comparison, because the whole
/// point is the pairs that are equal without being identical.
fn match_run(source: &str, at: usize, text: &str) -> Option<usize> {
    let mut end = at;
    let mut actual = source.get(at..)?.chars();

    for wanted in text.chars() {
        let character = actual.next()?;
        let same = character == wanted || (character.is_whitespace() && wanted.is_whitespace());
        if !same {
            return None;
        }
        end += character.len_utf8();
    }
    Some(end)
}

/// Advances a cursor over a tree, handing each text leaf its span.
///
/// Depth-first in document order, because that is the order the runs appear in
/// the source and the whole method depends on it.
///
/// A structural node's own span moves the cursor to where that node begins, so
/// a run inside a `\add ... \add*` is looked for after the opening marker
/// rather than from wherever the previous paragraph ended. Nodes without spans
/// simply do not move it, which is the right thing: they are the ones whose
/// position is unknown.
pub struct Cursor {
    at: usize,
}

impl Cursor {
    /// Starts at the beginning of the region being walked.
    pub const fn new(at: usize) -> Self {
        Self { at }
    }

    /// Moves to the start of a node whose position is known.
    ///
    /// Forward only. A parser that emits a span behind the cursor would
    /// otherwise send the search back over text already placed, and the next
    /// run would match its own earlier occurrence.
    pub fn enter(&mut self, span: &ByteSpan) {
        self.at = self.at.max(span.start);
    }

    /// Moves past a node whose position is known.
    pub fn leave(&mut self, span: &ByteSpan) {
        self.at = self.at.max(span.end);
    }

    /// The span of a text run, and moves past it.
    pub fn take(&mut self, source: &str, text: &str) -> Option<ByteSpan> {
        let span = locate(source, self.at, text)?;
        self.at = span.end;
        Some(span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_a_run_from_the_cursor() {
        let source = "\\v 1 In the beginning\n";
        assert_eq!(
            locate(source, 5, "In the beginning"),
            Some(ByteSpan::new(5, 21))
        );
    }

    #[test]
    fn takes_the_leading_whitespace_the_run_carries() {
        // The run the parser emits begins with the space after `\v 1`, so the
        // span has to begin there too -- an offset into the run is added to
        // its start, and a start one character late puts every click one
        // character early.
        let source = "\\v 1 In the beginning\n";
        assert_eq!(
            locate(source, 0, " In the beginning"),
            Some(ByteSpan::new(4, 21))
        );
    }

    #[test]
    fn does_not_look_backwards() {
        // The point of the cursor. "God" appears twice; from past the first
        // one, the answer has to be the second.
        let source = "\\v 1 God said\n\\v 2 God saw\n";
        assert_eq!(locate(source, 14, "God"), Some(ByteSpan::new(19, 22)));
    }

    #[test]
    fn a_line_break_inside_a_verse_reads_as_a_space() {
        // Every verse but the last one in a chapter, and the case an exact
        // search silently fails on.
        let source = "\\v 1 God said\nlet there be light\n";
        let span = locate(source, 5, "God said let there be light").expect("the run is there");
        assert_eq!(span, ByteSpan::new(5, 32));
        assert_eq!(span.slice(source), Some("God said\nlet there be light"));
    }

    #[test]
    fn one_whitespace_character_never_stands_for_two() {
        // The rule that keeps offsets exact. Collapsing here would return a
        // span whose text is one character longer than the run, and every
        // click after the gap would land one character off.
        assert_eq!(locate("\\v 1 God  said\n", 5, "God said"), None);
    }

    #[test]
    fn declines_an_empty_run() {
        assert_eq!(locate("\\p\n", 0, ""), None);
    }

    #[test]
    fn declines_a_run_of_only_whitespace() {
        assert_eq!(locate("\\p \n", 0, " "), None);
    }

    #[test]
    fn declines_text_that_is_not_there() {
        assert_eq!(locate("\\v 1 one two\n", 0, "one three"), None);
    }

    #[test]
    fn declines_a_cursor_past_the_end() {
        assert_eq!(locate("\\p\n", 99, "p"), None);
    }

    #[test]
    fn skips_a_false_start_and_finds_the_real_one() {
        // The anchor matches in two places and only the second one is the run,
        // so a search that gave up on the first miss would find nothing.
        let source = "\\v 1 God\n\\v 2 God said\n";
        assert_eq!(locate(source, 0, "God said"), Some(ByteSpan::new(14, 22)));
    }

    #[test]
    fn spans_are_bytes_not_characters() {
        // The distinction UNICODE 1 names as the likeliest serious bug, and the
        // reason this returns a ByteSpan that cannot cross to JavaScript.
        let source = "\\v 1 அப்பா\n";
        let span = locate(source, 0, "அப்பா").expect("the run is there");
        assert_eq!(span.slice(source), Some("அப்பா"));
        assert_eq!(span.len(), "அப்பா".len());
        assert!(span.len() > "அப்பா".chars().count());
    }

    #[test]
    fn a_conjunct_is_matched_whole() {
        // Tamil க்ஷ is several code points and one glyph. Nothing here treats
        // it specially, which is the point: the match is over code points and
        // the cluster boundaries are the browser's problem, not this one's.
        let source = "\\v 1 லட்சுமி வந்தார்\n";
        let span = locate(source, 5, "லட்சுமி").expect("the run is there");
        assert_eq!(span.slice(source), Some("லட்சுமி"));
    }

    #[test]
    fn a_cursor_walks_runs_in_order() {
        let source = "\\v 1 God said\n\\v 2 God saw\n";
        let mut cursor = Cursor::new(0);

        cursor.leave(&ByteSpan::new(0, 5)); // \v 1
        assert_eq!(cursor.take(source, "God said "), Some(ByteSpan::new(5, 14)));

        cursor.leave(&ByteSpan::new(14, 19)); // \v 2
        assert_eq!(
            cursor.take(source, "God saw\n"),
            Some(ByteSpan::new(19, 27))
        );
    }

    #[test]
    fn a_cursor_never_goes_backwards() {
        let mut cursor = Cursor::new(50);
        cursor.enter(&ByteSpan::new(10, 20));
        cursor.leave(&ByteSpan::new(10, 20));

        let source = "x".repeat(60);
        // Still looking from 50, not from 20 -- so a run before it is not found
        // rather than being matched against text that has already been placed.
        assert_eq!(cursor.take(&source, &"x".repeat(20)), None);
    }
}
