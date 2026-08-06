//! Scripture references, as a person types them.
//!
//! PRODUCT §6.2 accepts `GEN 1:1`, `Gen 1.1`, `1:1`, and `3`; falls back to
//! matching `\vp` published numbers; and accepts non-ASCII digits.
//!
//! # Digits
//!
//! UNICODE §6: `௩:௧` and `3:1` resolve identically, "via Unicode
//! `Numeric_Value` rather than a hardcoded table, so every script works
//! without enumeration". That constraint is the whole design of
//! [`decimal_value`] — a match arm per script is a list that is wrong the day
//! a translation team uses a script nobody enumerated, and there is no way for
//! them to tell it is the list and not their file that is at fault.

use crate::verse::VerseIndex;
use crate::ByteSpan;

/// The value of a decimal digit in any script.
///
/// UNICODE §6 asks for this to work "via Unicode `Numeric_Value` rather than a
/// hardcoded table, so every script works without enumeration". `std` exposes
/// `char::is_numeric` — the whole `N` category — but no numeric value, and
/// `char::to_digit` is ASCII-only. This closes the gap using the one thing the
/// standard guarantees about the shape of the data: decimal digits come in
/// contiguous runs of exactly ten, in ascending order from zero (UAX #44,
/// `Numeric_Type=Decimal`).
///
/// So the run start is found by walking back to a non-numeric neighbour, and
/// the value is the distance. Both halves of that need care, and each is a
/// real character that broke it:
///
/// - **The walk must be able to fail.** Roman numerals are `is_numeric` and
///   sit in a run far longer than ten (U+2160 onward, after the fractions), so
///   a walk that always yields a start reports Ⅰ as a digit. Ten steps without
///   reaching a boundary means this is not a ten-long run.
/// - **A boundary is not enough.** `¼½¾` is a numeric run three long, so ½ has
///   a clean start one step back and would read as the digit one. The run has
///   to be checked for actually being ten long.
pub fn decimal_value(character: char) -> Option<u32> {
    if let Some(digit) = character.to_digit(10) {
        return Some(digit);
    }
    if !character.is_numeric() {
        return None;
    }

    let code = character as u32;
    let mut start = code;
    let mut bounded = false;

    for _ in 0..10 {
        match start.checked_sub(1).and_then(char::from_u32) {
            Some(previous) if previous.is_numeric() => start -= 1,
            _ => {
                bounded = true;
                break;
            }
        }
    }

    // Ten steps and still numeric: a longer run, so not decimal digits.
    if !bounded {
        return None;
    }

    let value = code - start;
    if value > 9 {
        return None;
    }

    // And the run has to be ten long, or a short numeric run reads as digits.
    (0..10)
        .all(|offset| char::from_u32(start + offset).is_some_and(char::is_numeric))
        .then_some(value)
}

/// Reads a run of digits from any single script as a number.
///
/// `None` if the text is empty, holds anything that is not a digit, or names a
/// number too large to be a chapter. Mixed scripts are accepted — nobody types
/// `௩4` on purpose, but refusing it would mean explaining why, and the value
/// is unambiguous either way.
pub fn parse_digits(text: &str) -> Option<u16> {
    let mut value: u32 = 0;
    let mut any = false;

    for character in text.chars() {
        let digit = decimal_value(character)?;
        value = value.checked_mul(10)?.checked_add(digit)?;
        if value > u16::MAX as u32 {
            return None;
        }
        any = true;
    }

    any.then_some(value as u16)
}

/// A reference someone has typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The book code, upper-cased. `None` when the reference did not name one,
    /// which is the common case in a one-file editor.
    pub book: Option<String>,
    pub chapter: u16,
    /// `None` for a bare chapter, which means the top of it.
    pub verse: Option<u16>,
    /// The verse exactly as typed, for the `\vp` fallback — a published number
    /// is a string, and may not be a number this side can compare against.
    pub verse_text: Option<String>,
}

impl Reference {
    /// Parses the accepted forms, and nothing else.
    ///
    /// The book code is told from the chapter by whether the first token is
    /// all digits, rather than by position or length. Book codes may *start*
    /// with a digit — `1CO`, `2SA`, `3JN` — so "begins with a letter" is the
    /// wrong test and rejects a third of the New Testament.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        let mut book = None;
        let mut rest = text;

        // A leading token is the book only when something follows it. Without
        // that condition `1:1` reads as a book code named "1:1", since it is
        // not a bare number either.
        if let Some((first, tail)) = text.split_once(char::is_whitespace) {
            if parse_digits(first).is_none() {
                book = Some(first.to_ascii_uppercase());
                rest = tail.trim_start();
            }
        }

        // `:` and `.` are both in use; PRODUCT §6.2 names both forms.
        let (chapter_text, verse_text) = match rest.split_once([':', '.']) {
            Some((chapter, verse)) => (chapter.trim(), Some(verse.trim())),
            None => (rest.trim(), None),
        };

        let Some(chapter) = parse_digits(chapter_text) else {
            // Not a number. A bare book code is the one remaining reading, and
            // only when nothing has already been taken as the book and there is
            // no chapter separator left unexplained.
            if book.is_none() && verse_text.is_none() && !chapter_text.is_empty() {
                return Some(Self {
                    book: Some(chapter_text.to_ascii_uppercase()),
                    chapter: 1,
                    verse: None,
                    verse_text: None,
                });
            }
            return None;
        };

        // An empty tail — `1:` — is someone mid-typing, and means the chapter.
        let verse_text = verse_text.filter(|text| !text.is_empty());
        let verse = match verse_text {
            // Kept even when it does not parse: a published number may be
            // anything, and the fallback compares against the text.
            Some(text) => parse_digits(text),
            None => None,
        };

        Some(Self {
            book,
            chapter,
            verse,
            verse_text: verse_text.map(str::to_string),
        })
    }
}

/// What came of looking a reference up.
///
/// An enum rather than an `Option<ByteSpan>` because every failure here has a
/// different thing to tell the user, and "not found" tells them none of it.
/// Being in the wrong file is the one that matters most: this is a one-file
/// editor, so `GEN 1:1` typed into Exodus is not a typo to correct but a
/// document to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Found(ByteSpan),
    Unparseable,
    WrongBook {
        document: Option<String>,
        asked: String,
    },
    NoSuchChapter(u16),
    NoSuchVerse {
        chapter: u16,
        verse: String,
    },
}

/// Looks a reference up in a verse index.
///
/// `chapter_start` supplies where a chapter begins, which the verse index does
/// not know — a chapter with no verses still has a location, and `\c 3` with
/// nothing under it is exactly the kind of half-finished file this editor
/// exists to be usable on.
pub fn resolve(
    reference: &Reference,
    index: &VerseIndex,
    book: Option<&str>,
    chapter_start: impl Fn(u16) -> Option<ByteSpan>,
) -> Resolution {
    if let Some(asked) = &reference.book {
        // Compared only when the document says what it is. A file whose `\id`
        // is missing or malformed is one this editor should still navigate.
        if let Some(document) = book {
            if !document.eq_ignore_ascii_case(asked) {
                return Resolution::WrongBook {
                    document: Some(document.to_string()),
                    asked: asked.clone(),
                };
            }
        }
    }

    let Some(verse_text) = reference.verse_text.as_deref() else {
        return match chapter_start(reference.chapter) {
            Some(span) => Resolution::Found(span),
            None => Resolution::NoSuchChapter(reference.chapter),
        };
    };

    if let Some(verse) = reference.verse {
        if let Some(entry) = index.find(reference.chapter, verse) {
            return Resolution::Found(entry.span.clone());
        }
    }

    // The `\vp` fallback. A published number is what the reader sees, so it is
    // what they will type — and it need not be the `\v` number at all, which
    // is the entire reason `\vp` exists.
    if let Some(entry) = find_published(index, reference.chapter, verse_text) {
        return Resolution::Found(entry);
    }

    if chapter_start(reference.chapter).is_none() {
        return Resolution::NoSuchChapter(reference.chapter);
    }
    Resolution::NoSuchVerse {
        chapter: reference.chapter,
        verse: verse_text.to_string(),
    }
}

/// A verse whose `\vp` matches what was typed.
///
/// Compared by value, not by string: a Tamil file's `\vp ௩` has to be findable
/// by someone typing `3` on a keyboard that cannot produce Tamil digits, and
/// by someone typing `௩` on one that can.
fn find_published(index: &VerseIndex, chapter: u16, typed: &str) -> Option<ByteSpan> {
    let wanted = parse_digits(typed);

    index
        .entries()
        .iter()
        .find(|entry| {
            if entry.chapter != chapter {
                return false;
            }
            let Some(published) = entry.published.as_deref() else {
                return false;
            };
            published == typed || (wanted.is_some() && parse_digits(published) == wanted)
        })
        .map(|entry| entry.span.clone())
}

/// How a position reads as a reference, for the status bar.
///
/// The last verse at or before `offset`, which is what "where am I" means in a
/// document: the text between two `\v` markers belongs to the first of them.
///
/// `chapter` is which chapter the offset is actually in, and it is what stops
/// the answer being confidently wrong. A cursor on a `\c 2` line sits *after*
/// the last verse of chapter 1, so the verse index alone reports 1:2 — the
/// status bar naming the wrong chapter at the exact moment the user has just
/// navigated to a new one. Where the two disagree, the position is between
/// chapters and the honest answer is the chapter alone.
pub fn reference_at(
    index: &VerseIndex,
    book: Option<&str>,
    offset: usize,
    chapter: Option<u16>,
) -> Option<String> {
    // The entries are in source order, so the last one that starts at or
    // before the cursor is the one the cursor is inside.
    let entry = index
        .entries()
        .iter()
        .rev()
        .find(|entry| entry.span.start <= offset)
        .filter(|entry| chapter.is_none_or(|chapter| chapter == entry.chapter));

    let Some(entry) = entry else {
        // No verse yet in this chapter, or none before the cursor at all.
        let chapter = chapter?;
        return Some(match book {
            Some(book) => format!("{book} {chapter}"),
            None => chapter.to_string(),
        });
    };

    // The published number is shown when there is one, because it is what the
    // reader sees on the page and therefore what they are looking for.
    let verse = entry
        .published
        .clone()
        .unwrap_or_else(|| entry.verse.to_string());

    Some(match book {
        Some(book) => format!("{book} {}:{verse}", entry.chapter),
        None => format!("{}:{verse}", entry.chapter),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_digits_read_as_themselves() {
        assert_eq!(parse_digits("0"), Some(0));
        assert_eq!(parse_digits("150"), Some(150));
        assert_eq!(parse_digits(""), None);
        assert_eq!(parse_digits("1a"), None);
    }

    #[test]
    fn every_script_works_without_being_enumerated() {
        // None of these appear anywhere in this file, which is the point:
        // the value comes from Unicode's own structure, so a script nobody
        // thought of works too.
        assert_eq!(parse_digits("\u{BE9}"), Some(3)); // Tamil ௩
        assert_eq!(parse_digits("\u{966}\u{967}"), Some(1)); // Devanagari ०१
        assert_eq!(parse_digits("\u{6F3}"), Some(3)); // Extended Arabic-Indic ۳
        assert_eq!(parse_digits("\u{FF11}\u{FF10}"), Some(10)); // Fullwidth １０
        assert_eq!(parse_digits("\u{1041}"), Some(1)); // Myanmar one
    }

    #[test]
    fn a_numeral_that_is_not_a_digit_is_refused() {
        // U+0BF0 TAMIL NUMBER TEN sits immediately after ௦–௯ and is numeric,
        // but it is not a digit and must not read as one.
        assert_eq!(parse_digits("\u{BF0}"), None);
        // Roman numerals and fractions are numeric too.
        assert_eq!(parse_digits("\u{2160}"), None); // Ⅰ
        assert_eq!(parse_digits("\u{00BD}"), None); // ½
    }

    #[test]
    fn a_number_too_large_for_a_chapter_is_refused_rather_than_wrapped() {
        assert_eq!(parse_digits("99999"), None);
        assert_eq!(parse_digits("65536"), None);
        assert_eq!(parse_digits("65535"), Some(65535));
    }

    #[test]
    fn the_accepted_forms_parse() {
        let full = Reference::parse("GEN 1:1").expect("book chapter verse");
        assert_eq!(full.book.as_deref(), Some("GEN"));
        assert_eq!((full.chapter, full.verse), (1, Some(1)));

        // A dot separator, and a lower-case book code.
        let dotted = Reference::parse("Gen 1.1").expect("dot form");
        assert_eq!(dotted.book.as_deref(), Some("GEN"));
        assert_eq!((dotted.chapter, dotted.verse), (1, Some(1)));

        let bare = Reference::parse("1:1").expect("chapter and verse");
        assert_eq!(bare.book, None);
        assert_eq!((bare.chapter, bare.verse), (1, Some(1)));

        let chapter = Reference::parse("3").expect("a chapter");
        assert_eq!((chapter.chapter, chapter.verse), (3, None));
    }

    #[test]
    fn a_book_code_may_begin_with_a_digit() {
        // 1CO, 2SA, 3JN. Telling the book from the chapter by "starts with a
        // letter" rejects a third of the New Testament.
        for code in ["1CO", "2SA", "3JN"] {
            let reference = Reference::parse(&format!("{code} 2:3")).expect("a numbered book code");
            assert_eq!(reference.book.as_deref(), Some(code));
            assert_eq!((reference.chapter, reference.verse), (2, Some(3)));
        }
    }

    #[test]
    fn a_reference_in_another_script_reads_the_same_as_in_ascii() {
        let tamil = Reference::parse("\u{BE9}:\u{BE7}").expect("௩:௧");
        let ascii = Reference::parse("3:1").expect("3:1");
        assert_eq!((tamil.chapter, tamil.verse), (ascii.chapter, ascii.verse));
    }

    #[test]
    fn a_half_typed_reference_means_the_chapter() {
        let partial = Reference::parse("2:").expect("mid-typing");
        assert_eq!((partial.chapter, partial.verse), (2, None));
        assert_eq!(partial.verse_text, None);
    }

    #[test]
    fn nonsense_is_refused() {
        assert_eq!(Reference::parse(""), None);
        assert_eq!(Reference::parse("   "), None);
        assert_eq!(Reference::parse("GEN x:1"), None);
    }

    #[test]
    fn a_bare_book_code_means_its_first_chapter() {
        let book = Reference::parse("gen").expect("a book on its own");
        assert_eq!(book.book.as_deref(), Some("GEN"));
        assert_eq!((book.chapter, book.verse), (1, None));
    }
}
