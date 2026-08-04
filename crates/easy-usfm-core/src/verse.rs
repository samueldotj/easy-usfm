//! The verse index — what verses a document contains, and where.
//!
//! ARCHITECTURE §7. Verse numbering is not a sequence of integers, and every
//! part of the model here exists because published Scripture does something
//! an integer cannot express:
//!
//! - `\v 1-2` — a **range**, where one block of text carries two verses,
//!   because the translation reorders what the original separated.
//! - `\v 1a` — a **segment**, where one verse is split across paragraphs.
//! - `\va 3\va*` — an **alternate** number from a different versification.
//! - `\vp ௩\vp*` — the **published** number, which may be in any script's
//!   digits (UNICODE §6) and is what the reader actually sees.
//!
//! Treating any of these as a plain number silently loses text or moves it.

use std::collections::BTreeMap;

use crate::{ByteSpan, Diagnostic, DiagnosticCode, Node, NodeKind, Severity};

/// A verse number: a range with an optional segment letter.
///
/// A single verse is a range of one, so callers never branch on the
/// distinction — which is the point, since forgetting to handle `1-2` is how a
/// verse goes missing from an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct VerseId {
    pub start: u16,
    pub end: u16,
    /// The `a` in `\v 1a`.
    pub segment: Option<char>,
}

impl VerseId {
    pub const fn single(number: u16) -> Self {
        Self {
            start: number,
            end: number,
            segment: None,
        }
    }

    /// Parses `1`, `1-2`, `1a`, or `1-2a`.
    ///
    /// Returns `None` for anything else, including non-ASCII digits: USFM
    /// requires `\v` numbers to be ASCII, and the published form belongs in
    /// `\vp` (UNICODE §6). Reporting that is [`non_ascii_digits`]'s job.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        // The segment letter may be from any script. The Septuagint numbers
        // its segments α, β, γ — twenty-odd times in Proverbs alone in the
        // committed corpus — and rejecting those reports real, published
        // Scripture as malformed. Only the *number* must be ASCII; the
        // specification constrains the digits, not the letter.
        //
        // Sliced by character rather than by byte, because a Greek letter is
        // two bytes and `len() - 1` would land inside it.
        let (numbers, segment) = match text.char_indices().next_back() {
            Some((offset, last)) if last.is_alphabetic() => (&text[..offset], Some(last)),
            _ => (text, None),
        };

        let (start, end) = match numbers.split_once('-') {
            Some((first, second)) => (first.trim(), second.trim()),
            None => (numbers, numbers),
        };

        let start: u16 = parse_ascii(start)?;
        let end: u16 = parse_ascii(end)?;
        // A reversed range is malformed rather than empty; normalising it
        // would hide the error and index the verses under the wrong numbers.
        (start <= end).then_some(Self {
            start,
            end,
            segment,
        })
    }

    /// Whether this covers `number`.
    pub const fn contains(&self, number: u16) -> bool {
        self.start <= number && number <= self.end
    }

    /// Whether the two cover any number in common.
    ///
    /// **Different segments never collide**, including when one of them is
    /// absent. `\v 1a` and `\v 1b` are two halves of one verse; `\v 22`
    /// followed by `\v 22α` is how the Septuagint marks material additional
    /// to verse 22, and both appear in the committed corpus. Treating a
    /// segment as colliding with the bare verse it qualifies produced five
    /// spurious errors in Proverbs alone.
    ///
    /// So a collision needs the same segment — or neither — and then
    /// overlapping ranges. That leaves the cases that are genuinely wrong: the
    /// same verse stated twice, and a range covering a verse stated separately.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.segment == other.segment && self.start <= other.end && other.start <= self.end
    }
}

/// Rejects a number that is not plain ASCII digits, rather than accepting
/// whatever `str::parse` tolerates.
fn parse_ascii(text: &str) -> Option<u16> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// Whether a `\v` number uses digits from a script other than ASCII.
///
/// `USFM-E018`. The published number belongs in `\vp`, which renders as-is.
pub fn non_ascii_digits(text: &str) -> bool {
    text.chars().any(|c| !c.is_ascii() && c.is_numeric())
}

impl std::fmt::Display for VerseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)?;
        } else {
            write!(f, "{}-{}", self.start, self.end)?;
        }
        if let Some(segment) = self.segment {
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

/// One verse, as it appears in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerseEntry {
    pub chapter: u16,
    pub verse: VerseId,
    /// `\vp` — what the reader sees. May be in any script's digits, so it
    /// stays a string rather than being parsed into a number.
    pub published: Option<String>,
    /// `\va` — the number in another versification.
    pub alternate: Option<VerseId>,
    /// The `\v` marker itself. Byte offsets; converted at the boundary like
    /// every other span.
    pub span: ByteSpan,
    /// The raw `\v` number, kept so a malformed one can still be reported and
    /// displayed.
    pub raw: String,
}

/// Every verse in a document, in source order.
#[derive(Debug, Clone, Default)]
pub struct VerseIndex {
    entries: Vec<VerseEntry>,
}

impl VerseIndex {
    /// Builds the index from a document tree.
    pub fn build(nodes: &[Node]) -> Self {
        let mut entries = Vec::new();
        let mut chapter = 0u16;

        for root in nodes {
            for node in root.descendants() {
                match node.kind {
                    NodeKind::Chapter => {
                        chapter = node
                            .attribute("number")
                            .and_then(parse_ascii)
                            .unwrap_or(chapter);
                    }
                    NodeKind::Verse => {
                        let Some(raw) = node.attribute("number") else {
                            continue;
                        };
                        let Some(span) = node.span.clone() else {
                            continue;
                        };
                        // A number we cannot parse still gets an entry, so it
                        // is visible to diagnostics and to the interface. A
                        // dropped verse is worse than a malformed one.
                        let verse = VerseId::parse(raw).unwrap_or(VerseId::single(0));

                        entries.push(VerseEntry {
                            chapter,
                            verse,
                            published: node.attribute("pubnumber").map(str::to_string),
                            alternate: node.attribute("altnumber").and_then(VerseId::parse),
                            span,
                            raw: raw.to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        Self { entries }
    }

    pub fn entries(&self) -> &[VerseEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry covering `chapter:verse`, taking ranges into account.
    pub fn find(&self, chapter: u16, verse: u16) -> Option<&VerseEntry> {
        self.entries
            .iter()
            .find(|entry| entry.chapter == chapter && entry.verse.contains(verse))
    }

    /// What is wrong with the numbering.
    ///
    /// Cross-chunk by nature — no chapter can see whether another repeats its
    /// verses — which is why this is Tier 3 work (ARCHITECTURE §8.2).
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // ---- malformed and non-ASCII numbers ----
        for entry in &self.entries {
            if non_ascii_digits(&entry.raw) {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::NonAsciiVerseDigits,
                    severity: Severity::Error,
                    span: entry.span.clone(),
                    message: format!(
                        "\\v {} uses non-ASCII digits; USFM verse numbers are ASCII, \
                         and the published form belongs in \\vp",
                        entry.raw
                    ),
                });
            } else if VerseId::parse(&entry.raw).is_none() {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::MissingVerseNumber,
                    severity: Severity::Error,
                    span: entry.span.clone(),
                    message: format!("\\v {} is not a verse number", entry.raw),
                });
            }
        }

        // ---- overlaps ----
        //
        // Only entries whose number actually parsed. An unparseable one is
        // recorded as verse 0 so it stays visible, and treating those as real
        // numbers made every malformed verse in a chapter "overlap" every
        // other — twenty duplicate-verse errors that were all the same single
        // problem, already reported above.
        let mut by_chapter: BTreeMap<u16, Vec<&VerseEntry>> = BTreeMap::new();
        for entry in &self.entries {
            if VerseId::parse(&entry.raw).is_some() {
                by_chapter.entry(entry.chapter).or_default().push(entry);
            }
        }

        for (chapter, entries) in &by_chapter {
            for (index, entry) in entries.iter().enumerate() {
                for earlier in &entries[..index] {
                    if earlier.verse.overlaps(&entry.verse) {
                        diagnostics.push(Diagnostic {
                            code: DiagnosticCode::DuplicateVerse,
                            severity: Severity::Error,
                            span: entry.span.clone(),
                            message: format!(
                                "{chapter}:{} overlaps {chapter}:{}, which appears earlier",
                                entry.verse, earlier.verse
                            ),
                        });
                        break;
                    }
                }
            }

            // ---- gaps ----
            //
            // Information, not a warning: a chapter that omits a verse is
            // ordinary in published Scripture, where a versification difference
            // or a text-critical decision routinely leaves a number unused.
            // Reporting it as a problem would cry wolf on most real files.
            let Some(first) = entries.first() else {
                continue;
            };
            let covered: Vec<(u16, u16)> = entries
                .iter()
                .map(|entry| (entry.verse.start, entry.verse.end))
                .collect();
            let highest = covered.iter().map(|(_, end)| *end).max().unwrap_or(0);

            for number in 1..=highest {
                if !covered
                    .iter()
                    .any(|(start, end)| *start <= number && number <= *end)
                {
                    diagnostics.push(Diagnostic {
                        code: DiagnosticCode::VerseGap,
                        severity: Severity::Information,
                        span: first.span.clone(),
                        message: format!("chapter {chapter} has no verse {number}"),
                    });
                }
            }
        }

        diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn index_of(source: &str) -> VerseIndex {
        let document = Document::parse(source.to_string());
        VerseIndex::build(document.content())
    }

    #[test]
    fn a_plain_number_is_a_range_of_one() {
        assert_eq!(VerseId::parse("1"), Some(VerseId::single(1)));
        assert_eq!(VerseId::parse("12"), Some(VerseId::single(12)));
    }

    #[test]
    fn a_range_keeps_both_ends() {
        let range = VerseId::parse("1-2").expect("a range");
        assert_eq!((range.start, range.end), (1, 2));
        assert!(range.contains(1) && range.contains(2));
        assert!(!range.contains(3));
    }

    #[test]
    fn a_segment_letter_is_kept() {
        let segment = VerseId::parse("1a").expect("a segment");
        assert_eq!(segment.start, 1);
        assert_eq!(segment.segment, Some('a'));
        assert_eq!(VerseId::parse("1-2a").map(|v| v.segment), Some(Some('a')));
    }

    #[test]
    fn a_segment_letter_may_be_greek() {
        // The Septuagint numbers segments α, β, γ. Found in the committed
        // corpus, in Proverbs, twenty-odd times in one file — reported as
        // malformed until the parser stopped insisting on ASCII letters.
        let segment = VerseId::parse("18\u{3b1}").expect("a Greek segment");
        assert_eq!(segment.start, 18);
        assert_eq!(segment.segment, Some('\u{3b1}'));
    }

    #[test]
    fn different_segments_never_collide() {
        // \v 1a and \v 1b are two halves of one verse.
        let a = VerseId::parse("1a").unwrap();
        let b = VerseId::parse("1b").unwrap();
        assert!(!a.overlaps(&b));

        // And \v 22 followed by \v 22α is how the Septuagint marks additional
        // material, not a duplicate. Real, and in the corpus.
        let plain = VerseId::single(22);
        let alpha = VerseId::parse("22\u{3b1}").unwrap();
        assert!(!plain.overlaps(&alpha));

        // The same segment stated twice is still a duplicate.
        assert!(alpha.overlaps(&VerseId::parse("22\u{3b1}").unwrap()));
    }

    #[test]
    fn malformed_numbers_are_refused_rather_than_coerced() {
        assert_eq!(VerseId::parse(""), None);
        assert_eq!(VerseId::parse("abc"), None);
        assert_eq!(VerseId::parse("-1"), None);
        // Reversed. Normalising would index the verses under the wrong numbers.
        assert_eq!(VerseId::parse("5-2"), None);
        // Non-ASCII digits are not verse numbers.
        assert_eq!(VerseId::parse("௩"), None);
    }

    #[test]
    fn the_index_records_chapter_and_verse() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\v 2 b\n\\c 2\n\\p\n\\v 1 c\n");
        let entries = index.entries();

        assert_eq!(entries.len(), 3);
        assert_eq!((entries[0].chapter, entries[0].verse.start), (1, 1));
        assert_eq!((entries[2].chapter, entries[2].verse.start), (2, 1));
        assert!(index.find(2, 1).is_some());
        assert!(index.find(9, 1).is_none());
    }

    #[test]
    fn a_range_is_found_by_either_of_its_numbers() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1-2 both\n\\v 3 c\n");
        assert!(index.find(1, 1).is_some());
        assert!(index.find(1, 2).is_some());
        assert_eq!(index.find(1, 1), index.find(1, 2));
    }

    #[test]
    fn alternate_and_published_numbers_are_kept() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1 \\va 2\\va* \\vp \u{BE9}\\vp* text\n");
        let entry = &index.entries()[0];

        assert_eq!(entry.alternate, Some(VerseId::single(2)));
        assert_eq!(entry.published.as_deref(), Some("\u{BE9}"));
    }

    #[test]
    fn a_range_overlapping_a_separate_verse_is_a_duplicate() {
        // The case a plain integer model misses entirely.
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1-2 both\n\\v 2 again\n");
        assert!(index
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateVerse));
    }

    #[test]
    fn a_repeated_verse_is_a_duplicate() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\v 1 b\n");
        assert!(index
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateVerse));
    }

    #[test]
    fn the_same_number_in_different_chapters_is_not_a_duplicate() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\c 2\n\\p\n\\v 1 b\n");
        assert!(!index
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::DuplicateVerse));
    }

    #[test]
    fn a_missing_verse_is_information_rather_than_a_problem() {
        // Ordinary in published Scripture; a warning here would cry wolf on
        // most real files.
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\v 3 c\n");
        let gap = index
            .diagnostics()
            .into_iter()
            .find(|d| d.code == DiagnosticCode::VerseGap)
            .expect("the gap at verse 2");

        assert_eq!(gap.severity, Severity::Information);
        assert!(gap.message.contains("verse 2"), "{}", gap.message);
    }

    #[test]
    fn a_range_fills_the_gap_it_covers() {
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v 1-3 all\n\\v 4 d\n");
        assert!(!index
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::VerseGap));
    }

    #[test]
    fn non_ascii_verse_digits_are_an_error() {
        // USFM-E018, fixed in UNICODE §6 long before anything emitted it.
        let index = index_of("\\id GEN\n\\c 1\n\\p\n\\v \u{BE9} text\n");
        let found = index
            .diagnostics()
            .into_iter()
            .find(|d| d.code == DiagnosticCode::NonAsciiVerseDigits);

        assert!(found.is_some(), "non-ASCII \\v number went unreported");
        assert_eq!(found.unwrap().severity, Severity::Error);
    }
}
