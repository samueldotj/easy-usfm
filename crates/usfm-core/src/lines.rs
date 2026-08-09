//! Where a byte offset falls, as a human would say it: line and column.
//!
//! The line index already existed, but only inside [`Utf16Mapper`] — so the
//! only way to ask "which line is this?" was through the type that exists to
//! cross into JavaScript. That is the wrong shape for a consumer with no
//! JavaScript in it: a compositor reporting `book.usfm:42:7` in a terminal
//! would have been paying for a UTF-16 conversion it then discarded.
//!
//! So the byte-only half lives here and is public, and [`Utf16Mapper`] holds
//! one of these plus the UTF-16 column of each line start. There is one line
//! table, not two.
//!
//! # Which coordinate space this is
//!
//! `docs/UNICODE.md` §1 names three — byte, Char16, and grapheme — and warns
//! that conflating them is the most likely serious bug here, because on ASCII
//! all three agree. [`LineCol`] deliberately mixes two of them, and says so:
//! the **line** is a count of newlines, and the **column** is a count of
//! grapheme clusters, because that is what a person reading `42:7` means by
//! it. It is not a byte offset and must never be used as one.
//!
//! [`Utf16Mapper`]: crate::Utf16Mapper

/// A position as a person would write it: `line:column`, both 1-based.
///
/// **Not an offset.** `column` counts grapheme clusters, matching UNICODE §2
/// and the editor's status bar, so a Tamil conjunct written with four code
/// points advances it by one. Use [`crate::ByteSpan`] to index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineCol {
    /// 1-based, counting `\n`.
    pub line: u32,
    /// 1-based, counting grapheme clusters from the start of the line.
    pub column: u32,
}

impl std::fmt::Display for LineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Byte offsets of every line start in a source text.
///
/// One pass to build, one binary search to query.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset at the start of each line. Always begins with `0`, so the
    /// count of starts at or before an offset is its 1-based line number.
    starts: Vec<u32>,
    len_bytes: u32,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0u32];
        for (byte, character) in source.char_indices() {
            if character == '\n' {
                starts.push((byte + character.len_utf8()) as u32);
            }
        }
        Self {
            starts,
            len_bytes: source.len() as u32,
        }
    }

    /// Number of lines.
    ///
    /// A source ending in a newline has an empty final line, and it is counted
    /// — the same convention as the editor gutter, which matters because these
    /// numbers sit next to each other in the diagnostics panel.
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Whether this index was built from `source`.
    ///
    /// Length only, which is the same check [`crate::Utf16Mapper`] makes: it
    /// catches the mistake that actually happens — an index kept across an
    /// edit — without hashing the document on every query.
    pub fn matches(&self, source: &str) -> bool {
        source.len() == self.len_bytes as usize
    }

    /// The 1-based line a byte offset falls on.
    ///
    /// Total: an offset past the end clamps to the last line, so a malformed
    /// span produces a number that is nearly right rather than a panic. The
    /// same bargain the rest of this crate makes with bad offsets.
    pub fn line(&self, byte: usize) -> u32 {
        self.index_of(byte) as u32 + 1
    }

    /// Byte offset where a 1-based line begins, or `None` past the end.
    pub fn line_start(&self, line: u32) -> Option<usize> {
        let i = (line as usize).checked_sub(1)?;
        self.starts.get(i).map(|b| *b as usize)
    }

    /// The text of a 1-based line, without its trailing newline.
    pub fn line_text<'a>(&self, source: &'a str, line: u32) -> Option<&'a str> {
        if !self.matches(source) {
            return None;
        }
        let start = self.line_start(line)?;
        let end = self
            .line_start(line + 1)
            .unwrap_or(source.len())
            .min(source.len());
        let text = source.get(start..end)?;
        Some(text.strip_suffix('\n').unwrap_or(text))
    }

    /// Line and grapheme column for a byte offset.
    ///
    /// `None` only if `source` is not the text this index was built from.
    pub fn locate(&self, source: &str, byte: usize) -> Option<LineCol> {
        if !self.matches(source) {
            return None;
        }
        let byte = byte.min(source.len());
        let line = self.line(byte);
        let start = self.line_start(line)?;
        let text = self.line_text(source, line)?;
        // `grapheme::column` counts clusters *before* the offset, so it is
        // 0-based; a human reading `42:7` counts from one.
        let column = crate::grapheme::column(text, byte.saturating_sub(start)) as u32 + 1;
        Some(LineCol { line, column })
    }

    /// 0-based index into `starts` for the line containing `byte`.
    pub(crate) fn index_of(&self, byte: usize) -> usize {
        self.starts
            .partition_point(|start| (*start as usize) <= byte)
            .saturating_sub(1)
    }

    /// Byte offset of the line at a 0-based index. For [`crate::Utf16Mapper`],
    /// which keeps its own parallel column table against these same lines.
    pub(crate) fn start_at(&self, index: usize) -> u32 {
        self.starts[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_counted_from_one() {
        let index = LineIndex::new("a\nb\nc");
        assert_eq!(index.line(0), 1);
        assert_eq!(index.line(1), 1); // the newline belongs to the line it ends
        assert_eq!(index.line(2), 2);
        assert_eq!(index.line(4), 3);
    }

    #[test]
    fn a_trailing_newline_leaves_an_empty_last_line() {
        let index = LineIndex::new("a\n");
        assert_eq!(index.line_count(), 2);
        assert_eq!(index.line(2), 2);
    }

    /// The bug this whole module exists to make impossible on the Rust side:
    /// a column that counts bytes is right on ASCII and wrong everywhere else.
    #[test]
    fn columns_count_clusters_not_bytes() {
        // Devanagari "क्ष", where GB9c holds the conjunct together: one
        // cluster, three code points, nine UTF-8 bytes. The same fixture
        // `grapheme.rs` uses, so the two agree by construction.
        let source = "\\v 1 क्षa\n";
        let index = LineIndex::new(source);

        let at = |byte| index.locate(source, byte).expect("same source");

        assert_eq!(at(0), LineCol { line: 1, column: 1 });
        assert_eq!(at(5), LineCol { line: 1, column: 6 });
        // Immediately after the cluster: one column further on, nine bytes on.
        assert_eq!(at(14), LineCol { line: 1, column: 7 });
    }

    #[test]
    fn a_byte_inside_a_cluster_does_not_advance_the_column() {
        let source = "क्षa";
        let index = LineIndex::new(source);
        for byte in 0..9 {
            assert_eq!(index.locate(source, byte).unwrap().column, 1, "{byte}");
        }
        assert_eq!(index.locate(source, 9).unwrap().column, 2);
    }

    #[test]
    fn the_column_restarts_on_each_line() {
        let source = "\\id GEN\n\\v 1 text\n";
        let index = LineIndex::new(source);
        assert_eq!(
            index.locate(source, 8).unwrap(),
            LineCol { line: 2, column: 1 }
        );
        assert_eq!(
            index.locate(source, 10).unwrap(),
            LineCol { line: 2, column: 3 }
        );
    }

    #[test]
    fn line_text_excludes_the_newline() {
        let source = "\\id GEN\n\\c 1\n";
        let index = LineIndex::new(source);
        assert_eq!(index.line_text(source, 1), Some("\\id GEN"));
        assert_eq!(index.line_text(source, 2), Some("\\c 1"));
        assert_eq!(index.line_text(source, 3), Some(""));
        assert_eq!(index.line_text(source, 4), None);
    }

    #[test]
    fn a_byte_past_the_end_clamps_rather_than_panicking() {
        let source = "abc";
        let index = LineIndex::new(source);
        assert_eq!(index.line(9_999), 1);
        assert_eq!(index.locate(source, 9_999).unwrap().column, 4);
    }

    #[test]
    fn the_wrong_source_is_refused_rather_than_answered() {
        let index = LineIndex::new("\\c 1\n");
        assert_eq!(index.locate("something else entirely", 3), None);
        assert_eq!(index.line_text("something else entirely", 1), None);
    }

    #[test]
    fn it_displays_the_way_a_compiler_would() {
        assert_eq!(
            LineCol {
                line: 42,
                column: 7
            }
            .to_string(),
            "42:7"
        );
    }
}
